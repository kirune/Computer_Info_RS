//! Real Windows implementations, using hand-declared Win32 FFI.
//! Behavior deliberately mirrors the WPF original (PRH_Computer_Info).

#![allow(non_snake_case, clippy::upper_case_acronyms)]

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use crate::info::os_friendly_name;
use crate::printers::{PrinterInfo, PRINT_SERVER};

use super::format_short;

type BOOL = i32;
type DWORD = u32;
type HANDLE = *mut c_void;
type HGLOBAL = *mut c_void;
type HWND = *mut c_void;
type LPWSTR = *mut u16;
type LPCWSTR = *const u16;

const CF_UNICODETEXT: u32 = 13;
const GHND: u32 = 0x0042; // GMEM_MOVEABLE | GMEM_ZEROINIT
const TH32CS_SNAPPROCESS: DWORD = 0x0000_0002;
const PROCESS_QUERY_LIMITED_INFORMATION: DWORD = 0x1000;
const INVALID_HANDLE_VALUE: HANDLE = -1isize as HANDLE;
const PRINTER_ENUM_NAME: DWORD = 0x0000_0008;
const PRINTER_ATTRIBUTE_SHARED: DWORD = 0x0000_0008;
const ERROR_INSUFFICIENT_BUFFER: DWORD = 122;
const SW_SHOWNORMAL: i32 = 1;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Windows/.NET FILETIME epoch (1601-01-01) to Unix epoch offset, in 100ns ticks.
const FILETIME_UNIX_EPOCH_DIFF: u64 = 116_444_736_000_000_000;

#[repr(C)]
#[derive(Clone, Copy)]
struct FILETIME {
    dwLowDateTime: DWORD,
    dwHighDateTime: DWORD,
}

impl FILETIME {
    const ZERO: FILETIME = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };

    fn as_u64(&self) -> u64 {
        ((self.dwHighDateTime as u64) << 32) | self.dwLowDateTime as u64
    }

    /// Convert a UTC FILETIME to a local chrono timestamp.
    fn to_local(self) -> Option<chrono::DateTime<chrono::Local>> {
        let ticks = self.as_u64();
        if ticks < FILETIME_UNIX_EPOCH_DIFF {
            return None;
        }
        let unix_100ns = ticks - FILETIME_UNIX_EPOCH_DIFF;
        let secs = (unix_100ns / 10_000_000) as i64;
        let nanos = ((unix_100ns % 10_000_000) * 100) as u32;
        use chrono::TimeZone;
        chrono::Local.timestamp_opt(secs, nanos).single()
    }
}

#[repr(C)]
#[allow(dead_code)] // layout-only fields
struct OSVERSIONINFOW {
    dwOSVersionInfoSize: DWORD,
    dwMajorVersion: DWORD,
    dwMinorVersion: DWORD,
    dwBuildNumber: DWORD,
    dwPlatformId: DWORD,
    szCSDVersion: [u16; 128],
}

#[repr(C)]
#[allow(dead_code)] // layout-only fields
struct PROCESSENTRY32W {
    dwSize: DWORD,
    cntUsage: DWORD,
    th32ProcessID: DWORD,
    th32DefaultHeapID: usize,
    th32ModuleID: DWORD,
    cntThreads: DWORD,
    th32ParentProcessID: DWORD,
    pcPriClassBase: i32,
    dwFlags: DWORD,
    szExeFile: [u16; 260],
}

/// PRINTER_INFO_2W — only the fields up to `Attributes` are read.
#[repr(C)]
#[allow(dead_code)] // layout-only fields
struct PRINTER_INFO_2W {
    pServerName: LPWSTR,
    pPrinterName: LPWSTR,
    pShareName: LPWSTR,
    pPortName: LPWSTR,
    pDriverName: LPWSTR,
    pComment: LPWSTR,
    pLocation: LPWSTR,
    pDevMode: *mut c_void,
    pSepFile: LPWSTR,
    pPrintProcessor: LPWSTR,
    pDatatype: LPWSTR,
    pParameters: LPWSTR,
    pSecurityDescriptor: *mut c_void,
    Attributes: DWORD,
    Priority: DWORD,
    DefaultPriority: DWORD,
    StartTime: DWORD,
    UntilTime: DWORD,
    Status: DWORD,
    cJobs: DWORD,
    AveragePPM: DWORD,
}

