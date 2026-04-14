// token_validation.rs — thin facade re-exporting from submodules
//
// Split into:
//   token_consume.rs — validate_pin, validate_qr, start_now (token consumption paths)
//   token_manage.rs  — cancel, expire, get_pending, dashboard commands, kiosk validation

pub use super::token_consume::{validate_pin, validate_qr, start_now};
pub use super::token_manage::{
    cancel_auth_token, expire_stale_tokens, get_pending_tokens,
    handle_dashboard_command, handle_pin_entered,
    validate_pin_kiosk, KioskPinResult,
};
