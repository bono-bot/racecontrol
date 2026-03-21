//! rc-process-guard — Standalone process guard for James workstation (.27).
//!
//! Reports via HTTP POST to racecontrol — never via WebSocket (standing rule #2).
//! Reads config from C:\Users\bono\racingpoint\rc-process-guard.toml.

fn main() {}

// --------------------------------------------------------------------------
// Helper stubs (not yet implemented — tests below will fail RED)
// --------------------------------------------------------------------------

fn is_james_self_excluded(_name: &str) -> bool { todo!() }
fn is_james_critical(_name: &str) -> bool { todo!() }
fn parse_netstat_listening_james(_stdout: &str) -> Vec<(u16, u32)> { todo!() }
fn parse_schtasks_csv_james(_stdout: &str) -> Vec<(String, String)> { todo!() }

#[cfg(test)]
mod tests {
    use super::*;

    // is_james_self_excluded
    #[test]
    fn self_excluded_own_binary() {
        assert!(is_james_self_excluded("rc-process-guard.exe"));
    }

    #[test]
    fn self_excluded_notepad_false() {
        assert!(!is_james_self_excluded("notepad.exe"));
    }

    // is_james_critical
    #[test]
    fn critical_rc_agent() {
        assert!(is_james_critical("rc-agent.exe"));
    }

    #[test]
    fn critical_kiosk() {
        assert!(is_james_critical("kiosk.exe"));
    }

    #[test]
    fn critical_code_false() {
        assert!(!is_james_critical("code.exe"));
    }

    // parse_netstat_listening_james
    #[test]
    fn netstat_parse_basic() {
        let input = "  TCP    0.0.0.0:4444    0.0.0.0:0    LISTENING    1234\n";
        let result = parse_netstat_listening_james(input);
        assert_eq!(result, vec![(4444u16, 1234u32)]);
    }

    #[test]
    fn netstat_parse_skips_non_listening() {
        let input = "  TCP    0.0.0.0:8080    192.168.1.1:443    ESTABLISHED    5678\n";
        let result = parse_netstat_listening_james(input);
        assert!(result.is_empty());
    }

    #[test]
    fn netstat_parse_ipv6() {
        let input = "  TCP    [::]:9090    [::]:0    LISTENING    999\n";
        let result = parse_netstat_listening_james(input);
        assert_eq!(result, vec![(9090u16, 999u32)]);
    }

    // parse_schtasks_csv_james
    #[test]
    fn schtasks_header_skipped() {
        let input = "\"TaskName\",\"Status\",\"Run As User\"\n\"\\MyTask\",\"ReadyTask\",\"Ready\"\n";
        let result = parse_schtasks_csv_james(input);
        // Header line should be skipped
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn schtasks_microsoft_system_tasks_skipped() {
        let input = "\"\\Microsoft\\Windows\\Defrag\",\"IdleTask\",\"Ready\"\n\"\\MyTask\",\"MyViolation\",\"Ready\"\n";
        let result = parse_schtasks_csv_james(input);
        // Microsoft task should be skipped at caller level — parser returns both, caller filters
        // parse function returns all non-header, non-empty entries; caller filters \\Microsoft\\
        assert_eq!(result.len(), 2);
    }
}
