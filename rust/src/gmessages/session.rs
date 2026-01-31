// Google Messages session management
// Handles cookie storage and session state (encrypted at rest)

use crate::gmessages::models::{GmError, GmPairingState};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Session data stored securely
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmSessionData {
    /// Cookies as key-value pairs
    pub cookies: Vec<(String, String)>,
    /// Session created timestamp (Unix millis)
    pub created_at: i64,
    /// Last activity timestamp (Unix millis)
    pub last_activity_at: i64,
    /// User agent used during pairing
    pub user_agent: String,
}

impl GmSessionData {
    pub fn new(cookies: Vec<(String, String)>, user_agent: String) -> Self {
        let now = current_timestamp_millis();
        Self {
            cookies,
            created_at: now,
            last_activity_at: now,
            user_agent,
        }
    }

    /// Update last activity timestamp
    pub fn touch(&mut self) {
        self.last_activity_at = current_timestamp_millis();
    }

    /// Check if session might be expired (heuristic: 30 days)
    pub fn is_likely_expired(&self) -> bool {
        let now = current_timestamp_millis();
        let thirty_days_millis = 30 * 24 * 60 * 60 * 1000;
        now - self.last_activity_at > thirty_days_millis
    }

    /// Get cookies as a header string
    pub fn get_cookie_header(&self) -> String {
        self.cookies
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// Thread-safe session manager
pub struct GmSessionManager {
    session: Arc<RwLock<Option<GmSessionData>>>,
    pairing_state: Arc<RwLock<GmPairingState>>,
    /// Callback to encrypt and save session data
    save_callback: Arc<RwLock<Option<Box<dyn Fn(&[u8]) -> Result<(), String> + Send + Sync>>>>,
    /// Callback to load and decrypt session data
    load_callback: Arc<RwLock<Option<Box<dyn Fn() -> Result<Option<Vec<u8>>, String> + Send + Sync>>>>,
}

impl Default for GmSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GmSessionManager {
    pub fn new() -> Self {
        Self {
            session: Arc::new(RwLock::new(None)),
            pairing_state: Arc::new(RwLock::new(GmPairingState::NotPaired)),
            save_callback: Arc::new(RwLock::new(None)),
            load_callback: Arc::new(RwLock::new(None)),
        }
    }

    /// Set callbacks for secure storage (called from Flutter/platform code)
    pub fn set_storage_callbacks<S, L>(&self, save: S, load: L)
    where
        S: Fn(&[u8]) -> Result<(), String> + Send + Sync + 'static,
        L: Fn() -> Result<Option<Vec<u8>>, String> + Send + Sync + 'static,
    {
        *self.save_callback.write().unwrap() = Some(Box::new(save));
        *self.load_callback.write().unwrap() = Some(Box::new(load));
    }

    /// Get current pairing state
    pub fn get_pairing_state(&self) -> GmPairingState {
        self.pairing_state.read().unwrap().clone()
    }

    /// Set pairing state
    pub fn set_pairing_state(&self, state: GmPairingState) {
        *self.pairing_state.write().unwrap() = state;
    }

    /// Check if we have a valid session
    pub fn has_session(&self) -> bool {
        self.session.read().unwrap().is_some()
    }

    /// Get session data (if available)
    pub fn get_session(&self) -> Option<GmSessionData> {
        self.session.read().unwrap().clone()
    }

    /// Set session from cookies (after successful pairing)
    /// Does NOT log cookie values for security
    pub fn set_session_from_cookies(
        &self,
        cookies: Vec<(String, String)>,
        user_agent: String,
    ) -> Result<(), GmError> {
        log::info!("Setting GM session with {} cookies", cookies.len());
        
        let session_data = GmSessionData::new(cookies, user_agent);
        
        // Save to secure storage
        self.persist_session(&session_data)?;
        
        // Update in-memory state
        *self.session.write().unwrap() = Some(session_data);
        *self.pairing_state.write().unwrap() = GmPairingState::Paired;
        
        Ok(())
    }

    /// Parse cookies from a header string (e.g., from WebView)
    pub fn parse_cookie_header(cookie_header: &str) -> Vec<(String, String)> {
        cookie_header
            .split(';')
            .filter_map(|part| {
                let part = part.trim();
                if part.is_empty() {
                    return None;
                }
                let mut iter = part.splitn(2, '=');
                let key = iter.next()?.trim().to_string();
                let value = iter.next().unwrap_or("").trim().to_string();
                Some((key, value))
            })
            .collect()
    }

    /// Touch session (update last activity)
    pub fn touch_session(&self) {
        if let Some(session) = self.session.write().unwrap().as_mut() {
            session.touch();
            // Persist the updated timestamp
            if let Err(e) = self.persist_session(session) {
                log::warn!("Failed to persist session after touch: {:?}", e);
            }
        }
    }

    /// Persist session to secure storage
    fn persist_session(&self, session: &GmSessionData) -> Result<(), GmError> {
        let save_cb = self.save_callback.read().unwrap();
        if let Some(ref save) = *save_cb {
            let data = serde_json::to_vec(session).map_err(|e| GmError::DatabaseError {
                message: format!("Failed to serialize session: {}", e),
            })?;
            save(&data).map_err(|e| GmError::DatabaseError { message: e })?;
        } else {
            log::warn!("No save callback set for GM session");
        }
        Ok(())
    }

    /// Load session from secure storage
    pub fn load_session(&self) -> Result<bool, GmError> {
        let load_cb = self.load_callback.read().unwrap();
        if let Some(ref load) = *load_cb {
            match load() {
                Ok(Some(data)) => {
                    let session: GmSessionData = serde_json::from_slice(&data).map_err(|e| {
                        GmError::DatabaseError {
                            message: format!("Failed to deserialize session: {}", e),
                        }
                    })?;
                    
                    // Check if likely expired
                    if session.is_likely_expired() {
                        log::info!("GM session appears to be expired");
                        *self.pairing_state.write().unwrap() = GmPairingState::Expired;
                        return Ok(false);
                    }
                    
                    log::info!("Loaded GM session from secure storage");
                    *self.session.write().unwrap() = Some(session);
                    *self.pairing_state.write().unwrap() = GmPairingState::Paired;
                    Ok(true)
                }
                Ok(None) => {
                    log::info!("No GM session found in storage");
                    Ok(false)
                }
                Err(e) => Err(GmError::DatabaseError { message: e }),
            }
        } else {
            log::warn!("No load callback set for GM session");
            Ok(false)
        }
    }

    /// Clear session (logout)
    pub fn clear_session(&self) -> Result<(), GmError> {
        log::info!("Clearing GM session");
        *self.session.write().unwrap() = None;
        *self.pairing_state.write().unwrap() = GmPairingState::NotPaired;
        
        // Clear from secure storage by saving empty
        let save_cb = self.save_callback.read().unwrap();
        if let Some(ref save) = *save_cb {
            // Save empty data to clear
            save(&[]).map_err(|e| GmError::DatabaseError { message: e })?;
        }
        
        Ok(())
    }

    /// Mark session as expired
    pub fn mark_expired(&self) {
        log::info!("Marking GM session as expired");
        *self.pairing_state.write().unwrap() = GmPairingState::Expired;
    }

    /// Mark session as errored
    pub fn mark_error(&self, message: String) {
        log::error!("GM session error: {}", message);
        *self.pairing_state.write().unwrap() = GmPairingState::Error { message };
    }
}

/// Get current timestamp in milliseconds
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
    fn test_parse_cookie_header() {
        let header = "foo=bar; baz=qux; empty=";
        let cookies = GmSessionManager::parse_cookie_header(header);
        assert_eq!(cookies.len(), 3);
        assert_eq!(cookies[0], ("foo".to_string(), "bar".to_string()));
        assert_eq!(cookies[1], ("baz".to_string(), "qux".to_string()));
        assert_eq!(cookies[2], ("empty".to_string(), "".to_string()));
    }

    #[test]
    fn test_session_cookie_header() {
        let session = GmSessionData::new(
            vec![
                ("foo".to_string(), "bar".to_string()),
                ("baz".to_string(), "qux".to_string()),
            ],
            "test-agent".to_string(),
        );
        let header = session.get_cookie_header();
        assert_eq!(header, "foo=bar; baz=qux");
    }
}
