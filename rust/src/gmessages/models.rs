//! Google Messages data models
//! 
//! Data structures for Google Messages integration (Android only, experimental).
//! These models are used to track thread/message mappings between Google Messages
//! and the local database.

use serde::{Deserialize, Serialize};
use flutter_rust_bridge::frb;

/// Pairing state for Google Messages WebView pairing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[frb(dart_metadata=("freezed"))]
pub enum GMPairingState {
    /// Not paired - initial state
    NotPaired,
    /// WebView is open, waiting for user to scan QR
    WaitingForPairing,
    /// Successfully paired with Google Messages
    Paired,
    /// Session has expired, needs re-pairing
    Expired,
    /// An error occurred during pairing
    Error,
}

impl Default for GMPairingState {
    fn default() -> Self {
        GMPairingState::NotPaired
    }
}

/// Direction of a message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GMMessageDirection {
    Incoming,
    Outgoing,
}

/// Status of an outgoing message
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GMMessageStatus {
    /// Message is queued locally, not yet sent
    Queued,
    /// Message is currently being sent
    Sending,
    /// Message was sent successfully
    Sent,
    /// Message sending failed
    Failed,
    /// Message was delivered (if delivery receipts available)
    Delivered,
}

impl Default for GMMessageStatus {
    fn default() -> Self {
        GMMessageStatus::Queued
    }
}

/// A thread mapping between Google Messages and local DB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GMThread {
    /// Local ID (auto-increment)
    pub id: Option<i64>,
    /// Remote thread ID from Google Messages
    pub remote_thread_id: String,
    /// Local chat UUID in the app's DB
    pub local_chat_guid: Option<String>,
    /// Last updated timestamp
    pub updated_at: i64,
    /// Sync cursor for this thread (last message timestamp or ID)
    pub cursor: Option<String>,
    /// Display name of the thread/conversation
    pub display_name: Option<String>,
    /// Participants (comma-separated phone numbers/emails)
    pub participants: Option<String>,
    /// Whether this thread is a group conversation
    pub is_group: bool,
}

impl GMThread {
    pub fn new(remote_thread_id: String) -> Self {
        Self {
            id: None,
            remote_thread_id,
            local_chat_guid: None,
            updated_at: chrono_timestamp_ms(),
            cursor: None,
            display_name: None,
            participants: None,
            is_group: false,
        }
    }
}

/// A message mapping between Google Messages and local DB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GMMessage {
    /// Local ID (auto-increment)
    pub id: Option<i64>,
    /// Remote message ID from Google Messages
    pub remote_message_id: String,
    /// Remote thread ID this message belongs to
    pub remote_thread_id: String,
    /// Local message UUID in the app's DB
    pub local_message_guid: Option<String>,
    /// Message timestamp (ms since epoch)
    pub timestamp: i64,
    /// Direction (incoming/outgoing)
    pub direction: GMMessageDirection,
    /// Message status
    pub status: GMMessageStatus,
    /// Message text content
    pub text: Option<String>,
}

impl GMMessage {
    pub fn new(remote_message_id: String, remote_thread_id: String, direction: GMMessageDirection) -> Self {
        Self {
            id: None,
            remote_message_id,
            remote_thread_id,
            local_message_guid: None,
            timestamp: chrono_timestamp_ms(),
            direction,
            status: GMMessageStatus::Sent,
            text: None,
        }
    }
}

/// An outgoing message in the outbox queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GMOutboxItem {
    /// Local ID (auto-increment)
    pub id: Option<i64>,
    /// Remote thread ID to send to
    pub remote_thread_id: String,
    /// Local message UUID
    pub local_message_guid: String,
    /// Message text to send
    pub text: String,
    /// Current status
    pub status: GMMessageStatus,
    /// Number of send attempts
    pub attempts: i32,
    /// Next retry timestamp (ms since epoch), None if not scheduled
    pub next_retry_at: Option<i64>,
    /// Last error message if failed
    pub last_error: Option<String>,
    /// Created timestamp
    pub created_at: i64,
}

