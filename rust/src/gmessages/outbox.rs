// Google Messages outbox/send queue
// Handles message queuing with retry logic

use crate::gmessages::db::GmDatabase;
use crate::gmessages::models::{GmError, GmMessageStatus, GmOutboxItem, GmSendResult};
use crate::gmessages::protocol::GmProtocolClient;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// Outbox processor configuration
pub struct GmOutboxConfig {
    /// How often to check for pending messages (ms)
    pub check_interval_ms: u64,
    /// Maximum concurrent sends
    pub max_concurrent_sends: usize,
    /// Maximum retry attempts per message
    pub max_retry_attempts: u32,
}

impl Default for GmOutboxConfig {
    fn default() -> Self {
        Self {
            check_interval_ms: 5_000,     // 5 seconds
            max_concurrent_sends: 1,       // Sequential for now
            max_retry_attempts: 5,
        }
    }
}

/// Outbox processor that sends queued messages
pub struct GmOutboxProcessor {
    db: Arc<GmDatabase>,
    protocol: Arc<GmProtocolClient>,
    config: GmOutboxConfig,
    /// Whether the processor is running
    running: AtomicBool,
    /// Notify to wake up processor for new messages
    send_notify: Notify,
    /// Callback for send status updates
    on_status_callback: std::sync::RwLock<Option<Box<dyn Fn(GmOutboxEvent) + Send + Sync>>>,
}

/// Events from outbox processor
#[derive(Debug, Clone)]
pub enum GmOutboxEvent {
    /// Message send started
    Sending { local_message_uuid: String },
    /// Message sent successfully
    Sent { local_message_uuid: String, remote_message_id: String },
    /// Message send failed
    Failed { local_message_uuid: String, error: String, can_retry: bool },
    /// Max retries reached
    MaxRetriesReached { local_message_uuid: String },
}

impl GmOutboxProcessor {
    pub fn new(
        db: Arc<GmDatabase>,
        protocol: Arc<GmProtocolClient>,
        config: GmOutboxConfig,
    ) -> Self {
        Self {
            db,
            protocol,
            config,
            running: AtomicBool::new(false),
            send_notify: Notify::new(),
            on_status_callback: std::sync::RwLock::new(None),
        }
    }

    /// Set callback for send status updates
    pub fn set_status_callback<F>(&self, callback: F)
    where
        F: Fn(GmOutboxEvent) + Send + Sync + 'static,
    {
        *self.on_status_callback.write().unwrap() = Some(Box::new(callback));
    }

    fn emit_event(&self, event: GmOutboxEvent) {
        if let Some(ref callback) = *self.on_status_callback.read().unwrap() {
            callback(event);
        }
    }

    /// Queue a message for sending
    /// Returns the outbox item ID
    pub fn queue_message(
        &self,
        account_id: &str,
        remote_thread_id: &str,
        local_message_uuid: &str,
        text: &str,
    ) -> Result<i64, GmError> {
        log::info!(
            "GM: queuing message for thread {} (local: {})",
            remote_thread_id,
            local_message_uuid
        );

        let id = self.db.add_to_outbox(account_id, remote_thread_id, local_message_uuid, text)?;
        
        // Wake up processor
        self.send_notify.notify_one();
        
        Ok(id)
    }

