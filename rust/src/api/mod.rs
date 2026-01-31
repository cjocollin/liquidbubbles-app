//
// Do not put code in `mod.rs`, but put in e.g. `simple.rs`.
//

pub mod api;

// Re-export Google Messages FRB functions
pub use crate::gmessages::{
    gm_is_enabled,
    gm_set_enabled,
    gm_get_pairing_state,
    gm_set_cookies_from_webview,
    gm_start_sync_loop,
    gm_stop_sync_loop,
    gm_sync_now,
    gm_set_foreground,
    gm_list_threads,
    gm_list_messages,
    gm_send_text,
    gm_retry_message,
    gm_cancel_message,
    gm_logout,
    gm_get_diagnostics,
    gm_sync_from_webview,
    gm_set_storage_callbacks,
    gm_set_db_callbacks,
    GmDiagnostics,
};

// Re-export Google Messages models
pub use crate::gmessages::models::{
    GmPairingState,
    GmMessageStatus,
    GmMessageDirection,
    GmThreadSummary,
    GmMessage,
    GmSendResult,
    GmError,
};