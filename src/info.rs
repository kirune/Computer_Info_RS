//! System information model and the "Copy Info" / email body text builder.

/// Snapshot of everything the main card displays.
#[derive(Debug, Clone, Default)]
pub struct SystemInfo {
    pub computer_name: String,
    pub ip_addresses: Vec<String>,
    pub current_user: String,
    pub logon_time: String,
    pub default_printer: String,
    pub os: String,
    pub last_reboot: String,
}

impl SystemInfo {
    /// Multi-line summary — format matches the WPF `GetSystemInfoString()` exactly.
    pub fn summary(&self) -> String {
        format!(
            "Computer Name: {}\nUser: {}\nLogon Time: {}\nPrinter: {}\nOS: {}\nLast Reboot: {}\nIP(s): {}",
            self.computer_name,
            self.current_user,
            self.logon_time,
            self.default_printer,
            self.os,
            self.last_reboot,
            self.ip_addresses.join(", "),
        )
    }
}

/// Friendly OS name from a Windows build number
/// (mirrors the WPF `GetOsFriendlyName` switch).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn os_friendly_name(major: u32, build: u32) -> String {
    let name = match major {
        10 if build >= 22000 => "Windows 11",
        10 => "Windows 10",
        _ => "Windows",
    };
    format!("{name} (Build {build})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_matches_wpf_format() {
        let info = SystemInfo {
            computer_name: "PRH-PC01".into(),
            ip_addresses: vec!["172.27.1.10".into(), "10.0.0.5".into()],
            current_user: "10145".into(),
            logon_time: "8/3/2026 7:02 AM".into(),
            default_printer: "ER-Laser1".into(),
            os: "Windows 11 (Build 26100)".into(),
            last_reboot: "8/1/2026 3:00 AM".into(),
        };
        assert_eq!(
            info.summary(),
            "Computer Name: PRH-PC01\nUser: 10145\nLogon Time: 8/3/2026 7:02 AM\nPrinter: ER-Laser1\nOS: Windows 11 (Build 26100)\nLast Reboot: 8/1/2026 3:00 AM\nIP(s): 172.27.1.10, 10.0.0.5"
        );
    }

    #[test]
    fn os_names() {
        assert_eq!(os_friendly_name(10, 26100), "Windows 11 (Build 26100)");
        assert_eq!(os_friendly_name(10, 19045), "Windows 10 (Build 19045)");
        assert_eq!(os_friendly_name(6, 9600), "Windows (Build 9600)");
    }
}
