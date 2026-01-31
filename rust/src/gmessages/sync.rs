// Google Messages sync loop
// Handles periodic polling and data synchronization

use crate::gmessages::db::GmDatabase;
use crate::gmessages::models::{GmError, GmMessage, GmPairingState, GmThreadSummary};
use crate::gmessages::protocol::GmProtocolClient;
use crate::gmessages::session::GmSessionManager;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// Sync configuration
pub struct GmSyncConfig {
    /// Interval between syncs when app is in foreground (ms)
    pub foreground_interval_ms: u64,
    /// Interval between syncs when app is in background (ms)
    /// Set to 0 to disable background sync
    pub background_interval_ms: u64,
    /// Maximum number of consecutive errors before stopping
    pub max_consecutive_errors: u32,
    /// Base delay for exponential backoff on errors (ms)
    pub error_backoff_base_ms: u64,
    /// Maximum backoff delay (ms)
    pub error_backoff_max_ms: u64,
}

impl Default for GmSyncConfig {
    fn default() -> Self {
        Self {
            foreground_interval_ms: 15_000,  // 15 seconds
            background_interval_ms: 0,       // Disabled for MVP (battery-safe)
            max_consecutive_errors: 5,
            error_backoff_base_ms: 5_000,    // 5 seconds
            error_backoff_max_ms: 300_000,   // 5 minutes
        }
    }
}

/// Sync loop state
pub struct GmSyncLoop {
    session: Arc<GmSessionManager>,
    db: Arc<GmDatabase>,
    protocol: Arc<GmProtocolClient>,
    config: GmSyncConfig,
    /// Whether the sync loop is running
    running: AtomicBool,
    /// Whether app is in foreground
    is_foreground: AtomicBool,
    /// Notify to wake up sync loop for immediate sync
    sync_notify: Notify,
    /// Consecutive error count
    consecutive_errors: std::sync::atomic::AtomicU32,
    /// Last sync timestamp
    last_sync_at: std::sync::atomic::AtomicI64,
    /// Last error message
    last_error: std::sync::RwLock<Option<String>>,
    /// Callback for sync events (new messages, etc.)
    on_sync_callback: std::sync::RwLock<Option<Box<dyn Fn(GmSyncEvent) + Send + Sync>>>,
}

/// Events emitted by sync loop
#[derive(Debug, Clone)]
pub enum GmSyncEvent {
    /// Sync started
    Started,
    /// Sync completed successfully
    Completed { threads_updated: usize, messages_new: usize },
    /// Sync failed
    Failed { error: String },
    /// New messages received
    NewMessages { thread_id: String, count: usize },
    /// Session expired
    SessionExpired,
}

impl GmSyncLoop {
    pub fn new(
        session: Arc<GmSessionManager>,
        db: Arc<GmDatabase>,
        protocol: Arc<GmProtocolClient>,
        config: GmSyncConfig,
    ) -> Self {
        Self {
            session,
            db,
            protocol,
            config,
            running: AtomicBool::new(false),
            is_foreground: AtomicBool::new(true),
            sync_notify: Notify::new(),
            consecutive_errors: std::sync::atomic::AtomicU32::new(0),
            last_sync_at: std::sync::atomic::AtomicI64::new(0),
            last_error: std::sync::RwLock::new(None),
            on_sync_callback: std::sync::RwLock::new(None),
        }
    }

    /// Set callback for sync events
    pub fn set_sync_callback<F>(&self, callback: F)
    where
        F: Fn(GmSyncEvent) + Send + Sync + 'static,
    {
        *self.on_sync_callback.write().unwrap() = Some(Box::new(callback));
    }

    fn emit_event(&self, event: GmSyncEvent) {
        if let Some(ref callback) = *self.on_sync_callback.read().unwrap() {
            callback(event);
        }
    }

