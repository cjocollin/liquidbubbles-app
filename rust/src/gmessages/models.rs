// Google Messages models for Flutter Rust Bridge
// This module defines data types for Google Messages integration

use flutter_rust_bridge::frb;
use serde::{Deserialize, Serialize};

/// Pairing state for Google Messages WebView connection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[frb(dart_metadata = ("freezed"))]
pub enum GmPairingState {
    /// Not yet paired - initial state
    NotPaired,
    /// WebView is open, waiting for user to scan QR
    WaitingForPairing,
    /// Successfully paired with Google Messages
    Paired,
    /// Session has expired, needs re-pairing
    Expired,
    /// Error occurred during pairing
    Error { message: String },
}

impl Default for GmPairingState {
    fn default() -> Self {
        GmPairingState::NotPaired
    }
}

/// Status of an outgoing message in the queue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[frb(dart_metadata = ("freezed"))]
pub enum GmMessageStatus {
    /// Message is queued for sending
    Queued,
    /// Message is currently being sent
    Sending,
    /// Message was sent successfully
    Sent,
    /// Message failed to send (can be retried)
    Failed { error: String },
    /// Message send was cancelled
    Cancelled,
}

impl Default for GmMessageStatus {
    fn default() -> Self {
        GmMessageStatus::Queued
    }
}

/// Direction of a message
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[frb(dart_metadata = ("freezed"))]
pub enum GmMessageDirection {
    /// Message sent by the user
    Outgoing,
    /// Message received from another party
    Incoming,
}

/// A thread/conversation summary from Google Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[frb(dart_metadata = ("freezed"))]
pub struct GmThreadSummary {
    /// Remote thread ID from Google Messages
    pub remote_thread_id: String,
    /// Local UUID mapping (if exists)
    pub local_thread_uuid: Option<String>,
    /// Display name of the conversation
    pub display_name: Option<String>,
    /// Participant phone numbers/emails
    pub participants: Vec<String>,
    /// Snippet of the last message
    pub last_message_snippet: Option<String>,
    /// Timestamp of the last message (Unix millis)
    pub last_message_timestamp: Option<i64>,
    /// Number of unread messages
    pub unread_count: u32,
    /// Whether this is a group conversation
    pub is_group: bool,
    /// Last sync timestamp for this thread
    pub updated_at: i64,
}

/// A message from Google Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[frb(dart_metadata = ("freezed"))]
pub struct GmMessage {
    /// Remote message ID from Google Messages
    pub remote_message_id: String,
    /// Remote thread ID this message belongs to
    pub remote_thread_id: String,
    /// Local UUID mapping (if exists)
    pub local_message_uuid: Option<String>,
    /// Message text content
    pub text: Option<String>,
    /// Timestamp (Unix millis)
    pub timestamp: i64,
    /// Message direction
    pub direction: GmMessageDirection,
    /// Message status (for outgoing)
    pub status: GmMessageStatus,
    /// Sender identifier (for incoming group messages)
    pub sender: Option<String>,
    /// Whether this message has been read
    pub is_read: bool,
}

/// Result from sending a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[frb(dart_metadata = ("freezed"))]
pub struct GmSendResult {
    /// Whether the send was successful
    pub success: bool,
    /// Remote message ID if successful
    pub remote_message_id: Option<String>,
    /// Error message if failed
    pub error: Option<String>,
    /// Whether this can be retried
    pub can_retry: bool,
}

/// Outbox item for queued messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmOutboxItem {
    /// Unique ID for this outbox entry
    pub id: i64,
    /// Account ID
    pub account_id: String,
    /// Remote thread ID to send to
    pub remote_thread_id: String,
    /// Local message UUID for tracking
    pub local_message_uuid: String,
    /// Text content to send
    pub text: String,
    /// Current status
    pub status: GmMessageStatus,
    /// Number of send attempts
    pub attempts: u32,
    /// Next retry timestamp (Unix millis), if applicable
    pub next_retry_at: Option<i64>,
    /// Last error message
    pub last_error: Option<String>,
    /// Created timestamp
    pub created_at: i64,
}

/// Sync cursor for tracking sync progress per thread
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmSyncCursor {
    /// Thread ID
    pub remote_thread_id: String,
    /// Last synced message ID or timestamp
    pub cursor: String,
    /// Last sync timestamp
    pub synced_at: i64,
}

/// Google Messages error types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[frb(dart_metadata = ("freezed"))]
pub enum GmError {
    /// Not authenticated / session expired
    NotAuthenticated,
    /// Network error
    NetworkError { message: String },
    /// Rate limited
    RateLimited { retry_after_secs: Option<u64> },
    /// Invalid response from server
    InvalidResponse { message: String },
    /// Database error
    DatabaseError { message: String },
    /// Unknown error
    Unknown { message: String },
}

impl std::fmt::Display for GmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GmError::NotAuthenticated => write!(f, "Not authenticated"),
            GmError::NetworkError { message } => write!(f, "Network error: {}", message),
            GmError::RateLimited { retry_after_secs } => {
                if let Some(secs) = retry_after_secs {
                    write!(f, "Rate limited, retry after {} seconds", secs)
                } else {
                    write!(f, "Rate limited")
                }
            }
            GmError::InvalidResponse { message } => write!(f, "Invalid response: {}", message),
            GmError::DatabaseError { message } => write!(f, "Database error: {}", message),
            GmError::Unknown { message } => write!(f, "Unknown error: {}", message),
        }
    }
}

impl std::error::Error for GmError {}
