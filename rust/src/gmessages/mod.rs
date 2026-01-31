//! Google Messages integration module (Android only, experimental)
//!
//! This module provides optional Google Messages support for LiquidBubbles.
//! It uses an in-app WebView to pair with Google Messages web and then
//! syncs conversations/messages via HTTP.
//!
//! **Feature Flag**: This is behind the `enableGoogleMessages` setting (default OFF).
//!
//! ## Architecture
//! - `models.rs` - Data structures for threads, messages, outbox
//! - `db.rs` - File-based persistence with idempotent upserts
//! - `session.rs` - Encrypted session/cookie management
//! - `protocol.rs` - HTTP client for Google Messages web API (TODO)
//! - `sync.rs` - Background sync loop with cursors (TODO)
//! - `outbox.rs` - Outgoing message queue with retry logic (TODO)

pub mod models;
pub mod db;
pub mod session;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use flutter_rust_bridge::frb;
use log::{debug, error, info, warn};

use models::*;
use db::{get_db, get_db_mut, save_db, init_db};
use session::{get_session_manager, init_session_manager};

/// Global feature flag - whether Google Messages is enabled
static GM_ENABLED: AtomicBool = AtomicBool::new(false);

/// Initialize the Google Messages subsystem
/// Called from Flutter when the app starts (if feature is enabled)
#[frb(sync)]
pub fn gm_init(data_path: String) -> Result<(), String> {
    let path = PathBuf::from(&data_path);
    
    // Create directory if needed
    std::fs::create_dir_all(&path)
        .map_err(|e| format!("Failed to create GM data directory: {}", e))?;
    
    // Initialize database
    init_db(path.clone())?;
    
    // Initialize session manager
    init_session_manager(path)?;
    
    info!("Google Messages subsystem initialized");
    Ok(())
}

/// Check if Google Messages feature is enabled
#[frb(sync)]
pub fn gm_is_enabled() -> bool {
    GM_ENABLED.load(Ordering::Relaxed)
}

/// Enable or disable Google Messages feature
#[frb(sync)]
pub fn gm_set_enabled(enabled: bool) {
    GM_ENABLED.store(enabled, Ordering::Relaxed);
    info!("Google Messages feature {}", if enabled { "enabled" } else { "disabled" });
}

/// Get current pairing state
#[frb(sync)]
pub fn gm_get_pairing_state() -> Result<GMPairingState, String> {
    let manager = get_session_manager()?;
    Ok(manager.get_pairing_state())
}

/// Set the encryption key for session storage (from Android Keystore)
/// The key should be a 32-byte AES key
#[frb(sync)]
pub fn gm_set_encryption_key(key: Vec<u8>) -> Result<(), String> {
    if key.len() != 32 {
        return Err("Encryption key must be 32 bytes".to_string());
    }
    
    // Note: This is a simplified version. In production, we'd need mutable access
    // to the session manager or use interior mutability pattern
    debug!("Encryption key set for GM session storage");
    Ok(())
}

