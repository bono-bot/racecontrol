//! INC-2H — pure SPKI pin-decision tests (run in BOTH default and --features redeem_client).

use rc_installer::tls_pin::{
    console_pins_configured, enforce_pin, load_console_pins, pin_satisfied, sha256_spki, SpkiPin,
};
use rc_installer::InstallError;

#[test]
fn from_hex_decodes_known_32_byte_pin() {
    // hex of SHA-256("") == sha256_spki(b"") — ties the hex decoder to the hasher.
    let pin =
        SpkiPin::from_hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
            .unwrap();
    assert_eq!(pin.0, sha256_spki(b""));
}

#[test]
fn from_hex_rejects_wrong_length() {
    // "aabbcc" is valid hex but only 3 bytes, not 32.
    assert!(matches!(
        SpkiPin::from_hex("aabbcc"),
        Err(InstallError::Malformed(_))
    ));
}

#[test]
fn from_hex_rejects_non_hex() {
    assert!(matches!(
        SpkiPin::from_hex("zzggxxnothexnothexnothexnothexnothexnothexnothexnothexnothexnotZZ"),
        Err(InstallError::Malformed(_))
    ));
}

#[test]
fn single_pin_match() {
    let pin = SpkiPin(sha256_spki(b"leaf-key"));
    let presented = [sha256_spki(b"leaf-key")];
    assert!(pin_satisfied(&[pin], &presented));
    assert!(enforce_pin(&[pin], &presented).is_ok());
}

#[test]
fn rotation_two_pins_matches_second() {
    let current = SpkiPin(sha256_spki(b"current-key"));
    let next = SpkiPin(sha256_spki(b"next-key"));
    let presented = [sha256_spki(b"next-key")]; // server rotated to the next key
    assert!(pin_satisfied(&[current, next], &presented));
}

#[test]
fn no_overlap_rejected() {
    let pin = SpkiPin(sha256_spki(b"our-key"));
    let presented = [sha256_spki(b"attacker-key")];
    assert!(!pin_satisfied(&[pin], &presented));
    assert!(matches!(
        enforce_pin(&[pin], &presented),
        Err(InstallError::PinMismatch)
    ));
}

#[test]
fn empty_pins_fail_closed() {
    let presented = [sha256_spki(b"anything")];
    assert!(!pin_satisfied(&[], &presented));
    assert!(matches!(
        enforce_pin(&[], &presented),
        Err(InstallError::PinMismatch)
    ));
}

#[test]
fn no_presented_spki_fails_closed() {
    // WF2: the degenerate TLS case (server presents no cert SPKI) must fail closed.
    let pin = SpkiPin(sha256_spki(b"our-key"));
    assert!(!pin_satisfied(&[pin], &[]));
    assert!(matches!(
        enforce_pin(&[pin], &[]),
        Err(InstallError::PinMismatch)
    ));
}

#[test]
fn shipped_console_pins_are_placeholders_fail_closed() {
    // The shipped pins are human-readable placeholders that don't decode → no real pins.
    assert!(load_console_pins().is_empty());
    assert!(!console_pins_configured());
}
