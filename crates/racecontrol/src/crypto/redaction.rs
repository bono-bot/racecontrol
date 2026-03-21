/// Redact a phone number for safe logging.
/// Returns "***" + last 4 digits. If phone is shorter than 4 chars, returns "***".
pub fn redact_phone(phone: &str) -> String {
    let trimmed = phone.trim();
    if trimmed.len() < 4 {
        "***".to_string()
    } else {
        format!("***{}", &trimmed[trimmed.len() - 4..])
    }
}

/// Redact an OTP code for safe logging. Always returns "[REDACTED]".
pub fn redact_otp(_otp: &str) -> &'static str {
    "[REDACTED]"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_phone_normal() {
        assert_eq!(redact_phone("9876543210"), "***3210");
    }

    #[test]
    fn redact_phone_with_prefix() {
        assert_eq!(redact_phone("+919876543210"), "***3210");
    }

    #[test]
    fn redact_phone_short() {
        assert_eq!(redact_phone("12"), "***");
    }

    #[test]
    fn redact_otp_always_redacted() {
        assert_eq!(redact_otp("1234"), "[REDACTED]");
    }
}
