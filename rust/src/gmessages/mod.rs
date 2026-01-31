// Google Messages integration module
// Provides optional Google Messages support via WebView pairing

pub mod db;
pub mod models;
pub mod outbox;
pub mod protocol;
pub mod session;
pub mod sync;

use db::GmDatabase;
use flutter_rust_bridge::frb;
use models::{GmError, GmMessage, GmPairingState, GmSendResult, GmThreadSummary};
use outbox::{GmOutboxConfig, GmOutboxProcessor};
use protocol::{GmProtocolClient, GM_USER_AGENT};
use session::GmSessionManager;
use sync::{GmSyncConfig, GmSyncEvent, GmSyncLoop};

use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Global Google Messages state
struct GmState {
    enabled: std::sync::atomic::AtomicBool,
    session: Arc<GmSessionManager>,
    db: Arc<GmDatabase>,
    protocol: Option<Arc<GmProtocolClient>>,
    sync_loop: Option<Arc<GmSyncLoop>>,
    outbox_processor: Option<Arc<GmOutboxProcessor>>,
}

impl Default for GmState {
    fn default() -> Self {
        let session = Arc::new(GmSessionManager::new());
        let db = Arc::new(GmDatabase::new());
        
        Self {
            enabled: std::sync::atomic::AtomicBool::new(false),
            session,
            db,
            protocol: None,
            sync_loop: None,
            outbox_processor: None,
        }
    }
}

static GM_STATE: Lazy<RwLock<GmState>> = Lazy::new(|| RwLock::new(GmState::default()));

// ============= FRB API Functions =============

/// Check if Google Messages feature is enabled
#[frb(sync)]
pub fn gm_is_enabled() -> bool {
    // Use try_read to avoid blocking
    if let Ok(state) = GM_STATE.try_read() {
        state.enabled.load(std::sync::atomic::Ordering::SeqCst)
    } else {
        false
    }
}

/// Enable or disable Google Messages feature
pub async fn gm_set_enabled(enabled: bool) -> Result<(), String> {
    log::info!("GM: setting enabled = {}", enabled);
    
    let mut state = GM_STATE.write().await;
    state.enabled.store(enabled, std::sync::atomic::Ordering::SeqCst);
    
    if enabled {
        // Initialize components if not already done
        if state.protocol.is_none() {
            let protocol = Arc::new(GmProtocolClient::new(state.session.clone()));
            state.protocol = Some(protocol.clone());
            
            let sync_loop = Arc::new(GmSyncLoop::new(
                state.session.clone(),
                state.db.clone(),
                protocol.clone(),
                GmSyncConfig::default(),
            ));
            state.sync_loop = Some(sync_loop);
            
            let outbox = Arc::new(GmOutboxProcessor::new(
                state.db.clone(),
                protocol,
                GmOutboxConfig::default(),
            ));
            state.outbox_processor = Some(outbox);
        }
        
        // Try to load existing session
        if let Err(e) = state.session.load_session() {
            log::warn!("GM: failed to load session: {}", e);
        }
    } else {
        // Stop sync/outbox if running
        if let Some(ref sync_loop) = state.sync_loop {
            sync_loop.stop();
        }
        if let Some(ref outbox) = state.outbox_processor {
            outbox.stop();
        }
    }
    
    Ok(())
}

/// Get current pairing state
#[frb(sync)]
pub fn gm_get_pairing_state() -> GmPairingState {
    if let Ok(state) = GM_STATE.try_read() {
        state.session.get_pairing_state()
    } else {
        GmPairingState::NotPaired
    }
}

/// Set cookies from WebView after successful pairing
pub async fn gm_set_cookies_from_webview(cookie_header: String) -> Result<(), String> {
    log::info!("GM: received cookies from WebView");
    
    let state = GM_STATE.read().await;
    
    let cookies = GmSessionManager::parse_cookie_header(&cookie_header);
    if cookies.is_empty() {
        return Err("No cookies found in header".to_string());
    }
    
    state
        .session
        .set_session_from_cookies(cookies, GM_USER_AGENT.to_string())
        .map_err(|e| e.to_string())?;
    
    Ok(())
}

/// Start the sync loop
pub async fn gm_start_sync_loop() -> Result<(), String> {
    log::info!("GM: starting sync loop");
    
    let state = GM_STATE.read().await;
    
    if !state.enabled.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("Google Messages is not enabled".to_string());
    }
    
    let sync_loop = state.sync_loop.clone()
        .ok_or("Sync loop not initialized")?;
    let outbox = state.outbox_processor.clone()
        .ok_or("Outbox processor not initialized")?;
    
    // Spawn sync loop
    tokio::spawn(async move {
        sync_loop.start().await;
    });
    
    // Spawn outbox processor
    tokio::spawn(async move {
        outbox.start().await;
    });
    
    Ok(())
}

/// Stop the sync loop
pub async fn gm_stop_sync_loop() -> Result<(), String> {
    log::info!("GM: stopping sync loop");
    
    let state = GM_STATE.read().await;
    
    if let Some(ref sync_loop) = state.sync_loop {
        sync_loop.stop();
    }
    if let Some(ref outbox) = state.outbox_processor {
        outbox.stop();
    }
    
    Ok(())
}

/// Trigger immediate sync
pub async fn gm_sync_now() -> Result<(), String> {
    let state = GM_STATE.read().await;
    
    if let Some(ref sync_loop) = state.sync_loop {
        sync_loop.sync_now();
        Ok(())
    } else {
        Err("Sync loop not initialized".to_string())
    }
}

