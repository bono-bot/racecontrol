//! v29.0 Phase 15: 3-tier escalation workflow for maintenance alerts.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum EscalationTier {
    Auto,       // Tier 1: auto-fix, log only
    Technician, // Tier 2: notify staff on duty
    Manager,    // Tier 3: escalate to Uday
}

/// Determine escalation tier based on severity and attempt count.
pub fn determine_escalation(
    severity: &str,
    auto_fix_attempts: u32,
    is_recurring: bool,
) -> EscalationTier {
    if severity == "Critical" {
        return EscalationTier::Manager;
    }
    if auto_fix_attempts == 0 && !is_recurring && severity != "High" {
        return EscalationTier::Auto;
    }
    // MMA-v29: Fixed — High with >2 attempts must escalate to Manager (was stuck at Technician
    // because `||` short-circuited: `severity == "High"` was always true for High, so the
    // `auto_fix_attempts > 2` branch was unreachable).
    if auto_fix_attempts > 2 {
        return EscalationTier::Manager;
    }
    if severity == "High" || is_recurring {
        return EscalationTier::Technician;
    }
    if auto_fix_attempts <= 2 {
        return EscalationTier::Technician;
    }
    EscalationTier::Manager
}

/// Format WhatsApp alert message based on tier.
pub fn format_alert_message(
    tier: &EscalationTier,
    pod_id: u8,
    component: &str,
    description: &str,
    severity: &str,
) -> String {
    let prefix = match tier {
        EscalationTier::Auto => return String::new(), // no message for auto-fix
        EscalationTier::Technician => "\u{26a0}\u{fe0f} MAINTENANCE ALERT",
        EscalationTier::Manager => "\u{1f6a8} CRITICAL ESCALATION",
    };
    format!(
        "{}\nPod: {}\nComponent: {}\nSeverity: {}\n{}\n\nAction required.",
        prefix, pod_id, component, severity, description
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_critical_always_manager() {
        assert_eq!(
            determine_escalation("Critical", 0, false),
            EscalationTier::Manager
        );
    }

    #[test]
    fn test_first_attempt_low_is_auto() {
        assert_eq!(
            determine_escalation("Low", 0, false),
            EscalationTier::Auto
        );
        assert_eq!(
            determine_escalation("Medium", 0, false),
            EscalationTier::Auto
        );
    }

    #[test]
    fn test_high_is_technician() {
        assert_eq!(
            determine_escalation("High", 0, false),
            EscalationTier::Technician
        );
    }

    #[test]
    fn test_recurring_escalates() {
        assert_eq!(
            determine_escalation("Medium", 0, true),
            EscalationTier::Technician
        );
    }

    #[test]
    fn test_many_attempts_escalates_to_manager() {
        assert_eq!(
            determine_escalation("Medium", 3, false),
            EscalationTier::Manager
        );
    }

    #[test]
    fn test_high_many_attempts_escalates_to_manager() {
        // MMA-v29: This was the bug — High + >2 attempts was stuck at Technician
        assert_eq!(
            determine_escalation("High", 3, false),
            EscalationTier::Manager
        );
        assert_eq!(
            determine_escalation("High", 5, true),
            EscalationTier::Manager
        );
    }

    #[test]
    fn test_low_many_attempts_escalates_to_manager() {
        assert_eq!(
            determine_escalation("Low", 3, false),
            EscalationTier::Manager
        );
    }

    #[test]
    fn test_auto_tier_empty_message() {
        let msg = format_alert_message(&EscalationTier::Auto, 1, "GPU", "Temp high", "Low");
        assert!(msg.is_empty());
    }

    #[test]
    fn test_technician_message_format() {
        let msg = format_alert_message(
            &EscalationTier::Technician,
            3,
            "Cooling",
            "Fan RPM low",
            "High",
        );
        assert!(msg.contains("MAINTENANCE ALERT"));
        assert!(msg.contains("Pod: 3"));
        assert!(msg.contains("Cooling"));
    }

    #[test]
    fn test_manager_message_format() {
        let msg = format_alert_message(
            &EscalationTier::Manager,
            5,
            "GPU",
            "GPU memory errors",
            "Critical",
        );
        assert!(msg.contains("CRITICAL ESCALATION"));
        assert!(msg.contains("Pod: 5"));
    }
}
