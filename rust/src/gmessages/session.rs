//! Google Messages session management
//! 
//! Handles session state, encrypted cookie storage, and expiry detection.
//! Session data is encrypted at rest using Android Keystore-backed keys.

use std::sync::RwLock;
use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};
use log::{debug, error, info, warn};
use aes_gcm::{
    aead::{Aead, KeyInit, generic_array::GenericArray},
    Aes256Gcm, Nonce,
};
use base64::prelude::*;

use super::models::{GMPairingState, GMError};

/// Session data stored for Google Messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GMSession {
    /// HTTP cookies from the WebView (encrypted at rest)
    pub cookies: String,
    /// When the session was established
    pub created_at: i64,
    /// Last successful activity timestamp
    pub last_active_at: i64,
    /// User agent string to use
    pub user_agent: Option<String>,
}

impl GMSession {
    pub fn new(cookies: String) -> Self {
        let now = chrono_timestamp_ms();
        Self {
            cookies,
            created_at: now,
            last_active_at: now,
            user_agent: None,
        }
    }

    /// Check if the session might be expired (heuristic)
    /// Google Messages sessions can expire after inactivity
    pub fn is_likely_expired(&self) -> bool {
        let now = chrono_timestamp_ms();
        let inactivity_threshold_ms = 7 * 24 * 60 * 60 * 1000; // 7 days
        now - self.last_active_at > inactivity_threshold_ms
    }

    /// Update the last active timestamp
    pub fn touch(&mut self) {
        self.last_active_at = chrono_timestamp_ms();
    }
}

/// Encrypted session file format
#[derive(Serialize, Deserialize)]
struct EncryptedSessionFile {
    /// Base64-encoded encrypted data
    ciphertext: String,
    /// Base64-encoded nonce/IV
    nonce: String,
}

/// Session manager - handles persistence and encryption
pub struct GMSessionManager {
    /// Current session (if paired)
    session: RwLock<Option<GMSession>>,
    /// Current pairing state
    pairing_state: RwLock<GMPairingState>,
    /// Path to store session file
    storage_path: PathBuf,
    /// Encryption key (should come from Android Keystore)
    /// In production, this is derived from Keystore; for now we use a placeholder
    encryption_key: Option<[u8; 32]>,
}

impl GMSessionManager {
    /// Create a new session manager
    pub fn new(storage_path: PathBuf) -> Self {
        Self {
            session: RwLock::new(None),
            pairing_state: RwLock::new(GMPairingState::NotPaired),
            storage_path,
            encryption_key: None,
        }
    }

    /// Set the encryption key (called from Flutter/Android with Keystore-derived key)
    pub fn set_encryption_key(&mut self, key: [u8; 32]) {
        self.encryption_key = Some(key);
    }

    /// Get current pairing state
    pub fn get_pairing_state(&self) -> GMPairingState {
        *self.pairing_state.read().unwrap()
    }

    /// Set pairing state
    pub fn set_pairing_state(&self, state: GMPairingState) {
        *self.pairing_state.write().unwrap() = state;
    }

    /// Check if currently paired
    pub fn is_paired(&self) -> bool {
        matches!(self.get_pairing_state(), GMPairingState::Paired)
    }

    /// Store a new session from WebView cookies
    pub fn set_session_from_cookies(&self, cookies: String) -> Result<(), GMError> {
        let session = GMSession::new(cookies);
        
        // Save encrypted session
        self.save_session(&session)?;
        
        // Update in-memory state
        *self.session.write().unwrap() = Some(session);
        self.set_pairing_state(GMPairingState::Paired);
        
        info!("Google Messages session established");
        Ok(())
    }

    /// Get current session cookies
    pub fn get_cookies(&self) -> Option<String> {
        let session_guard = self.session.read().unwrap();
        session_guard.as_ref().map(|s| s.cookies.clone())
    }

    /// Update session last active time
    pub fn touch_session(&self) {
        if let Some(session) = self.session.write().unwrap().as_mut() {
            session.touch();
            // Don't save on every touch to avoid excessive I/O
        }
    }

    /// Load session from disk (called on startup)
    pub fn load_session(&self) -> Result<bool, GMError> {
        let session_file = self.storage_path.join("gm_session.enc");
        
        if !session_file.exists() {
            debug!("No existing GM session file");
            return Ok(false);
        }

        let content = fs::read_to_string(&session_file)
            .map_err(|e| GMError::DatabaseError { 
                message: format!("Failed to read session file: {}", e) 
            })?;

        // If no encryption key, we can't decrypt
        let key = self.encryption_key.ok_or_else(|| GMError::AuthError {
            message: "Encryption key not set".to_string()
        })?;

        let encrypted: EncryptedSessionFile = serde_json::from_str(&content)
            .map_err(|e| GMError::DatabaseError { 
                message: format!("Failed to parse session file: {}", e) 
            })?;

        let session_json = self.decrypt_data(&key, &encrypted)?;
        let session: GMSession = serde_json::from_str(&session_json)
            .map_err(|e| GMError::DatabaseError { 
                message: format!("Failed to parse session data: {}", e) 
            })?;

        // Check if session is likely expired
        if session.is_likely_expired() {
            warn!("Loaded GM session appears expired");
            self.set_pairing_state(GMPairingState::Expired);
            *self.session.write().unwrap() = Some(session);
            return Ok(false);
        }

        info!("Loaded existing GM session");
        *self.session.write().unwrap() = Some(session);
        self.set_pairing_state(GMPairingState::Paired);
        Ok(true)
    }

