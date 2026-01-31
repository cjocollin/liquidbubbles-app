//! Google Messages database operations
//! 
//! Simple file-based persistence for Google Messages thread/message mappings.
//! Uses JSON files for storage to keep things simple and avoid additional dependencies.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use serde::{Deserialize, Serialize};
use log::{debug, error, info, warn};

use super::models::{GMThread, GMMessage, GMOutboxItem, GMMessageStatus};

/// In-memory + file-backed database for Google Messages data
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct GMDatabase {
    /// Thread mappings indexed by remote_thread_id
    pub threads: HashMap<String, GMThread>,
    /// Message mappings indexed by remote_message_id
    pub messages: HashMap<String, GMMessage>,
    /// Outbox items indexed by local_message_guid
    pub outbox: HashMap<String, GMOutboxItem>,
    /// Auto-increment counters
    pub next_thread_id: i64,
    pub next_message_id: i64,
    pub next_outbox_id: i64,
    /// Last sync timestamp
    pub last_sync_time: Option<i64>,
}

impl GMDatabase {
    /// Create a new empty database
    pub fn new() -> Self {
        Self::default()
    }

    /// Load database from file, or create new if doesn't exist
    pub fn load(path: &PathBuf) -> Result<Self, String> {
        let db_file = path.join("gmessages_db.json");
        
        if db_file.exists() {
            let content = fs::read_to_string(&db_file)
                .map_err(|e| format!("Failed to read database file: {}", e))?;
            
            let db: GMDatabase = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse database: {}", e))?;
            
            info!("Loaded GM database with {} threads, {} messages", 
                  db.threads.len(), db.messages.len());
            
            Ok(db)
        } else {
            info!("Creating new GM database");
            Ok(Self::new())
        }
    }

    /// Save database to file
    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        let db_file = path.join("gmessages_db.json");
        
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize database: {}", e))?;
        
        fs::write(&db_file, content)
            .map_err(|e| format!("Failed to write database file: {}", e))?;
        
        debug!("Saved GM database");
        Ok(())
    }

    // ==================== Thread Operations ====================

    /// Upsert a thread by remote_thread_id (idempotent)
    pub fn upsert_thread(&mut self, thread: GMThread) -> GMThread {
        let remote_id = thread.remote_thread_id.clone();
        
        if let Some(existing) = self.threads.get_mut(&remote_id) {
            // Update existing thread, preserve ID
            existing.display_name = thread.display_name.or(existing.display_name.clone());
            existing.participants = thread.participants.or(existing.participants.clone());
            existing.local_chat_guid = thread.local_chat_guid.or(existing.local_chat_guid.clone());
            existing.cursor = thread.cursor.or(existing.cursor.clone());
            existing.is_group = thread.is_group;
            existing.updated_at = chrono_timestamp_ms();
            existing.clone()
        } else {
            // Insert new thread
            let mut new_thread = thread;
            new_thread.id = Some(self.next_thread_id);
            self.next_thread_id += 1;
            self.threads.insert(remote_id, new_thread.clone());
            new_thread
        }
    }

    /// Get thread by remote ID
    pub fn get_thread(&self, remote_thread_id: &str) -> Option<&GMThread> {
        self.threads.get(remote_thread_id)
    }

    /// Get thread by local chat GUID
    pub fn get_thread_by_local_guid(&self, local_chat_guid: &str) -> Option<&GMThread> {
        self.threads.values()
            .find(|t| t.local_chat_guid.as_deref() == Some(local_chat_guid))
    }

    /// List all threads sorted by updated_at descending
    pub fn list_threads(&self) -> Vec<GMThread> {
        let mut threads: Vec<_> = self.threads.values().cloned().collect();
        threads.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        threads
    }

    // ==================== Message Operations ====================

    /// Upsert a message by remote_message_id (idempotent)
    pub fn upsert_message(&mut self, message: GMMessage) -> GMMessage {
        let remote_id = message.remote_message_id.clone();
        
        if let Some(existing) = self.messages.get_mut(&remote_id) {
            // Update existing message, preserve ID
            existing.local_message_guid = message.local_message_guid.or(existing.local_message_guid.clone());
            existing.status = message.status;
            existing.text = message.text.or(existing.text.clone());
            existing.clone()
        } else {
            // Insert new message
            let mut new_message = message;
            new_message.id = Some(self.next_message_id);
            self.next_message_id += 1;
            self.messages.insert(remote_id, new_message.clone());
            new_message
        }
    }

    /// Get message by remote ID
    pub fn get_message(&self, remote_message_id: &str) -> Option<&GMMessage> {
        self.messages.get(remote_message_id)
    }

    /// Get messages for a thread, sorted by timestamp descending
    pub fn get_messages_for_thread(&self, remote_thread_id: &str, limit: usize, cursor: Option<i64>) -> Vec<GMMessage> {
        let mut messages: Vec<_> = self.messages.values()
            .filter(|m| m.remote_thread_id == remote_thread_id)
            .filter(|m| cursor.map_or(true, |c| m.timestamp < c))
            .cloned()
            .collect();
        
        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        messages.truncate(limit);
        messages
    }

    // ==================== Outbox Operations ====================

    /// Add item to outbox
    pub fn add_to_outbox(&mut self, item: GMOutboxItem) -> GMOutboxItem {
        let mut new_item = item;
        new_item.id = Some(self.next_outbox_id);
        self.next_outbox_id += 1;
        self.outbox.insert(new_item.local_message_guid.clone(), new_item.clone());
        new_item
    }

    /// Update outbox item status
    pub fn update_outbox_status(
        &mut self, 
        local_message_guid: &str, 
        status: GMMessageStatus,
        error: Option<String>,
    ) -> Option<GMOutboxItem> {
        if let Some(item) = self.outbox.get_mut(local_message_guid) {
            item.status = status;
            item.last_error = error;
            item.attempts += 1;
            
            if status == GMMessageStatus::Failed {
                // Calculate next retry with exponential backoff (max 5 attempts)
                item.next_retry_at = item.calculate_next_retry(5);
            } else {
                item.next_retry_at = None;
            }
            
            Some(item.clone())
        } else {
            None
        }
    }

    /// Remove item from outbox (after successful send)
    pub fn remove_from_outbox(&mut self, local_message_guid: &str) -> Option<GMOutboxItem> {
        self.outbox.remove(local_message_guid)
    }

    /// Get queued outbox items ready for sending
    pub fn get_pending_outbox_items(&self) -> Vec<GMOutboxItem> {
        let now = chrono_timestamp_ms();
        
        self.outbox.values()
            .filter(|item| {
                match item.status {
                    GMMessageStatus::Queued => true,
                    GMMessageStatus::Failed => {
                        // Check if retry time has passed
                        item.next_retry_at.map_or(false, |t| t <= now)
                    }
                    _ => false,
                }
            })
            .cloned()
            .collect()
    }

    /// Get total outbox size
    pub fn outbox_size(&self) -> usize {
        self.outbox.len()
    }

    /// Clear the entire database (for logout/reset)
    pub fn clear(&mut self) {
        self.threads.clear();
        self.messages.clear();
        self.outbox.clear();
        self.next_thread_id = 0;
        self.next_message_id = 0;
        self.next_outbox_id = 0;
        self.last_sync_time = None;
    }
}

