fn main() {
    use argon2::{
        password_hash::{rand_core::OsRng, SaltString},
        Argon2, PasswordHasher, PasswordVerifier, PasswordHash,
    };
    let pin = std::env::args().nth(1).unwrap_or_else(|| "0009".to_string());
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(pin.as_bytes(), &salt)
        .expect("hash failed")
        .to_string();

    // Verify
    let parsed = PasswordHash::new(&hash).expect("parse failed");
    assert!(argon2.verify_password(pin.as_bytes(), &parsed).is_ok());

    println!("{}", hash);
}
