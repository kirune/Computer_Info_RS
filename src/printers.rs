//! Printer list model, 24h JSON cache, and live-search filtering.
//!
//! The cache file format is a plain JSON array of full UNC names
//! (`["\\\\PrintServer\\Foo", ...]`) — byte-compatible with the cache the
//! WPF version writes, so the two apps can share it.

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

/// How long a cached printer list stays valid (WPF: 24h).
pub const CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// The print server queried for shared queues (WPF: `\\PrintServer`).
#[cfg_attr(not(windows), allow(dead_code))]
pub const PRINT_SERVER: &str = r"\\PrintServer";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrinterInfo {
    /// `\\PrintServer\PrinterName` — used for mapping.
    pub full_name: String,
    /// `PrinterName` — used for display.
    pub display_name: String,
}

impl PrinterInfo {
    /// Build from a full UNC name, deriving the display name after the last `\`.
    pub fn from_full_name(full: impl Into<String>) -> Self {
        let full_name = full.into();
        let display_name = full_name
            .rsplit('\\')
            .next()
            .unwrap_or(full_name.as_str())
            .to_string();
        Self {
            full_name,
            display_name,
        }
    }
}

/// `%ProgramData%\PRH\HelpCard` (falls back to a temp dir off Windows).
pub fn cache_dir() -> PathBuf {
    #[cfg(windows)]
    {
        let base = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
        PathBuf::from(base).join("PRH").join("HelpCard")
    }
    #[cfg(not(windows))]
    {
        std::env::temp_dir().join("PRH").join("HelpCard")
    }
}

/// Full path of the printers cache file.
pub fn cache_file() -> PathBuf {
    cache_dir().join("printers.json")
}

/// Read the cache if it exists, parses, is non-empty, and is younger than the TTL.
pub fn read_cache() -> Option<Vec<PrinterInfo>> {
    read_cache_at(&cache_file(), SystemTime::now())
}

fn read_cache_at(path: &std::path::Path, now: SystemTime) -> Option<Vec<PrinterInfo>> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    if now.duration_since(modified).unwrap_or(Duration::MAX) >= CACHE_TTL {
        return None;
    }
    let json = std::fs::read_to_string(path).ok()?;
    let names: Vec<String> = serde_json::from_str(&json).ok()?;
    if names.is_empty() {
        return None;
    }
    Some(names.into_iter().map(PrinterInfo::from_full_name).collect())
}

/// Persist the full UNC names to the cache (best effort).
pub fn write_cache(printers: &[PrinterInfo]) {
    let names: Vec<&str> = printers.iter().map(|p| p.full_name.as_str()).collect();
    if let Ok(json) = serde_json::to_string(&names) {
        let dir = cache_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(cache_file(), json);
    }
}

/// Case-insensitive substring filter over display names
/// (mirrors the WPF `FilterPrinters`).
pub fn filter<'a>(printers: &'a [PrinterInfo], query: &str) -> Vec<&'a PrinterInfo> {
    if query.trim().is_empty() {
        return printers.iter().collect();
    }
    let q = query.to_lowercase();
    printers
        .iter()
        .filter(|p| p.display_name.to_lowercase().contains(&q))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<PrinterInfo> {
        [
            r"\\PrintServer\ER-Laser1",
            r"\\PrintServer\ICU-Zebra",
            r"\\PrintServer\Lab-Laser2",
        ]
        .into_iter()
        .map(PrinterInfo::from_full_name)
        .collect()
    }

    #[test]
    fn display_name_strips_server() {
        let p = PrinterInfo::from_full_name(r"\\PrintServer\ER-Laser1");
        assert_eq!(p.display_name, "ER-Laser1");
        let bare = PrinterInfo::from_full_name("LocalPrinter");
        assert_eq!(bare.display_name, "LocalPrinter");
    }

    #[test]
    fn filter_is_case_insensitive_substring() {
        let printers = sample();
        assert_eq!(filter(&printers, "laser").len(), 2);
        assert_eq!(filter(&printers, "ZEBRA").len(), 1);
        assert_eq!(filter(&printers, "").len(), 3);
        assert_eq!(filter(&printers, "   ").len(), 3);
        assert_eq!(filter(&printers, "nomatch").len(), 0);
    }

    #[test]
    fn cache_roundtrip_and_ttl() {
        let dir = std::env::temp_dir().join(format!("prh_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("printers.json");

        let printers = sample();
        let names: Vec<&str> = printers.iter().map(|p| p.full_name.as_str()).collect();
        std::fs::write(&file, serde_json::to_string(&names).unwrap()).unwrap();

        // Fresh cache is readable.
        let now = SystemTime::now();
        let loaded = read_cache_at(&file, now).unwrap();
        assert_eq!(loaded, printers);

        // Expired cache is rejected.
        let later = now + CACHE_TTL + Duration::from_secs(1);
        assert!(read_cache_at(&file, later).is_none());

        // Empty list is rejected.
        std::fs::write(&file, "[]").unwrap();
        assert!(read_cache_at(&file, now).is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
