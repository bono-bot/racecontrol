//! PIN entry keyboard handling for the native lock screen.
//!
//! Handles WM_CHAR (digit input) and WM_KEYDOWN (backspace, enter) messages.
//! PIN buffer accepts digits '0'-'9', max 6 chars. Auto-submits at 6 digits.
//! Enter key submits on 4+ digits.

/// Tracks current PIN input state for the lock screen.
pub struct PinInputState {
    buffer: String,
    max_len: usize,
}

impl PinInputState {
    /// Create a new empty PIN input with max 6 digits.
    pub fn new() -> Self {
        Self {
            buffer: String::with_capacity(6),
            max_len: 6,
        }
    }

    /// Push a digit character. Returns true if accepted.
    /// Only accepts '0'-'9' and only if buffer is not full.
    pub fn push_digit(&mut self, ch: char) -> bool {
        if ch.is_ascii_digit() && self.buffer.len() < self.max_len {
            self.buffer.push(ch);
            true
        } else {
            false
        }
    }

    /// Remove the last digit. Returns true if a digit was removed.
    pub fn pop(&mut self) -> bool {
        self.buffer.pop().is_some()
    }

    /// Returns true if the PIN has enough digits to submit (>= 4).
    pub fn is_complete(&self) -> bool {
        self.buffer.len() >= 4
    }

    /// Returns true if the buffer is at max capacity (6 digits).
    pub fn is_full(&self) -> bool {
        self.buffer.len() == self.max_len
    }

    /// Take the buffer contents and reset to empty.
    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.buffer)
    }

    /// Returns a string of dots for display, one per entered digit.
    /// Uses Unicode bullet character with spacing.
    pub fn display_dots(&self) -> String {
        let mut s = String::new();
        for (i, _) in self.buffer.chars().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push('\u{25CF}'); // Black circle (bullet)
        }
        s
    }

    /// Current number of digits entered.
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Reset the buffer to empty.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pin_input_new_is_empty() {
        let pin = PinInputState::new();
        assert_eq!(pin.len(), 0);
        assert!(!pin.is_complete());
        assert!(!pin.is_full());
        assert_eq!(pin.display_dots(), "");
    }

    #[test]
    fn pin_input_push_digits() {
        let mut pin = PinInputState::new();
        assert!(pin.push_digit('1'));
        assert!(pin.push_digit('2'));
        assert!(pin.push_digit('3'));
        assert_eq!(pin.len(), 3);
        assert!(!pin.is_complete());
        assert!(pin.push_digit('4'));
        assert!(pin.is_complete());
        assert!(!pin.is_full());
    }

    #[test]
    fn pin_input_rejects_non_digits() {
        let mut pin = PinInputState::new();
        assert!(!pin.push_digit('a'));
        assert!(!pin.push_digit('!'));
        assert!(!pin.push_digit(' '));
        assert!(!pin.push_digit('\n'));
        assert_eq!(pin.len(), 0);
    }

    #[test]
    fn pin_input_overflow_at_6() {
        let mut pin = PinInputState::new();
        for ch in ['1', '2', '3', '4', '5', '6'] {
            assert!(pin.push_digit(ch));
        }
        assert!(pin.is_full());
        assert!(!pin.push_digit('7'));
        assert_eq!(pin.len(), 6);
    }

    #[test]
    fn pin_input_backspace() {
        let mut pin = PinInputState::new();
        assert!(!pin.pop()); // Empty backspace is no-op
        pin.push_digit('1');
        pin.push_digit('2');
        assert!(pin.pop());
        assert_eq!(pin.len(), 1);
        assert!(pin.pop());
        assert_eq!(pin.len(), 0);
        assert!(!pin.pop()); // Empty again
    }

    #[test]
    fn pin_input_take_resets() {
        let mut pin = PinInputState::new();
        pin.push_digit('1');
        pin.push_digit('2');
        pin.push_digit('3');
        pin.push_digit('4');
        let taken = pin.take();
        assert_eq!(taken, "1234");
        assert_eq!(pin.len(), 0);
        assert!(!pin.is_complete());
    }

    #[test]
    fn pin_input_display_dots() {
        let mut pin = PinInputState::new();
        assert_eq!(pin.display_dots(), "");
        pin.push_digit('1');
        assert_eq!(pin.display_dots(), "\u{25CF}");
        pin.push_digit('2');
        assert_eq!(pin.display_dots(), "\u{25CF} \u{25CF}");
        pin.push_digit('3');
        assert_eq!(pin.display_dots(), "\u{25CF} \u{25CF} \u{25CF}");
    }

    #[test]
    fn pin_input_clear() {
        let mut pin = PinInputState::new();
        pin.push_digit('1');
        pin.push_digit('2');
        pin.clear();
        assert_eq!(pin.len(), 0);
        assert!(!pin.is_complete());
    }

    #[test]
    fn pin_input_auto_submit_detection() {
        let mut pin = PinInputState::new();
        for ch in ['1', '2', '3', '4', '5'] {
            pin.push_digit(ch);
            assert!(!pin.is_full());
        }
        pin.push_digit('6');
        assert!(pin.is_full());
        // At 6 digits, caller should auto-submit
    }

    #[test]
    fn pin_input_is_complete_at_4() {
        let mut pin = PinInputState::new();
        for ch in ['1', '2', '3'] {
            pin.push_digit(ch);
        }
        assert!(!pin.is_complete()); // 3 digits: not enough
        pin.push_digit('4');
        assert!(pin.is_complete()); // 4 digits: enough for manual submit
    }
}