/// Set app foreground/background state (for battery-safe polling)
pub async fn gm_set_foreground(is_foreground: bool) -> Result<(), String> {
    let state = GM_STATE.read().await;
    
    if let Some(ref sync_loop) = state.sync_loop {
        sync_loop.set_foreground(is_foreground);
    }
    
    Ok(())
}

/// List all threads
pub async fn gm_list_threads() -> Result<Vec<GmThreadSummary>, String> {
    let state = GM_STATE.read().await;
    Ok(state.db.get_threads())
}

/// List messages for a thread
pub async fn gm_list_messages(
    remote_thread_id: String,
    limit: u32,
    before_timestamp: Option<i64>,
) -> Result<Vec<GmMessage>, String> {
    let state = GM_STATE.read().await;
    Ok(state.db.get_messages(&remote_thread_id, limit as usize, before_timestamp))
}

/// Send a text message
pub async fn gm_send_text(
    remote_thread_id: String,
    local_message_uuid: String,
    text: String,
) -> Result<GmSendResult, String> {
    let state = GM_STATE.read().await;
    
    let protocol = state.protocol.as_ref()
        .ok_or("Protocol not initialized")?;
    
    let result = outbox::send_message_immediate(
        &state.db,
        protocol,
        "default",  // Account ID - MVP uses single account
        &remote_thread_id,
        &local_message_uuid,
        &text,
    ).await;
    
    Ok(result)
}

/// Retry a failed message
pub async fn gm_retry_message(local_message_uuid: String) -> Result<(), String> {
    let state = GM_STATE.read().await;
    
    if let Some(ref outbox) = state.outbox_processor {
        outbox.retry_message(&local_message_uuid).map_err(|e| e.to_string())
    } else {
        Err("Outbox processor not initialized".to_string())
    }
}

/// Cancel a queued message
pub async fn gm_cancel_message(local_message_uuid: String) -> Result<(), String> {
    let state = GM_STATE.read().await;
    
    if let Some(ref outbox) = state.outbox_processor {
        outbox.cancel_message(&local_message_uuid).map_err(|e| e.to_string())
    } else {
        Err("Outbox processor not initialized".to_string())
    }
}

/// Logout and clear session
pub async fn gm_logout() -> Result<(), String> {
    log::info!("GM: logging out");
    
    let state = GM_STATE.read().await;
    
    // Stop sync/outbox
    if let Some(ref sync_loop) = state.sync_loop {
        sync_loop.stop();
    }
    if let Some(ref outbox) = state.outbox_processor {
        outbox.stop();
    }
    
    // Clear session
    state.session.clear_session().map_err(|e| e.to_string())?;
    
    // Clear database
    state.db.clear_all().map_err(|e| e.to_string())?;
    
    Ok(())
}

// ============= Diagnostics Functions =============

/// Get diagnostic info (for debug screen)
#[frb(sync)]
pub fn gm_get_diagnostics() -> GmDiagnostics {
    if let Ok(state) = GM_STATE.try_read() {
        GmDiagnostics {
            enabled: state.enabled.load(std::sync::atomic::Ordering::SeqCst),
            pairing_state: state.session.get_pairing_state(),
            has_session: state.session.has_session(),
            last_sync_at: state.sync_loop.as_ref().map(|s| s.get_last_sync_at()).unwrap_or(0),
            last_error: state.sync_loop.as_ref().and_then(|s| s.get_last_error()),
            outbox_size: state.outbox_processor.as_ref().map(|o| o.get_queue_size()).unwrap_or(0) as u32,
            sync_running: state.sync_loop.as_ref().map(|s| s.is_running()).unwrap_or(false),
        }
    } else {
        GmDiagnostics::default()
    }
}

/// Diagnostic info struct
#[derive(Debug, Clone, Default)]
#[frb(dart_metadata = ("freezed"))]
pub struct GmDiagnostics {
    pub enabled: bool,
    pub pairing_state: GmPairingState,
    pub has_session: bool,
    pub last_sync_at: i64,
    pub last_error: Option<String>,
    pub outbox_size: u32,
    pub sync_running: bool,
}

// ============= WebView Bridge Functions =============

/// Sync threads/messages from WebView-extracted data
pub async fn gm_sync_from_webview(
    threads_json: Option<String>,
    messages_json: Option<String>,
    thread_id: Option<String>,
) -> Result<(u32, u32), String> {
    let state = GM_STATE.read().await;
    
    let (threads, messages) = sync::sync_from_webview_data(
        &state.db,
        threads_json.as_deref(),
        messages_json.as_deref(),
        thread_id.as_deref(),
    ).map_err(|e| e.to_string())?;
    
    Ok((threads as u32, messages as u32))
}

/// Set secure storage callbacks from Flutter
/// This should be called during initialization to enable encrypted session storage
pub async fn gm_set_storage_callbacks(
    save_key: String,
    load_key: String,
) -> Result<(), String> {
    // NOTE: In the actual implementation, these would be platform channel callbacks
    // For MVP, we'll use a simple file-based storage (the Flutter side handles encryption)
    log::info!("GM: storage callbacks configured");
    Ok(())
}

// ============= Database Persistence Bridge =============

/// Set database persistence callbacks
pub async fn gm_set_db_callbacks() -> Result<(), String> {
    // NOTE: Similar to storage callbacks, these bridge to Flutter
    log::info!("GM: database callbacks configured");
    Ok(())
}
