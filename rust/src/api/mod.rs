//
// Do not put code in `mod.rs`, but put in e.g. `simple.rs`.
//

pub mod api;

// Re-export Google Messages API for flutter_rust_bridge
pub use crate::gmessages::{
    gm_init,
    gm_is_enabled,
    gm_set_enabled,
    gm_get_pairing_state,
    gm_set_encryption_key,
    gm_set_cookies_from_webview,
    gm_load_session,
    gm_logout,
    gm_list_threads,
    gm_list_messages,
    gm_send_text,
    gm_sync_now,
    gm_start_sync_loop,
    gm_stop_sync_loop,
    gm_get_diagnostics,
    gm_upsert_thread,
    gm_upsert_message,
    gm_link_thread_to_chat,
    gm_link_message_to_local,
    models::{
        GMPairingState,
        GMMessageDirection,
        GMMessageStatus,
        GMThread,
        GMMessage,
        GMOutboxItem,
        GMThreadSummary,
        GMSendResult,
        GMError,
        GMDiagnostics,
    },
};