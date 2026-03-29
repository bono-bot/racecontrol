//! F2-SEC: Input validation helpers for customer-facing endpoints.
//!
//! Centralized validation so routes don't duplicate validation logic.
//! Call these from registration, booking, and profile update handlers.

/// Validate Indian phone number format (10 digits, starts with 6-9).
pub fn validate_phone(phone: &str) -> Result<(), String> {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() == 10 && digits.starts_with(|c: char| "6789".contains(c)) {
        Ok(())
    } else if digits.len() == 12 && digits.starts_with("91") {
        Ok(()) // +91 prefix
    } else {
        Err(format!("Invalid phone number: must be 10-digit Indian mobile (got {} digits)", digits.len()))
    }
}

/// Validate email format (basic RFC 5322 subset).
pub fn validate_email(email: &str) -> Result<(), String> {
    let email = email.trim();
    if email.is_empty() {
        return Ok(()); // Email is optional
    }
    if !email.contains('@') || !email.contains('.') {
        return Err("Invalid email format".to_string());
    }
    let parts: Vec<&str> = email.splitn(2, '@').collect();
    if parts.len() != 2 || parts[0].is_empty() || parts[1].len() < 3 {
        return Err("Invalid email format".to_string());
    }
    if email.len() > 254 {
        return Err("Email too long (max 254 chars)".to_string());
    }
    Ok(())
}

/// Validate driver name (2-100 chars, no HTML/script injection).
pub fn validate_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.len() < 2 {
        return Err("Name must be at least 2 characters".to_string());
    }
    if trimmed.len() > 100 {
        return Err("Name too long (max 100 characters)".to_string());
    }
    // Strip potential HTML/script tags
    if trimmed.contains('<') || trimmed.contains('>') || trimmed.contains("javascript:") {
        return Err("Name contains invalid characters".to_string());
    }
    Ok(trimmed.to_string())
}

/// Validate booking count (prevent DoS via mass booking).
pub fn validate_booking_count(current_active: usize, max_per_customer: usize) -> Result<(), String> {
    if current_active >= max_per_customer {
        return Err(format!(
            "Maximum {} active bookings per customer (currently {})",
            max_per_customer, current_active
        ));
    }
    Ok(())
}

/// Validate multiplayer pod count (max 4 pods per session, prevents DoS).
pub fn validate_multiplayer_pods(requested: usize) -> Result<(), String> {
    const MAX_PODS_PER_MULTIPLAYER: usize = 4;
    if requested > MAX_PODS_PER_MULTIPLAYER {
        return Err(format!(
            "Maximum {} pods per multiplayer session (requested {})",
            MAX_PODS_PER_MULTIPLAYER, requested
        ));
    }
    if requested < 2 {
        return Err("Multiplayer requires at least 2 pods".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_indian_phones() {
        assert!(validate_phone("9876543210").is_ok());
        assert!(validate_phone("6123456789").is_ok());
        assert!(validate_phone("919876543210").is_ok()); // +91 prefix
    }

    #[test]
    fn invalid_phones() {
        assert!(validate_phone("1234567890").is_err()); // starts with 1
        assert!(validate_phone("12345").is_err()); // too short
        assert!(validate_phone("abc").is_err());
    }

    #[test]
    fn valid_emails() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("").is_ok()); // optional
        assert!(validate_email("a@b.co").is_ok());
    }

    #[test]
    fn invalid_emails() {
        assert!(validate_email("notanemail").is_err());
        assert!(validate_email("@no-local.com").is_err());
        assert!(validate_email("no-at-sign.com").is_err());
    }

    #[test]
    fn name_validation() {
        assert!(validate_name("John").is_ok());
        assert!(validate_name("A").is_err()); // too short
        assert!(validate_name("<script>alert(1)</script>").is_err()); // XSS
    }

    #[test]
    fn booking_limits() {
        assert!(validate_booking_count(0, 3).is_ok());
        assert!(validate_booking_count(3, 3).is_err());
    }

    #[test]
    fn multiplayer_limits() {
        assert!(validate_multiplayer_pods(2).is_ok());
        assert!(validate_multiplayer_pods(4).is_ok());
        assert!(validate_multiplayer_pods(5).is_err());
        assert!(validate_multiplayer_pods(1).is_err());
    }
}
