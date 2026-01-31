// Google Messages web protocol client
// Handles HTTP communication with messages.google.com/web

use crate::gmessages::models::{GmError, GmMessage, GmMessageDirection, GmMessageStatus, GmThreadSummary};
use crate::gmessages::session::GmSessionManager;
use std::sync::Arc;
use std::time::Duration;

/// Base URL for Google Messages web
const GM_WEB_BASE: &str = "https://messages.google.com";

/// User agent to use for requests (mimics Chrome on Android)
pub const GM_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 13; Pixel 7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";

/// HTTP client for Google Messages
pub struct GmProtocolClient {
    session: Arc<GmSessionManager>,
    http_client: reqwest::Client,
}

impl GmProtocolClient {
    pub fn new(session: Arc<GmSessionManager>) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(GM_USER_AGENT)
            .build()
            .expect("Failed to create HTTP client");

        Self {
            session,
            http_client,
        }
    }

    /// Check if we have a valid session
    pub fn is_authenticated(&self) -> bool {
        self.session.has_session()
    }

    /// Get cookies for request headers
    fn get_cookie_header(&self) -> Result<String, GmError> {
        let session = self.session.get_session().ok_or(GmError::NotAuthenticated)?;
        Ok(session.get_cookie_header())
    }

    /// Make an authenticated GET request
    async fn get(&self, path: &str) -> Result<reqwest::Response, GmError> {
        let cookies = self.get_cookie_header()?;
        let url = format!("{}{}", GM_WEB_BASE, path);

        log::debug!("GM GET: {}", path);

        let response = self
            .http_client
            .get(&url)
            .header("Cookie", cookies)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| GmError::NetworkError {
                message: e.to_string(),
            })?;

        self.check_response_status(&response)?;
        self.session.touch_session();
        Ok(response)
    }

    /// Make an authenticated POST request
    async fn post(&self, path: &str, body: &str) -> Result<reqwest::Response, GmError> {
        let cookies = self.get_cookie_header()?;
        let url = format!("{}{}", GM_WEB_BASE, path);

        log::debug!("GM POST: {}", path);

        let response = self
            .http_client
            .post(&url)
            .header("Cookie", cookies)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| GmError::NetworkError {
                message: e.to_string(),
            })?;

        self.check_response_status(&response)?;
        self.session.touch_session();
        Ok(response)
    }

    /// Check response status and handle errors
    fn check_response_status(&self, response: &reqwest::Response) -> Result<(), GmError> {
        let status = response.status();

        if status.is_success() {
            return Ok(());
        }

        match status.as_u16() {
            401 | 403 => {
                log::warn!("GM session appears invalid ({})", status);
                self.session.mark_expired();
                Err(GmError::NotAuthenticated)
            }
            429 => {
                let retry_after = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse().ok());
                Err(GmError::RateLimited {
                    retry_after_secs: retry_after,
                })
            }
            _ => Err(GmError::NetworkError {
                message: format!("HTTP error: {}", status),
            }),
        }
    }

    /// Validate session is still active
    pub async fn validate_session(&self) -> Result<bool, GmError> {
        // Try to fetch a minimal resource to verify session
        match self.get("/web/u/0/").await {
            Ok(_) => Ok(true),
            Err(GmError::NotAuthenticated) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Fetch conversation/thread list
    /// Note: This is a simplified MVP implementation. The actual Google Messages
    /// web API uses protobuf over WebSocket. For MVP, we'll work with what we can
    /// extract from cookies and basic HTTP endpoints.
    pub async fn fetch_threads(&self) -> Result<Vec<GmThreadSummary>, GmError> {
        // For MVP, this would need to be implemented based on reverse-engineering
        // the actual GM web protocol. The mautrix/gmessages project does this via
        // protobuf, but we cannot copy their code due to AGPL licensing.
        //
        // For now, return an empty list and let the sync layer handle it
        // through the WebView bridge approach where Flutter extracts data
        // from the WebView's DOM/JS context.
        
        log::info!("GM fetch_threads called - MVP placeholder");
        
        // Placeholder - real implementation would parse response
        Ok(vec![])
    }

    /// Fetch messages for a thread
    pub async fn fetch_messages(
        &self,
        _thread_id: &str,
        _cursor: Option<&str>,
        _limit: usize,
    ) -> Result<(Vec<GmMessage>, Option<String>), GmError> {
        // Similar to fetch_threads - MVP placeholder
        // Real implementation would need to reverse-engineer GM protocol
        
        log::info!("GM fetch_messages called - MVP placeholder");
        
        Ok((vec![], None))
    }

    /// Send a text message
    /// Returns the remote message ID on success
    pub async fn send_message(
        &self,
        _thread_id: &str,
        _text: &str,
    ) -> Result<String, GmError> {
        // MVP placeholder - would need GM protocol implementation
        
        log::info!("GM send_message called - MVP placeholder");
        
        // For now, return an error indicating not implemented
        Err(GmError::Unknown {
            message: "Send not yet implemented in MVP".to_string(),
        })
    }

    /// Mark messages as read
    pub async fn mark_read(&self, _thread_id: &str, _message_ids: &[String]) -> Result<(), GmError> {
        log::info!("GM mark_read called - MVP placeholder");
        Ok(())
    }
}

/// Parse thread data from JSON response (placeholder structure)
/// Real implementation would need actual GM API response format
#[allow(dead_code)]
fn parse_thread_response(_json: &str) -> Result<Vec<GmThreadSummary>, GmError> {
    // Placeholder parser
    Ok(vec![])
}

/// Parse message data from JSON response (placeholder structure)
#[allow(dead_code)]
fn parse_message_response(_json: &str) -> Result<(Vec<GmMessage>, Option<String>), GmError> {
    // Placeholder parser
    Ok((vec![], None))
}

// ============= WebView Bridge Functions =============
// These are called from Flutter when extracting data from the WebView

/// Parse threads from WebView-extracted JSON
pub fn parse_threads_from_webview(json: &str) -> Result<Vec<GmThreadSummary>, GmError> {
    // Expected format from Flutter WebView JS extraction:
    // [{ "id": "...", "name": "...", "participants": [...], "lastMessage": {...}, ... }]
    
    #[derive(serde::Deserialize)]
    struct WebViewThread {
        id: String,
        name: Option<String>,
        participants: Option<Vec<String>>,
        #[serde(rename = "lastMessageText")]
        last_message_text: Option<String>,
        #[serde(rename = "lastMessageTime")]
        last_message_time: Option<i64>,
        #[serde(rename = "unreadCount")]
        unread_count: Option<u32>,
        #[serde(rename = "isGroup")]
        is_group: Option<bool>,
    }

    let threads: Vec<WebViewThread> = serde_json::from_str(json).map_err(|e| {
        GmError::InvalidResponse {
            message: format!("Failed to parse threads JSON: {}", e),
        }
    })?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    Ok(threads
        .into_iter()
        .map(|t| GmThreadSummary {
            remote_thread_id: t.id,
            local_thread_uuid: None,
            display_name: t.name,
            participants: t.participants.unwrap_or_default(),
            last_message_snippet: t.last_message_text,
            last_message_timestamp: t.last_message_time,
            unread_count: t.unread_count.unwrap_or(0),
            is_group: t.is_group.unwrap_or(false),
            updated_at: now,
        })
        .collect())
}

/// Parse messages from WebView-extracted JSON
pub fn parse_messages_from_webview(
    thread_id: &str,
    json: &str,
) -> Result<Vec<GmMessage>, GmError> {
    #[derive(serde::Deserialize)]
    struct WebViewMessage {
        id: String,
        text: Option<String>,
        timestamp: i64,
        #[serde(rename = "isFromMe")]
        is_from_me: bool,
        sender: Option<String>,
        #[serde(rename = "isRead")]
        is_read: Option<bool>,
    }

    let messages: Vec<WebViewMessage> = serde_json::from_str(json).map_err(|e| {
        GmError::InvalidResponse {
            message: format!("Failed to parse messages JSON: {}", e),
        }
    })?;

    Ok(messages
        .into_iter()
        .map(|m| GmMessage {
            remote_message_id: m.id,
            remote_thread_id: thread_id.to_string(),
            local_message_uuid: None,
            text: m.text,
            timestamp: m.timestamp,
            direction: if m.is_from_me {
                GmMessageDirection::Outgoing
            } else {
                GmMessageDirection::Incoming
            },
            status: GmMessageStatus::Sent,
            sender: m.sender,
            is_read: m.is_read.unwrap_or(true),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_threads_from_webview() {
        let json = r#"[
            {
                "id": "thread-1",
                "name": "John Doe",
                "participants": ["+1234567890"],
                "lastMessageText": "Hello!",
                "lastMessageTime": 1704067200000,
                "unreadCount": 2,
                "isGroup": false
            }
        ]"#;

        let threads = parse_threads_from_webview(json).unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].remote_thread_id, "thread-1");
        assert_eq!(threads[0].display_name, Some("John Doe".to_string()));
        assert_eq!(threads[0].unread_count, 2);
    }

    #[test]
    fn test_parse_messages_from_webview() {
        let json = r#"[
            {
                "id": "msg-1",
                "text": "Hello!",
                "timestamp": 1704067200000,
                "isFromMe": false,
                "sender": "+1234567890",
                "isRead": true
            },
            {
                "id": "msg-2",
                "text": "Hi there!",
                "timestamp": 1704067260000,
                "isFromMe": true,
                "isRead": true
            }
        ]"#;

        let messages = parse_messages_from_webview("thread-1", json).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].direction, GmMessageDirection::Incoming);
        assert_eq!(messages[1].direction, GmMessageDirection::Outgoing);
    }
}