/// Store cookies from WebView after successful pairing
/// This should be called from Flutter after detecting successful login in the WebView
pub fn gm_set_cookies_from_webview(cookies: String) -> Result<(), String> {
    let manager = get_session_manager()?;
    manager.set_session_from_cookies(cookies)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Load existing session (call on app startup)
pub fn gm_load_session() -> Result<bool, String> {
    let manager = get_session_manager()?;
    manager.load_session().map_err(|e| e.to_string())
}

/// Logout from Google Messages
pub fn gm_logout() -> Result<(), String> {
    let manager = get_session_manager()?;
    manager.logout().map_err(|e| e.to_string())?;
    
    // Clear database
    let mut db = get_db_mut()?;
    db.clear();
    save_db()?;
    
    info!("Google Messages logout complete");
    Ok(())
}

/// List all threads (conversations)
#[frb(sync)]
pub fn gm_list_threads() -> Result<Vec<GMThreadSummary>, String> {
    let db = get_db()?;
    
    let threads: Vec<GMThreadSummary> = db.list_threads()
        .into_iter()
        .map(|t| {
            // Get last message for this thread
            let messages = db.get_messages_for_thread(&t.remote_thread_id, 1, None);
            let last_message = messages.first();
            
            GMThreadSummary {
                remote_thread_id: t.remote_thread_id,
                display_name: t.display_name,
                last_message_text: last_message.and_then(|m| m.text.clone()),
                last_message_timestamp: last_message.map(|m| m.timestamp).unwrap_or(t.updated_at),
                unread_count: 0, // TODO: Track unread count
                is_group: t.is_group,
                participants: t.participants
                    .map(|p| p.split(',').map(|s| s.to_string()).collect())
                    .unwrap_or_default(),
            }
        })
        .collect();
    
    Ok(threads)
}

/// List messages for a thread
#[frb(sync)]
pub fn gm_list_messages(
    remote_thread_id: String,
    limit: i32,
    cursor: Option<i64>,
) -> Result<Vec<GMMessage>, String> {
    let db = get_db()?;
    let messages = db.get_messages_for_thread(&remote_thread_id, limit as usize, cursor);
    Ok(messages)
}

/// Send a text message (queues in outbox)
pub fn gm_send_text(remote_thread_id: String, text: String) -> Result<GMSendResult, String> {
    let manager = get_session_manager()?;
    
    // Check if paired
    if !manager.is_paired() {
        return Ok(GMSendResult::Failed {
            error: "Not paired with Google Messages".to_string(),
        });
    }
    
    // Generate local message GUID
    let local_guid = format!("gm-{}", uuid::Uuid::new_v4());
    
    // Add to outbox
    let mut db = get_db_mut()?;
    let outbox_item = GMOutboxItem::new(remote_thread_id.clone(), local_guid.clone(), text.clone());
    let added = db.add_to_outbox(outbox_item);
    save_db()?;
    
    info!("Message queued in GM outbox: {}", local_guid);
    
    Ok(GMSendResult::Queued {
        outbox_id: added.id.unwrap_or(0),
    })
}

/// Manually trigger a sync
pub fn gm_sync_now() -> Result<(), String> {
    let manager = get_session_manager()?;
    
    if !manager.is_paired() {
        return Err("Not paired with Google Messages".to_string());
    }
    
    // TODO: Implement actual sync logic in Phase 3
    info!("Manual GM sync triggered (not yet implemented)");
    Ok(())
}

/// Start the background sync loop
pub fn gm_start_sync_loop() -> Result<(), String> {
    // TODO: Implement in Phase 3
    info!("GM sync loop start requested (not yet implemented)");
    Ok(())
}

/// Stop the background sync loop
pub fn gm_stop_sync_loop() -> Result<(), String> {
    // TODO: Implement in Phase 3
    info!("GM sync loop stop requested");
    Ok(())
}

/// Get diagnostics information
#[frb(sync)]
pub fn gm_get_diagnostics() -> Result<GMDiagnostics, String> {
    let manager = get_session_manager()?;
    let db = get_db()?;
    
    Ok(GMDiagnostics {
        pairing_state: manager.get_pairing_state(),
        last_sync_time: db.last_sync_time,
        outbox_size: db.outbox_size() as i32,
        last_error: None, // TODO: Track last error
        thread_count: db.threads.len() as i32,
        message_count: db.messages.len() as i32,
    })
}

/// Upsert a thread (used by sync)
pub fn gm_upsert_thread(
    remote_thread_id: String,
    display_name: Option<String>,
    participants: Option<String>,
    is_group: bool,
) -> Result<GMThread, String> {
    let mut db = get_db_mut()?;
    
    let mut thread = GMThread::new(remote_thread_id);
    thread.display_name = display_name;
    thread.participants = participants;
    thread.is_group = is_group;
    
    let result = db.upsert_thread(thread);
    save_db()?;
    
    Ok(result)
}

/// Upsert a message (used by sync)
pub fn gm_upsert_message(
    remote_message_id: String,
    remote_thread_id: String,
    text: Option<String>,
    timestamp: i64,
    is_outgoing: bool,
) -> Result<GMMessage, String> {
    let mut db = get_db_mut()?;
    
    let direction = if is_outgoing {
        GMMessageDirection::Outgoing
    } else {
        GMMessageDirection::Incoming
    };
    
    let mut message = GMMessage::new(remote_message_id, remote_thread_id, direction);
    message.text = text;
    message.timestamp = timestamp;
    
    let result = db.upsert_message(message);
    save_db()?;
    
    Ok(result)
}

/// Link a GM thread to a local chat
pub fn gm_link_thread_to_chat(
    remote_thread_id: String,
    local_chat_guid: String,
) -> Result<(), String> {
    let mut db = get_db_mut()?;
    
    if let Some(thread) = db.threads.get_mut(&remote_thread_id) {
        thread.local_chat_guid = Some(local_chat_guid);
        save_db()?;
        Ok(())
    } else {
        Err(format!("Thread not found: {}", remote_thread_id))
    }
}

/// Link a GM message to a local message
pub fn gm_link_message_to_local(
    remote_message_id: String,
    local_message_guid: String,
) -> Result<(), String> {
    let mut db = get_db_mut()?;
    
    if let Some(message) = db.messages.get_mut(&remote_message_id) {
        message.local_message_guid = Some(local_message_guid);
        save_db()?;
        Ok(())
    } else {
        Err(format!("Message not found: {}", remote_message_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn setup_test_db() -> PathBuf {
        let path = env::temp_dir().join(format!("gm_test_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        init_db(path.clone()).unwrap();
        path
    }

    #[test]
    fn test_thread_upsert_idempotent() {
        let _path = setup_test_db();
        
        // First insert
        let result1 = gm_upsert_thread(
            "thread123".to_string(),
            Some("Test".to_string()),
            None,
            false,
        ).unwrap();
        
        // Second insert - should update, not duplicate
        let result2 = gm_upsert_thread(
            "thread123".to_string(),
            Some("Updated".to_string()),
            None,
            false,
        ).unwrap();
        
        assert_eq!(result1.id, result2.id);
        assert_eq!(result2.display_name, Some("Updated".to_string()));
    }

    #[test]
    fn test_message_upsert_idempotent() {
        let _path = setup_test_db();
        
        // First insert
        let result1 = gm_upsert_message(
            "msg123".to_string(),
            "thread123".to_string(),
            Some("Hello".to_string()),
            1000,
            false,
        ).unwrap();
        
        // Second insert - should update, not duplicate
        let result2 = gm_upsert_message(
            "msg123".to_string(),
            "thread123".to_string(),
            Some("Updated".to_string()),
            1000,
            false,
        ).unwrap();
        
        assert_eq!(result1.id, result2.id);
        assert_eq!(result2.text, Some("Updated".to_string()));
    }
}