impl GMOutboxItem {
    pub fn new(remote_thread_id: String, local_message_guid: String, text: String) -> Self {
        Self {
            id: None,
            remote_thread_id,
            local_message_guid,
            text,
            status: GMMessageStatus::Queued,
            attempts: 0,
            next_retry_at: None,
            last_error: None,
            created_at: chrono_timestamp_ms(),
        }
    }
    
    /// Calculate next retry time with exponential backoff
    /// Returns None if max attempts exceeded
    pub fn calculate_next_retry(&self, max_attempts: i32) -> Option<i64> {
        if self.attempts >= max_attempts {
            return None;
        }
        // Exponential backoff: 5s, 10s, 20s, 40s, 80s, ...
        let delay_ms = 5000i64 * (1i64 << self.attempts.min(6));
        Some(chrono_timestamp_ms() + delay_ms)
    }
}

/// Summary of a thread for display in the conversation list
#[derive(Debug, Clone, Serialize, Deserialize)]
#[frb(dart_metadata=("freezed"))]
pub struct GMThreadSummary {
    pub remote_thread_id: String,
    pub display_name: Option<String>,
    pub last_message_text: Option<String>,
    pub last_message_timestamp: i64,
    pub unread_count: i32,
    pub is_group: bool,
    pub participants: Vec<String>,
}

/// Result of sending a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[frb(dart_metadata=("freezed"))]
pub enum GMSendResult {
    /// Message sent successfully
    Success { remote_message_id: String },
    /// Message queued for retry
    Queued { outbox_id: i64 },
    /// Message sending failed
    Failed { error: String },
}

/// Error types for Google Messages operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[frb(dart_metadata=("freezed"))]
pub enum GMError {
    /// Not paired with Google Messages
    NotPaired,
    /// Session expired, needs re-pairing
    SessionExpired,
    /// Network error
    NetworkError { message: String },
    /// Authentication error
    AuthError { message: String },
    /// Database error
    DatabaseError { message: String },
    /// Unknown error
    Unknown { message: String },
}

impl std::fmt::Display for GMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GMError::NotPaired => write!(f, "Not paired with Google Messages"),
            GMError::SessionExpired => write!(f, "Google Messages session expired"),
            GMError::NetworkError { message } => write!(f, "Network error: {}", message),
            GMError::AuthError { message } => write!(f, "Authentication error: {}", message),
            GMError::DatabaseError { message } => write!(f, "Database error: {}", message),
            GMError::Unknown { message } => write!(f, "Unknown error: {}", message),
        }
    }
}

impl std::error::Error for GMError {}

/// Diagnostics info for the Google Messages connection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[frb(dart_metadata=("freezed"))]
pub struct GMDiagnostics {
    pub pairing_state: GMPairingState,
    pub last_sync_time: Option<i64>,
    pub outbox_size: i32,
    pub last_error: Option<String>,
    pub thread_count: i32,
    pub message_count: i32,
}

/// Get current timestamp in milliseconds
fn chrono_timestamp_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outbox_retry_backoff() {
        let mut item = GMOutboxItem::new(
            "thread1".to_string(),
            "msg1".to_string(),
            "Hello".to_string(),
        );
        
        // First retry: 5 seconds
        item.attempts = 0;
        let retry1 = item.calculate_next_retry(5);
        assert!(retry1.is_some());
        
        // Second retry: 10 seconds
        item.attempts = 1;
        let retry2 = item.calculate_next_retry(5);
        assert!(retry2.is_some());
        
        // Should eventually return None when max attempts exceeded
        item.attempts = 5;
        let retry_max = item.calculate_next_retry(5);
        assert!(retry_max.is_none());
    }

    #[test]
    fn test_gm_thread_creation() {
        let thread = GMThread::new("remote123".to_string());
        assert_eq!(thread.remote_thread_id, "remote123");
        assert!(thread.id.is_none());
        assert!(thread.local_chat_guid.is_none());
    }

    #[test]
    fn test_gm_message_creation() {
        let msg = GMMessage::new(
            "msg123".to_string(),
            "thread123".to_string(),
            GMMessageDirection::Outgoing,
        );
        assert_eq!(msg.remote_message_id, "msg123");
        assert_eq!(msg.remote_thread_id, "thread123");
        assert_eq!(msg.direction, GMMessageDirection::Outgoing);
    }
}
