//! Development stubs for non-Windows platforms. The shipping target is
//! Windows; these exist so the UI and shared logic compile, run, and can be
//! visually iterated on anywhere.

use crate::printers::PrinterInfo;

pub fn computer_name() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "DEV-MACHINE".into())
}

pub fn current_user() -> String {
    std::env::var("USER").unwrap_or_else(|_| "devuser".into())
}

pub fn logon_time() -> String {
    "Unknown".into()
}

pub fn default_printer() -> String {
    "Unknown".into()
}

pub fn os_name() -> String {
    format!("{} (dev build)", std::env::consts::OS)
}

pub fn last_reboot_time() -> String {
    "Unknown".into()
}

pub fn is_domain_joined() -> bool {
    false
}

/// Pretend to query the print server.
pub fn query_shared_printers() -> Result<Vec<PrinterInfo>, String> {
    Ok(vec![
        PrinterInfo::from_full_name(r"\\PrintServer\Dev-Laser1"),
        PrinterInfo::from_full_name(r"\\PrintServer\Dev-Zebra"),
    ])
}

pub fn set_clipboard_text(text: &str) -> bool {
    eprintln!("[stub] clipboard set ({} chars)", text.len());
    true
}

pub fn map_printer(_full_unc: &str, _set_default: bool) -> Result<(), String> {
    Ok(())
}

pub fn open_url(url: &str) -> Result<(), String> {
    eprintln!("[stub] open url: {url}");
    Ok(())
}

pub fn open_url_prefer_chrome(url: &str) -> Result<(), String> {
    open_url(url)
}

/// Trebuchet MS bytes — not available off Windows.
pub fn load_ui_font() -> Option<Vec<u8>> {
    None
}