/// Get current timestamp in milliseconds
fn chrono_timestamp_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Global database instance with thread-safe access
pub static GM_DB: std::sync::OnceLock<RwLock<GMDatabase>> = std::sync::OnceLock::new();
/// Path where the database is stored
pub static GM_DB_PATH: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Initialize the global database
pub fn init_db(path: PathBuf) -> Result<(), String> {
    let db = GMDatabase::load(&path)?;
    
    GM_DB_PATH.set(path.clone()).map_err(|_| "DB path already set")?;
    GM_DB.set(RwLock::new(db)).map_err(|_| "DB already initialized")?;
    
    Ok(())
}

/// Get a read reference to the database
pub fn get_db() -> Result<std::sync::RwLockReadGuard<'static, GMDatabase>, String> {
    GM_DB.get()
        .ok_or_else(|| "GM database not initialized".to_string())?
        .read()
        .map_err(|e| format!("Failed to acquire read lock: {}", e))
}

/// Get a write reference to the database
pub fn get_db_mut() -> Result<std::sync::RwLockWriteGuard<'static, GMDatabase>, String> {
    GM_DB.get()
        .ok_or_else(|| "GM database not initialized".to_string())?
        .write()
        .map_err(|e| format!("Failed to acquire write lock: {}", e))
}

/// Save the database to disk
pub fn save_db() -> Result<(), String> {
    let path = GM_DB_PATH.get()
        .ok_or_else(|| "GM database path not set".to_string())?;
    
    let db = get_db()?;
    db.save(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gmessages::models::GMMessageDirection;

    #[test]
    fn test_thread_upsert_idempotent() {
        let mut db = GMDatabase::new();
        
        // First insert
        let thread1 = GMThread::new("thread123".to_string());
        let inserted = db.upsert_thread(thread1);
        assert_eq!(inserted.id, Some(0));
        
        // Second insert with same remote ID should update, not create new
        let mut thread2 = GMThread::new("thread123".to_string());
        thread2.display_name = Some("Test Group".to_string());
        let updated = db.upsert_thread(thread2);
        
        assert_eq!(updated.id, Some(0)); // Same ID
        assert_eq!(updated.display_name, Some("Test Group".to_string()));
        assert_eq!(db.threads.len(), 1); // Still only one thread
    }

    #[test]
    fn test_message_upsert_idempotent() {
        let mut db = GMDatabase::new();
        
        // First insert
        let msg1 = GMMessage::new(
            "msg123".to_string(),
            "thread123".to_string(),
            GMMessageDirection::Incoming,
        );
        let inserted = db.upsert_message(msg1);
        assert_eq!(inserted.id, Some(0));
        
        // Second insert with same remote ID should update
        let mut msg2 = GMMessage::new(
            "msg123".to_string(),
            "thread123".to_string(),
            GMMessageDirection::Incoming,
        );
        msg2.text = Some("Hello World".to_string());
        let updated = db.upsert_message(msg2);
        
        assert_eq!(updated.id, Some(0));
        assert_eq!(updated.text, Some("Hello World".to_string()));
        assert_eq!(db.messages.len(), 1);
    }

    #[test]
    fn test_outbox_operations() {
        let mut db = GMDatabase::new();
        
        // Add to outbox
        let item = GMOutboxItem::new(
            "thread123".to_string(),
            "local_msg_1".to_string(),
            "Hello".to_string(),
        );
        let added = db.add_to_outbox(item);
        assert_eq!(added.id, Some(0));
        assert_eq!(added.status, GMMessageStatus::Queued);
        
        // Get pending items
        let pending = db.get_pending_outbox_items();
        assert_eq!(pending.len(), 1);
        
        // Update status to sent and remove
        db.remove_from_outbox("local_msg_1");
        assert_eq!(db.outbox_size(), 0);
    }
}
