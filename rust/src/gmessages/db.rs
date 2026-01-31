// Google Messages database/storage layer
// Provides idempotent upsert operations for thread/message mapping

use crate::gmessages::models::{
    GmMessage, GmMessageDirection, GmMessageStatus, GmOutboxItem, GmSyncCursor, GmThreadSummary,
    GmError,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// In-memory storage for Google Messages data
/// In production, this would be backed by ObjectBox or SQLite via Flutter
#[derive(Default)]
pub struct GmDatabase {
    /// Thread mappings: remote_thread_id -> GmThreadSummary
    threads: Arc<RwLock<HashMap<String, GmThreadSummary>>>,
    /// Message mappings: remote_message_id -> GmMessage
    messages: Arc<RwLock<HashMap<String, GmMessage>>>,
    /// Messages by thread: remote_thread_id -> Vec<remote_message_id>
    messages_by_thread: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// Outbox queue
    outbox: Arc<RwLock<Vec<GmOutboxItem>>>,
    /// Next outbox ID
    next_outbox_id: Arc<RwLock<i64>>,
    /// Sync cursors per thread
    cursors: Arc<RwLock<HashMap<String, GmSyncCursor>>>,
    /// Persistence callbacks
    save_callback: Arc<RwLock<Option<Box<dyn Fn(&str, &[u8]) -> Result<(), String> + Send + Sync>>>>,
    load_callback: Arc<RwLock<Option<Box<dyn Fn(&str) -> Result<Option<Vec<u8>>, String> + Send + Sync>>>>,
}

impl GmDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set persistence callbacks for Flutter integration
    pub fn set_persistence_callbacks<S, L>(&self, save: S, load: L)
    where
        S: Fn(&str, &[u8]) -> Result<(), String> + Send + Sync + 'static,
        L: Fn(&str) -> Result<Option<Vec<u8>>, String> + Send + Sync + 'static,
    {
        *self.save_callback.write().unwrap() = Some(Box::new(save));
        *self.load_callback.write().unwrap() = Some(Box::new(load));
    }

    // ============= THREAD OPERATIONS =============

    /// Upsert a thread - idempotent by remote_thread_id
    pub fn upsert_thread(&self, thread: GmThreadSummary) -> Result<(), GmError> {
        let mut threads = self.threads.write().unwrap();
        
        // Check if thread exists and update only if newer
        if let Some(existing) = threads.get(&thread.remote_thread_id) {
            if existing.updated_at >= thread.updated_at {
                // Existing is newer or same, skip update
                return Ok(());
            }
        }
        
        threads.insert(thread.remote_thread_id.clone(), thread);
        drop(threads);
        
        self.persist_threads()?;
        Ok(())
    }

    /// Get all threads
    pub fn get_threads(&self) -> Vec<GmThreadSummary> {
        let threads = self.threads.read().unwrap();
        let mut result: Vec<_> = threads.values().cloned().collect();
        // Sort by last message timestamp descending
        result.sort_by(|a, b| {
            b.last_message_timestamp
                .unwrap_or(0)
                .cmp(&a.last_message_timestamp.unwrap_or(0))
        });
        result
    }

    /// Get thread by remote ID
    pub fn get_thread(&self, remote_thread_id: &str) -> Option<GmThreadSummary> {
        self.threads.read().unwrap().get(remote_thread_id).cloned()
    }

    /// Get thread by local UUID
    pub fn get_thread_by_local_uuid(&self, local_uuid: &str) -> Option<GmThreadSummary> {
        self.threads
            .read()
            .unwrap()
            .values()
            .find(|t| t.local_thread_uuid.as_deref() == Some(local_uuid))
            .cloned()
    }

    /// Link a remote thread to a local UUID
    pub fn link_thread_to_local(&self, remote_thread_id: &str, local_uuid: &str) -> Result<(), GmError> {
        let mut threads = self.threads.write().unwrap();
        if let Some(thread) = threads.get_mut(remote_thread_id) {
            thread.local_thread_uuid = Some(local_uuid.to_string());
            drop(threads);
            self.persist_threads()?;
            return Ok(());
        }
        Err(GmError::DatabaseError {
            message: format!("Thread not found: {}", remote_thread_id),
        })
    }

    // ============= MESSAGE OPERATIONS =============

    /// Upsert a message - idempotent by remote_message_id
    pub fn upsert_message(&self, message: GmMessage) -> Result<(), GmError> {
        let mut messages = self.messages.write().unwrap();
        let mut by_thread = self.messages_by_thread.write().unwrap();

        // Insert/update message
        let remote_id = message.remote_message_id.clone();
        let thread_id = message.remote_thread_id.clone();
        
        // Only update if message doesn't exist or has newer status
        if let Some(existing) = messages.get(&remote_id) {
            // If existing message, only update status-related fields
            let mut updated = existing.clone();
            updated.status = message.status;
            updated.is_read = message.is_read;
            if message.local_message_uuid.is_some() {
                updated.local_message_uuid = message.local_message_uuid;
            }
            messages.insert(remote_id.clone(), updated);
        } else {
            messages.insert(remote_id.clone(), message);
            
            // Add to thread index
            by_thread
                .entry(thread_id)
                .or_insert_with(Vec::new)
                .push(remote_id);
        }

        drop(messages);
        drop(by_thread);
        
        self.persist_messages()?;
        Ok(())
    }

    /// Get messages for a thread with pagination
    pub fn get_messages(
        &self,
        remote_thread_id: &str,
        limit: usize,
        before_timestamp: Option<i64>,
    ) -> Vec<GmMessage> {
        let messages = self.messages.read().unwrap();
        let by_thread = self.messages_by_thread.read().unwrap();

        let Some(message_ids) = by_thread.get(remote_thread_id) else {
            return vec![];
        };

        let mut result: Vec<_> = message_ids
            .iter()
            .filter_map(|id| messages.get(id).cloned())
            .filter(|m| {
                before_timestamp
                    .map(|ts| m.timestamp < ts)
                    .unwrap_or(true)
            })
            .collect();

        // Sort by timestamp descending
        result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        result.truncate(limit);
        result
    }

    /// Get a single message by remote ID
    pub fn get_message(&self, remote_message_id: &str) -> Option<GmMessage> {
        self.messages.read().unwrap().get(remote_message_id).cloned()
    }

    /// Link a remote message to a local UUID
    pub fn link_message_to_local(
        &self,
        remote_message_id: &str,
        local_uuid: &str,
    ) -> Result<(), GmError> {
        let mut messages = self.messages.write().unwrap();
        if let Some(msg) = messages.get_mut(remote_message_id) {
            msg.local_message_uuid = Some(local_uuid.to_string());
            drop(messages);
            self.persist_messages()?;
            return Ok(());
        }
        Err(GmError::DatabaseError {
            message: format!("Message not found: {}", remote_message_id),
        })
    }

    // ============= OUTBOX OPERATIONS =============

    /// Add a message to the outbox queue
    pub fn add_to_outbox(
        &self,
        account_id: &str,
        remote_thread_id: &str,
        local_message_uuid: &str,
        text: &str,
    ) -> Result<i64, GmError> {
        let mut outbox = self.outbox.write().unwrap();
        let mut next_id = self.next_outbox_id.write().unwrap();

        let id = *next_id;
        *next_id += 1;

        let item = GmOutboxItem {
            id,
            account_id: account_id.to_string(),
            remote_thread_id: remote_thread_id.to_string(),
            local_message_uuid: local_message_uuid.to_string(),
            text: text.to_string(),
            status: GmMessageStatus::Queued,
            attempts: 0,
            next_retry_at: None,
            last_error: None,
            created_at: current_timestamp_millis(),
        };

        outbox.push(item);
        drop(outbox);
        drop(next_id);
        
        self.persist_outbox()?;
        Ok(id)
    }

    /// Get queued outbox items ready to send
    pub fn get_pending_outbox_items(&self) -> Vec<GmOutboxItem> {
        let now = current_timestamp_millis();
        let outbox = self.outbox.read().unwrap();
        
        outbox
            .iter()
            .filter(|item| {
                matches!(item.status, GmMessageStatus::Queued | GmMessageStatus::Failed { .. })
                    && item.next_retry_at.map(|t| t <= now).unwrap_or(true)
            })
            .cloned()
            .collect()
    }

    /// Update outbox item status
    pub fn update_outbox_status(
        &self,
        id: i64,
        status: GmMessageStatus,
        error: Option<String>,
    ) -> Result<(), GmError> {
        let mut outbox = self.outbox.write().unwrap();
        
        if let Some(item) = outbox.iter_mut().find(|i| i.id == id) {
            item.status = status.clone();
            item.last_error = error;
            
            if matches!(status, GmMessageStatus::Failed { .. }) {
                item.attempts += 1;
                // Exponential backoff: 10s, 30s, 60s, 120s, max 5min
                let delay_secs = match item.attempts {
                    1 => 10,
                    2 => 30,
                    3 => 60,
                    4 => 120,
                    _ => 300,
                };
                item.next_retry_at = Some(current_timestamp_millis() + (delay_secs * 1000));
            }
            
            drop(outbox);
            self.persist_outbox()?;
            return Ok(());
        }
        
        Err(GmError::DatabaseError {
            message: format!("Outbox item not found: {}", id),
        })
    }

    /// Remove sent/cancelled items from outbox
    pub fn remove_from_outbox(&self, id: i64) -> Result<(), GmError> {
        let mut outbox = self.outbox.write().unwrap();
        outbox.retain(|item| item.id != id);
        drop(outbox);
        self.persist_outbox()?;
        Ok(())
    }

    /// Get outbox size
    pub fn get_outbox_size(&self) -> usize {
        self.outbox.read().unwrap().len()
    }

    // ============= CURSOR OPERATIONS =============

    /// Update sync cursor for a thread
    pub fn update_cursor(&self, remote_thread_id: &str, cursor: &str) -> Result<(), GmError> {
        let mut cursors = self.cursors.write().unwrap();
        cursors.insert(
            remote_thread_id.to_string(),
            GmSyncCursor {
                remote_thread_id: remote_thread_id.to_string(),
                cursor: cursor.to_string(),
                synced_at: current_timestamp_millis(),
            },
        );
        drop(cursors);
        self.persist_cursors()?;
        Ok(())
    }

    /// Get sync cursor for a thread
    pub fn get_cursor(&self, remote_thread_id: &str) -> Option<String> {
        self.cursors
            .read()
            .unwrap()
            .get(remote_thread_id)
            .map(|c| c.cursor.clone())
    }

    // ============= PERSISTENCE =============

    fn persist_threads(&self) -> Result<(), GmError> {
        let save_cb = self.save_callback.read().unwrap();
        if let Some(ref save) = *save_cb {
            let threads = self.threads.read().unwrap();
            let data = serde_json::to_vec(&*threads).map_err(|e| GmError::DatabaseError {
                message: format!("Failed to serialize threads: {}", e),
            })?;
            save("gm_threads", &data).map_err(|e| GmError::DatabaseError { message: e })?;
        }
        Ok(())
    }

    fn persist_messages(&self) -> Result<(), GmError> {
        let save_cb = self.save_callback.read().unwrap();
        if let Some(ref save) = *save_cb {
            let messages = self.messages.read().unwrap();
            let by_thread = self.messages_by_thread.read().unwrap();
            
            let data = serde_json::to_vec(&(&*messages, &*by_thread)).map_err(|e| {
                GmError::DatabaseError {
                    message: format!("Failed to serialize messages: {}", e),
                }
            })?;
            save("gm_messages", &data).map_err(|e| GmError::DatabaseError { message: e })?;
        }
        Ok(())
    }

    fn persist_outbox(&self) -> Result<(), GmError> {
        let save_cb = self.save_callback.read().unwrap();
        if let Some(ref save) = *save_cb {
            let outbox = self.outbox.read().unwrap();
            let next_id = *self.next_outbox_id.read().unwrap();
            
            let data = serde_json::to_vec(&(&*outbox, next_id)).map_err(|e| {
                GmError::DatabaseError {
                    message: format!("Failed to serialize outbox: {}", e),
                }
            })?;
            save("gm_outbox", &data).map_err(|e| GmError::DatabaseError { message: e })?;
        }
        Ok(())
    }

    fn persist_cursors(&self) -> Result<(), GmError> {
        let save_cb = self.save_callback.read().unwrap();
        if let Some(ref save) = *save_cb {
            let cursors = self.cursors.read().unwrap();
            let data = serde_json::to_vec(&*cursors).map_err(|e| GmError::DatabaseError {
                message: format!("Failed to serialize cursors: {}", e),
            })?;
            save("gm_cursors", &data).map_err(|e| GmError::DatabaseError { message: e })?;
        }
        Ok(())
    }

    /// Load all data from persistent storage
    pub fn load_all(&self) -> Result<(), GmError> {
        self.load_threads()?;
        self.load_messages()?;
        self.load_outbox()?;
        self.load_cursors()?;
        Ok(())
    }

    fn load_threads(&self) -> Result<(), GmError> {
        let load_cb = self.load_callback.read().unwrap();
        if let Some(ref load) = *load_cb {
            if let Some(data) = load("gm_threads").map_err(|e| GmError::DatabaseError { message: e })? {
                let threads: HashMap<String, GmThreadSummary> =
                    serde_json::from_slice(&data).map_err(|e| GmError::DatabaseError {
                        message: format!("Failed to deserialize threads: {}", e),
                    })?;
                *self.threads.write().unwrap() = threads;
            }
        }
        Ok(())
    }

    fn load_messages(&self) -> Result<(), GmError> {
        let load_cb = self.load_callback.read().unwrap();
        if let Some(ref load) = *load_cb {
            if let Some(data) = load("gm_messages").map_err(|e| GmError::DatabaseError { message: e })? {
                let (messages, by_thread): (HashMap<String, GmMessage>, HashMap<String, Vec<String>>) =
                    serde_json::from_slice(&data).map_err(|e| GmError::DatabaseError {
                        message: format!("Failed to deserialize messages: {}", e),
                    })?;
                *self.messages.write().unwrap() = messages;
                *self.messages_by_thread.write().unwrap() = by_thread;
            }
        }
        Ok(())
    }

    fn load_outbox(&self) -> Result<(), GmError> {
        let load_cb = self.load_callback.read().unwrap();
        if let Some(ref load) = *load_cb {
            if let Some(data) = load("gm_outbox").map_err(|e| GmError::DatabaseError { message: e })? {
                let (outbox, next_id): (Vec<GmOutboxItem>, i64) =
                    serde_json::from_slice(&data).map_err(|e| GmError::DatabaseError {
                        message: format!("Failed to deserialize outbox: {}", e),
                    })?;
                *self.outbox.write().unwrap() = outbox;
                *self.next_outbox_id.write().unwrap() = next_id;
            }
        }
        Ok(())
    }

    fn load_cursors(&self) -> Result<(), GmError> {
        let load_cb = self.load_callback.read().unwrap();
        if let Some(ref load) = *load_cb {
            if let Some(data) = load("gm_cursors").map_err(|e| GmError::DatabaseError { message: e })? {
                let cursors: HashMap<String, GmSyncCursor> =
                    serde_json::from_slice(&data).map_err(|e| GmError::DatabaseError {
                        message: format!("Failed to deserialize cursors: {}", e),
                    })?;
                *self.cursors.write().unwrap() = cursors;
            }
        }
        Ok(())
    }

    /// Clear all data (for logout)
    pub fn clear_all(&self) -> Result<(), GmError> {
        *self.threads.write().unwrap() = HashMap::new();
        *self.messages.write().unwrap() = HashMap::new();
        *self.messages_by_thread.write().unwrap() = HashMap::new();
        *self.outbox.write().unwrap() = Vec::new();
        *self.next_outbox_id.write().unwrap() = 1;
        *self.cursors.write().unwrap() = HashMap::new();
        
        self.persist_threads()?;
        self.persist_messages()?;
        self.persist_outbox()?;
        self.persist_cursors()?;
        
        Ok(())
    }
}