    /// Start the outbox processor
    pub async fn start(&self) {
        if self.running.swap(true, Ordering::SeqCst) {
            log::warn!("GM outbox processor already running");
            return;
        }

        log::info!("Starting GM outbox processor");

        while self.running.load(Ordering::SeqCst) {
            // Process pending messages
            self.process_pending().await;

            // Wait for interval or new message notification
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(self.config.check_interval_ms)) => {},
                _ = self.send_notify.notified() => {},
            }
        }

        log::info!("GM outbox processor stopped");
    }

    /// Stop the outbox processor
    pub fn stop(&self) {
        log::info!("Stopping GM outbox processor");
        self.running.store(false, Ordering::SeqCst);
        self.send_notify.notify_one();
    }

    /// Process pending messages in the outbox
    async fn process_pending(&self) {
        // Check if authenticated
        if !self.protocol.is_authenticated() {
            return;
        }

        // Get pending items
        let pending = self.db.get_pending_outbox_items();
        
        if pending.is_empty() {
            return;
        }

        log::debug!("GM outbox: processing {} pending messages", pending.len());

        for item in pending.iter().take(self.config.max_concurrent_sends) {
            // Check max retries
            if item.attempts >= self.config.max_retry_attempts {
                log::warn!(
                    "GM: message {} exceeded max retries, marking failed",
                    item.local_message_uuid
                );
                
                self.emit_event(GmOutboxEvent::MaxRetriesReached {
                    local_message_uuid: item.local_message_uuid.clone(),
                });
                
                // Update status to permanently failed
                let _ = self.db.update_outbox_status(
                    item.id,
                    GmMessageStatus::Failed {
                        error: "Max retries exceeded".to_string(),
                    },
                    Some("Max retries exceeded".to_string()),
                );
                continue;
            }

            // Attempt to send
            self.emit_event(GmOutboxEvent::Sending {
                local_message_uuid: item.local_message_uuid.clone(),
            });

            // Update status to sending
            let _ = self.db.update_outbox_status(
                item.id,
                GmMessageStatus::Sending,
                None,
            );

            match self.protocol.send_message(&item.remote_thread_id, &item.text).await {
                Ok(remote_id) => {
                    log::info!(
                        "GM: message {} sent successfully (remote: {})",
                        item.local_message_uuid,
                        remote_id
                    );

                    // Update status and remove from outbox
                    let _ = self.db.update_outbox_status(
                        item.id,
                        GmMessageStatus::Sent,
                        None,
                    );
                    let _ = self.db.remove_from_outbox(item.id);

                    self.emit_event(GmOutboxEvent::Sent {
                        local_message_uuid: item.local_message_uuid.clone(),
                        remote_message_id: remote_id,
                    });
                }
                Err(e) => {
                    let error_msg = e.to_string();
                    log::error!(
                        "GM: failed to send message {}: {}",
                        item.local_message_uuid,
                        error_msg
                    );

                    let can_retry = !matches!(e, GmError::NotAuthenticated);
                    
                    // Update status with backoff
                    let _ = self.db.update_outbox_status(
                        item.id,
                        GmMessageStatus::Failed {
                            error: error_msg.clone(),
                        },
                        Some(error_msg.clone()),
                    );

                    self.emit_event(GmOutboxEvent::Failed {
                        local_message_uuid: item.local_message_uuid.clone(),
                        error: error_msg,
                        can_retry,
                    });

                    // If auth error, stop processing
                    if matches!(e, GmError::NotAuthenticated) {
                        break;
                    }
                }
            }
        }
    }

    /// Retry a specific message (e.g., user tapped "Retry")
    pub fn retry_message(&self, local_message_uuid: &str) -> Result<(), GmError> {
        log::info!("GM: manually retrying message {}", local_message_uuid);
        
        // Find the outbox item and reset its retry state
        let pending = self.db.get_pending_outbox_items();
        
        for item in pending {
            if item.local_message_uuid == local_message_uuid {
                // Reset attempts and next_retry_at by setting status to Queued
                self.db.update_outbox_status(
                    item.id,
                    GmMessageStatus::Queued,
                    None,
                )?;
                
                // Wake up processor
                self.send_notify.notify_one();
                return Ok(());
            }
        }

        Err(GmError::DatabaseError {
            message: format!("Message not found in outbox: {}", local_message_uuid),
        })
    }

    /// Cancel a queued message
    pub fn cancel_message(&self, local_message_uuid: &str) -> Result<(), GmError> {
        log::info!("GM: cancelling message {}", local_message_uuid);
        
        let pending = self.db.get_pending_outbox_items();
        
        for item in pending {
            if item.local_message_uuid == local_message_uuid {
                self.db.update_outbox_status(
                    item.id,
                    GmMessageStatus::Cancelled,
                    None,
                )?;
                self.db.remove_from_outbox(item.id)?;
                return Ok(());
            }
        }

        Err(GmError::DatabaseError {
            message: format!("Message not found in outbox: {}", local_message_uuid),
        })
    }

    /// Get outbox size
    pub fn get_queue_size(&self) -> usize {
        self.db.get_outbox_size()
    }

    /// Check if processor is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Send a message immediately (bypassing queue for UI responsiveness)
/// Used when user sends a message and we want instant feedback
pub async fn send_message_immediate(
    db: &GmDatabase,
    protocol: &GmProtocolClient,
    account_id: &str,
    remote_thread_id: &str,
    local_message_uuid: &str,
    text: &str,
) -> GmSendResult {
    log::info!(
        "GM: sending message immediately for thread {} (local: {})",
        remote_thread_id,
        local_message_uuid
    );

    // Try to send directly
    match protocol.send_message(remote_thread_id, text).await {
        Ok(remote_id) => {
            log::info!("GM: immediate send successful (remote: {})", remote_id);
            GmSendResult {
                success: true,
                remote_message_id: Some(remote_id),
                error: None,
                can_retry: false,
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            log::error!("GM: immediate send failed: {}", error_msg);

            let can_retry = !matches!(e, GmError::NotAuthenticated);

            // Queue for retry if retriable
            if can_retry {
                if let Err(queue_err) = db.add_to_outbox(
                    account_id,
                    remote_thread_id,
                    local_message_uuid,
                    text,
                ) {
                    log::error!("GM: failed to queue message for retry: {}", queue_err);
                }
            }

            GmSendResult {
                success: false,
                remote_message_id: None,
                error: Some(error_msg),
                can_retry,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outbox_config_defaults() {
        let config = GmOutboxConfig::default();
        assert_eq!(config.check_interval_ms, 5_000);
        assert_eq!(config.max_retry_attempts, 5);
    }
}