    /// Save session to disk (encrypted)
    fn save_session(&self, session: &GMSession) -> Result<(), GMError> {
        let key = self.encryption_key.ok_or_else(|| GMError::AuthError {
            message: "Encryption key not set".to_string()
        })?;

        let session_json = serde_json::to_string(session)
            .map_err(|e| GMError::DatabaseError { 
                message: format!("Failed to serialize session: {}", e) 
            })?;

        let encrypted = self.encrypt_data(&key, &session_json)?;

        let content = serde_json::to_string(&encrypted)
            .map_err(|e| GMError::DatabaseError { 
                message: format!("Failed to serialize encrypted session: {}", e) 
            })?;

        let session_file = self.storage_path.join("gm_session.enc");
        fs::write(&session_file, content)
            .map_err(|e| GMError::DatabaseError { 
                message: format!("Failed to write session file: {}", e) 
            })?;

        debug!("Saved encrypted GM session");
        Ok(())
    }

    /// Encrypt data using AES-256-GCM
    fn encrypt_data(&self, key: &[u8; 32], plaintext: &str) -> Result<EncryptedSessionFile, GMError> {
        let cipher = Aes256Gcm::new(GenericArray::from_slice(key));
        
        // Generate random nonce
        let mut nonce_bytes = [0u8; 12];
        getrandom::getrandom(&mut nonce_bytes).map_err(|e| GMError::Unknown {
            message: format!("Failed to generate nonce: {}", e)
        })?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| GMError::Unknown {
                message: format!("Encryption failed: {}", e)
            })?;

        Ok(EncryptedSessionFile {
            ciphertext: BASE64_STANDARD.encode(&ciphertext),
            nonce: BASE64_STANDARD.encode(&nonce_bytes),
        })
    }

    /// Decrypt data using AES-256-GCM
    fn decrypt_data(&self, key: &[u8; 32], encrypted: &EncryptedSessionFile) -> Result<String, GMError> {
        let cipher = Aes256Gcm::new(GenericArray::from_slice(key));

        let ciphertext = BASE64_STANDARD.decode(&encrypted.ciphertext)
            .map_err(|e| GMError::Unknown {
                message: format!("Invalid ciphertext encoding: {}", e)
            })?;

        let nonce_bytes = BASE64_STANDARD.decode(&encrypted.nonce)
            .map_err(|e| GMError::Unknown {
                message: format!("Invalid nonce encoding: {}", e)
            })?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| GMError::AuthError {
                message: format!("Decryption failed: {}", e)
            })?;

        String::from_utf8(plaintext)
            .map_err(|e| GMError::Unknown {
                message: format!("Invalid UTF-8 in decrypted data: {}", e)
            })
    }

    /// Clear session (logout)
    pub fn logout(&self) -> Result<(), GMError> {
        // Clear in-memory session
        *self.session.write().unwrap() = None;
        self.set_pairing_state(GMPairingState::NotPaired);

        // Delete session file
        let session_file = self.storage_path.join("gm_session.enc");
        if session_file.exists() {
            fs::remove_file(&session_file)
                .map_err(|e| GMError::DatabaseError {
                    message: format!("Failed to delete session file: {}", e)
                })?;
        }

        info!("Google Messages session cleared");
        Ok(())
    }

    /// Mark session as expired
    pub fn mark_expired(&self) {
        self.set_pairing_state(GMPairingState::Expired);
    }
}

/// Get current timestamp in milliseconds
fn chrono_timestamp_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Global session manager instance
pub static GM_SESSION: std::sync::OnceLock<GMSessionManager> = std::sync::OnceLock::new();

/// Initialize the global session manager
pub fn init_session_manager(storage_path: PathBuf) -> Result<(), String> {
    let manager = GMSessionManager::new(storage_path);
    GM_SESSION.set(manager).map_err(|_| "Session manager already initialized")?;
    Ok(())
}

/// Get the global session manager
pub fn get_session_manager() -> Result<&'static GMSessionManager, String> {
    GM_SESSION.get().ok_or_else(|| "Session manager not initialized".to_string())
}
