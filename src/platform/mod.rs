//! Platform abstraction. All OS-specific behavior lives behind these
//! functions; Windows gets real implementations, everything else gets
//! development stubs so the UI can be built and checked anywhere.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
mod stub;
#[cfg(not(windows))]
pub use stub::*;

use crate::info::SystemInfo;

/// Gather everything shown on the main card.
pub fn load_system_info() -> SystemInfo {
    SystemInfo {
        computer_name: computer_name(),
        ip_addresses: ipv4_addresses(),
        current_user: current_user(),
        logon_time: logon_time(),
        default_printer: default_printer(),
        os: os_name(),
        last_reboot: last_reboot_time(),
    }
}

/// Non-loopback IPv4 addresses of "up" interfaces
/// (mirrors the WPF LINQ over `NetworkInterface.GetAllNetworkInterfaces()`).
pub fn ipv4_addresses() -> Vec<String> {
    let mut addrs: Vec<String> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| match i.addr.ip() {
            std::net::IpAddr::V4(v4) => Some(v4.to_string()),
            std::net::IpAddr::V6(_) => None,
        })
        .filter(|ip| !ip.starts_with("127."))
        .collect();
    addrs.sort();
    addrs.dedup();
    addrs
}

/// Format a local timestamp the way .NET's `"g"` (general short) pattern does
/// for en-US: `M/d/yyyy h:mm AM/PM`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn format_short(dt: chrono::DateTime<chrono::Local>) -> String {
    dt.format("%-m/%-d/%Y %-I:%M %p").to_string()
}