#[link(name = "user32")]
extern "system" {
    fn OpenClipboard(hWndNewOwner: HWND) -> BOOL;
    fn CloseClipboard() -> BOOL;
    fn EmptyClipboard() -> BOOL;
    fn SetClipboardData(uFormat: u32, hMem: HANDLE) -> HANDLE;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> HGLOBAL;
    fn GlobalLock(hMem: HGLOBAL) -> *mut c_void;
    fn GlobalUnlock(hMem: HGLOBAL) -> BOOL;
    fn GlobalFree(hMem: HGLOBAL) -> HGLOBAL;
    fn GetTickCount64() -> u64;
    fn GetLastError() -> DWORD;
    fn CreateToolhelp32Snapshot(dwFlags: DWORD, th32ProcessID: DWORD) -> HANDLE;
    fn Process32FirstW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
    fn Process32NextW(hSnapshot: HANDLE, lppe: *mut PROCESSENTRY32W) -> BOOL;
    fn OpenProcess(dwDesiredAccess: DWORD, bInheritHandle: BOOL, dwProcessId: DWORD) -> HANDLE;
    fn GetProcessTimes(
        hProcess: HANDLE,
        lpCreationTime: *mut FILETIME,
        lpExitTime: *mut FILETIME,
        lpKernelTime: *mut FILETIME,
        lpUserTime: *mut FILETIME,
    ) -> BOOL;
    fn CloseHandle(hObject: HANDLE) -> BOOL;
}

#[link(name = "ntdll")]
extern "system" {
    fn RtlGetVersion(lpVersionInformation: *mut OSVERSIONINFOW) -> i32;
}

#[link(name = "winspool")]
extern "system" {
    fn GetDefaultPrinterW(pszBuffer: LPWSTR, pcchBuffer: *mut DWORD) -> BOOL;
    fn EnumPrintersW(
        Flags: DWORD,
        Name: LPCWSTR,
        Level: DWORD,
        pPrinterEnum: *mut u8,
        cbBuf: DWORD,
        pcbNeeded: *mut DWORD,
        pcReturned: *mut DWORD,
    ) -> BOOL;
}

#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: HWND,
        lpOperation: LPCWSTR,
        lpFile: LPCWSTR,
        lpParameters: LPCWSTR,
        lpDirectory: LPCWSTR,
        nShowCmd: i32,
    ) -> HANDLE;
}

/// NUL-terminated UTF-16 for FFI.
fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// UTF-16 pointer (NUL-terminated) to String; empty on null.
unsafe fn from_wide_ptr(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

pub fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Unknown".into())
}

pub fn current_user() -> String {
    std::env::var("USERNAME").unwrap_or_else(|_| "Unknown".into())
}

/// Earliest `explorer.exe` start time — same heuristic as the WPF app.
pub fn logon_time() -> String {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return "Unknown".into();
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as DWORD;
        let mut earliest: Option<u64> = None;

        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            let exe = from_wide_ptr(entry.szExeFile.as_ptr()).to_lowercase();
            if exe == "explorer.exe" {
                let proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, entry.th32ProcessID);
                if !proc.is_null() {
                    let (mut c, mut e, mut k, mut u) = (
                        FILETIME::ZERO,
                        FILETIME::ZERO,
                        FILETIME::ZERO,
                        FILETIME::ZERO,
                    );
                    if GetProcessTimes(proc, &mut c, &mut e, &mut k, &mut u) != 0 {
                        let t = c.as_u64();
                        earliest = Some(earliest.map_or(t, |cur| cur.min(t)));
                    }
                    CloseHandle(proc);
                }
            }
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);

        earliest
            .and_then(|ticks| {
                FILETIME {
                    dwLowDateTime: (ticks & 0xFFFF_FFFF) as DWORD,
                    dwHighDateTime: (ticks >> 32) as DWORD,
                }
                .to_local()
            })
            .map(format_short)
            .unwrap_or_else(|| "Unknown".into())
    }
}

/// Default printer name with any `\\server\` prefix stripped
/// (mirrors the WPF `GetDefaultPrinter`).
pub fn default_printer() -> String {
    unsafe {
        let mut len: DWORD = 0;
        GetDefaultPrinterW(std::ptr::null_mut(), &mut len);
        if len == 0 {
            return "Unknown".into();
        }
        let mut buf = vec![0u16; len as usize];
        if GetDefaultPrinterW(buf.as_mut_ptr(), &mut len) == 0 {
            return "Unknown".into();
        }
        let full = from_wide_ptr(buf.as_ptr());
        if full.is_empty() {
            return "Unknown".into();
        }
        full.rsplit('\\').next().unwrap_or(&full).to_string()
    }
}

pub fn os_name() -> String {
    unsafe {
        let mut info: OSVERSIONINFOW = std::mem::zeroed();
        info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as DWORD;
        if RtlGetVersion(&mut info) == 0 {
            os_friendly_name(info.dwMajorVersion, info.dwBuildNumber)
        } else {
            "Windows".into()
        }
    }
}

/// `now - uptime`, formatted short (mirrors the WPF `GetLastRebootTime`).
pub fn last_reboot_time() -> String {
    let uptime_ms = unsafe { GetTickCount64() };
    let boot = chrono::Local::now()
        - chrono::Duration::milliseconds(uptime_ms.min(i64::MAX as u64) as i64);
    format_short(boot)
}