    /// Start the sync loop
    pub async fn start(&self) {
        if self.running.swap(true, Ordering::SeqCst) {
            log::warn!("GM sync loop already running");
            return;
        }

        log::info!("Starting GM sync loop");

        while self.running.load(Ordering::SeqCst) {
            // Check if we should sync
            if !self.should_sync() {
                // Wait for next interval or manual trigger
                let interval = self.get_current_interval();
                if interval > 0 {
                    tokio::select! {
                        _ = tokio::time::sleep(Duration::from_millis(interval)) => {},
                        _ = self.sync_notify.notified() => {},
                    }
                } else {
                    // Background sync disabled, just wait for notify
                    self.sync_notify.notified().await;
                }
                continue;
            }

            // Perform sync
            self.emit_event(GmSyncEvent::Started);
            
            match self.do_sync().await {
                Ok((threads, messages)) => {
                    self.consecutive_errors.store(0, Ordering::SeqCst);
                    self.last_sync_at.store(current_timestamp_millis(), Ordering::SeqCst);
                    *self.last_error.write().unwrap() = None;
                    
                    self.emit_event(GmSyncEvent::Completed {
                        threads_updated: threads,
                        messages_new: messages,
                    });
                }
                Err(e) => {
                    let error_count = self.consecutive_errors.fetch_add(1, Ordering::SeqCst) + 1;
                    let error_msg = e.to_string();
                    *self.last_error.write().unwrap() = Some(error_msg.clone());
                    
                    log::error!("GM sync error ({}): {}", error_count, error_msg);
                    self.emit_event(GmSyncEvent::Failed { error: error_msg });

                    // Check for session expiry
                    if matches!(e, GmError::NotAuthenticated) {
                        self.session.mark_expired();
                        self.emit_event(GmSyncEvent::SessionExpired);
                        // Stop loop on auth failure
                        self.running.store(false, Ordering::SeqCst);
                        break;
                    }

                    // Check if we've hit max errors
                    if error_count >= self.config.max_consecutive_errors {
                        log::error!("GM sync: max consecutive errors reached, pausing");
                        // Don't stop completely, just wait longer
                        tokio::time::sleep(Duration::from_millis(self.config.error_backoff_max_ms)).await;
                        self.consecutive_errors.store(0, Ordering::SeqCst);
                    } else {
                        // Exponential backoff
                        let backoff = std::cmp::min(
                            self.config.error_backoff_base_ms * (1 << error_count),
                            self.config.error_backoff_max_ms,
                        );
                        tokio::time::sleep(Duration::from_millis(backoff)).await;
                    }
                }
            }

            // Wait for next interval
            let interval = self.get_current_interval();
            if interval > 0 {
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(interval)) => {},
                    _ = self.sync_notify.notified() => {},
                }
            }
        }

        log::info!("GM sync loop stopped");
    }

    /// Stop the sync loop
    pub fn stop(&self) {
        log::info!("Stopping GM sync loop");
        self.running.store(false, Ordering::SeqCst);
        self.sync_notify.notify_one();
    }

    /// Trigger immediate sync
    pub fn sync_now(&self) {
        log::info!("Triggering immediate GM sync");
        self.sync_notify.notify_one();
    }

    /// Set foreground/background state
    pub fn set_foreground(&self, is_foreground: bool) {
        let was_foreground = self.is_foreground.swap(is_foreground, Ordering::SeqCst);
        if !was_foreground && is_foreground {
            // Coming to foreground - trigger immediate sync
            log::info!("GM: app came to foreground, triggering sync");
            self.sync_now();
        }
    }

    /// Check if sync loop is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get last sync timestamp
    pub fn get_last_sync_at(&self) -> i64 {
        self.last_sync_at.load(Ordering::SeqCst)
    }

    /// Get last error
    pub fn get_last_error(&self) -> Option<String> {
        self.last_error.read().unwrap().clone()
    }

    /// Check if we should sync
    fn should_sync(&self) -> bool {
        // Must have valid session
        if self.session.get_pairing_state() != GmPairingState::Paired {
            return false;
        }

        // Must be authenticated
        if !self.protocol.is_authenticated() {
            return false;
        }

        true
    }

    /// Get current sync interval based on foreground/background state
    fn get_current_interval(&self) -> u64 {
        if self.is_foreground.load(Ordering::SeqCst) {
            self.config.foreground_interval_ms
        } else {
            self.config.background_interval_ms
        }
    }

    /// Perform sync operation
    async fn do_sync(&self) -> Result<(usize, usize), GmError> {
        log::debug!("GM sync: starting");
        
        let mut threads_updated = 0;
        let mut messages_new = 0;

        // Fetch threads
        let threads = self.protocol.fetch_threads().await?;
        
        for thread in threads {
            // Upsert thread
            self.db.upsert_thread(thread.clone())?;
            threads_updated += 1;

            // Fetch messages for thread with cursor
            let cursor = self.db.get_cursor(&thread.remote_thread_id);
            let (messages, new_cursor) = self
                .protocol
                .fetch_messages(&thread.remote_thread_id, cursor.as_deref(), 50)
                .await?;

            let new_count = messages.len();
            for msg in messages {
                self.db.upsert_message(msg)?;
            }

            if new_count > 0 {
                messages_new += new_count;
                self.emit_event(GmSyncEvent::NewMessages {
                    thread_id: thread.remote_thread_id.clone(),
                    count: new_count,
                });
            }

            // Update cursor
            if let Some(cursor) = new_cursor {
                self.db.update_cursor(&thread.remote_thread_id, &cursor)?;
            }
        }

        log::debug!(
            "GM sync: complete - {} threads, {} new messages",
            threads_updated,
            messages_new
        );

        Ok((threads_updated, messages_new))
    }
}

/// Sync with data provided from WebView (Flutter bridge)
/// This is used when the WebView extracts data from GM web
pub fn sync_from_webview_data(
    db: &GmDatabase,
    threads_json: Option<&str>,
    messages_json: Option<&str>,
    thread_id: Option<&str>,
) -> Result<(usize, usize), GmError> {
    let mut threads_updated = 0;
    let mut messages_new = 0;

    // Process threads
    if let Some(json) = threads_json {
        let threads = crate::gmessages::protocol::parse_threads_from_webview(json)?;
        for thread in threads {
            db.upsert_thread(thread)?;
            threads_updated += 1;
        }
    }

    // Process messages
    if let (Some(json), Some(tid)) = (messages_json, thread_id) {
        let messages = crate::gmessages::protocol::parse_messages_from_webview(tid, json)?;
        for msg in messages {
            db.upsert_message(msg)?;
            messages_new += 1;
        }
    }

    Ok((threads_updated, messages_new))
}

fn current_timestamp_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_config_defaults() {
        let config = GmSyncConfig::default();
        assert_eq!(config.foreground_interval_ms, 15_000);
        assert_eq!(config.background_interval_ms, 0);
    }
}