fn current_timestamp_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_upsert_idempotent() {
        let db = GmDatabase::new();
        
        let thread1 = GmThreadSummary {
            remote_thread_id: "t1".to_string(),
            local_thread_uuid: None,
            display_name: Some("Test".to_string()),
            participants: vec!["+1234567890".to_string()],
            last_message_snippet: Some("Hello".to_string()),
            last_message_timestamp: Some(1000),
            unread_count: 0,
            is_group: false,
            updated_at: 1000,
        };
        
        db.upsert_thread(thread1.clone()).unwrap();
        assert_eq!(db.get_threads().len(), 1);
        
        // Same thread, same timestamp - should not duplicate
        db.upsert_thread(thread1.clone()).unwrap();
        assert_eq!(db.get_threads().len(), 1);
        
        // Same thread, older timestamp - should not update
        let mut older = thread1.clone();
        older.display_name = Some("Older".to_string());
        older.updated_at = 500;
        db.upsert_thread(older).unwrap();
        assert_eq!(db.get_thread("t1").unwrap().display_name, Some("Test".to_string()));
        
        // Same thread, newer timestamp - should update
        let mut newer = thread1.clone();
        newer.display_name = Some("Newer".to_string());
        newer.updated_at = 2000;
        db.upsert_thread(newer).unwrap();
        assert_eq!(db.get_thread("t1").unwrap().display_name, Some("Newer".to_string()));
    }

    #[test]
    fn test_message_upsert_idempotent() {
        let db = GmDatabase::new();
        
        let msg = GmMessage {
            remote_message_id: "m1".to_string(),
            remote_thread_id: "t1".to_string(),
            local_message_uuid: None,
            text: Some("Hello".to_string()),
            timestamp: 1000,
            direction: GmMessageDirection::Incoming,
            status: GmMessageStatus::Sent,
            sender: Some("+1234567890".to_string()),
            is_read: false,
        };
        
        db.upsert_message(msg.clone()).unwrap();
        let messages = db.get_messages("t1", 10, None);
        assert_eq!(messages.len(), 1);
        
        // Same message again - should not duplicate
        db.upsert_message(msg.clone()).unwrap();
        let messages = db.get_messages("t1", 10, None);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_outbox_retry_backoff() {
        let db = GmDatabase::new();
        
        let id = db.add_to_outbox("acc1", "t1", "local-1", "Hello").unwrap();
        
        // First failure
        db.update_outbox_status(id, GmMessageStatus::Failed { error: "Network".to_string() }, Some("Network".to_string())).unwrap();
        
        let items = db.get_pending_outbox_items();
        // Should not be immediately available due to backoff
        assert!(items.is_empty() || items[0].next_retry_at.unwrap() > current_timestamp_millis());
    }
}