/// Same heuristic as the WPF app: joined when `USERDOMAIN` is set and differs
/// from the machine name.
pub fn is_domain_joined() -> bool {
    let domain = std::env::var("USERDOMAIN").unwrap_or_default();
    !domain.is_empty() && !domain.eq_ignore_ascii_case(&computer_name())
}

/// Enumerate shared queues on [`PRINT_SERVER`].
pub fn query_shared_printers() -> Result<Vec<PrinterInfo>, String> {
    unsafe {
        let server = to_wide(PRINT_SERVER);
        let mut needed: DWORD = 0;
        let mut returned: DWORD = 0;

        // Size probe.
        let ok = EnumPrintersW(
            PRINTER_ENUM_NAME,
            server.as_ptr(),
            2,
            std::ptr::null_mut(),
            0,
            &mut needed,
            &mut returned,
        );
        if ok == 0 {
            let err = GetLastError();
            if err != ERROR_INSUFFICIENT_BUFFER {
                return Err(format!("EnumPrinters failed (error {err})"));
            }
        }
        if needed == 0 {
            return Ok(Vec::new());
        }

        // u64-backed buffer so the PRINTER_INFO_2W view is properly aligned.
        let mut buf = vec![0u64; needed.div_ceil(8) as usize];
        if EnumPrintersW(
            PRINTER_ENUM_NAME,
            server.as_ptr(),
            2,
            buf.as_mut_ptr() as *mut u8,
            needed,
            &mut needed,
            &mut returned,
        ) == 0
        {
            return Err(format!("EnumPrinters failed (error {})", GetLastError()));
        }

        let entries =
            std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_2W, returned as usize);
        let mut printers: Vec<PrinterInfo> = entries
            .iter()
            .filter(|p| p.Attributes & PRINTER_ATTRIBUTE_SHARED != 0)
            .map(|p| PrinterInfo::from_full_name(from_wide_ptr(p.pPrinterName)))
            .filter(|p| !p.full_name.is_empty())
            .collect();
        printers.sort_by(|a, b| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        });
        Ok(printers)
    }
}

/// One clipboard attempt via raw Win32, matching the WPF
/// `TrySetClipboardUnicodeNative`. The retry/backoff loop lives in the app.
pub fn set_clipboard_text(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }
        let result = (|| {
            if EmptyClipboard() == 0 {
                return false;
            }
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes = wide.len() * 2;
            let hglobal = GlobalAlloc(GHND, bytes);
            if hglobal.is_null() {
                return false;
            }
            let ptr = GlobalLock(hglobal);
            if ptr.is_null() {
                GlobalFree(hglobal);
                return false;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
            GlobalUnlock(hglobal);
            if SetClipboardData(CF_UNICODETEXT, hglobal).is_null() {
                GlobalFree(hglobal);
                return false;
            }
            true // system now owns hglobal
        })();
        CloseClipboard();
        result
    }
}

/// Map a shared printer with `rundll32 printui.dll,PrintUIEntry` — the same
/// mechanism as the WPF app (handles driver install elevation flows).
pub fn map_printer(full_unc: &str, set_default: bool) -> Result<(), String> {
    let run = |args: &[&str]| -> Result<(), String> {
        let status = Command::new("rundll32.exe")
            .args(args)
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| format!("failed to launch rundll32: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("rundll32 exited with {status}"))
        }
    };

    run(&["printui.dll,PrintUIEntry", "/in", "/n", full_unc])?;
    if set_default {
        run(&["printui.dll,PrintUIEntry", "/y", "/n", full_unc])?;
    }
    Ok(())
}

/// ShellExecute "open" on a URL / mailto link.
pub fn open_url(url: &str) -> Result<(), String> {
    let op = to_wide("open");
    let target = to_wide(url);
    let h = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if h as isize > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecute failed (code {})", h as isize))
    }
}

/// Prefer Chrome at the two standard install paths, then fall back to the
/// default handler (mirrors the WPF `EpicHelp_Click`).
pub fn open_url_prefer_chrome(url: &str) -> Result<(), String> {
    const CHROME_PATHS: [&str; 2] = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    ];
    for path in CHROME_PATHS {
        if Path::new(path).exists() {
            return Command::new(path)
                .arg(url)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("failed to launch Chrome: {e}"));
        }
    }
    open_url(url)
}

/// Trebuchet MS Bold from the system font directory (the WPF app used
/// `FontFamily="Trebuchet MS"` with `FontWeight="Bold"`).
pub fn load_ui_font() -> Option<Vec<u8>> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into());
    for file in ["trebucbd.ttf", "trebuc.ttf"] {
        let path = Path::new(&windir).join("Fonts").join(file);
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
    }
    None
}
