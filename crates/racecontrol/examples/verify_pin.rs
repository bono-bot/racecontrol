fn main() {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let pin = std::env::args().nth(1).unwrap_or_else(|| "0009".to_string());
    let hash_str = std::env::args().nth(2).unwrap_or_else(|| {
        "$argon2id$v=19$m=19456,t=2,p=1$2RJ6ErSAY4dtglnbvdc81g$9aC0H80cqvKwioaDC2ksA7XeFluZZ6gTYtmtS3zFzdE".to_string()
    });
    let parsed = PasswordHash::new(&hash_str).expect("parse hash failed");
    let valid = Argon2::default().verify_password(pin.as_bytes(), &parsed).is_ok();
    println!("PIN '{}' vs hash: valid={}", pin, valid);
}
