//! Build script.
//!
//! PRH branding assets are *not* committed to this repository (see the
//! "Additional Terms" section of the README). CI downloads them into
//! `Assets/` before building; local builds without them get a transparent
//! placeholder logo and no custom exe icon.

use std::env;
use std::fs;
use std::path::Path;

/// 1x1 transparent PNG used when `Assets/prh_logo.png` is absent.
/// The app detects the 1x1 size at runtime and simply hides the logo.
const PLACEHOLDER_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x62, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

fn main() {
    println!("cargo:rerun-if-changed=Assets/prh_logo.png");
    println!("cargo:rerun-if-changed=Assets/AppIconNew.ico");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let logo_out = Path::new(&out_dir).join("prh_logo.png");

    // Stage the logo (real asset if present, placeholder otherwise) so the
    // source can unconditionally `include_bytes!` it from OUT_DIR.
    let logo_src = Path::new("Assets/prh_logo.png");
    if logo_src.exists() {
        fs::copy(logo_src, &logo_out).expect("failed to copy Assets/prh_logo.png");
    } else {
        fs::write(&logo_out, PLACEHOLDER_PNG).expect("failed to write placeholder logo");
    }

    // Embed the exe icon + version resource on Windows when the icon exists.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        if Path::new("Assets/AppIconNew.ico").exists() {
            res.set_icon("Assets/AppIconNew.ico");
        }
        res.set("ProductName", "ComputerInfo");
        res.set("CompanyName", "PRH IT");
        res.set(
            "FileDescription",
            "Displays system and printer information for IT support",
        );
        res.set(
            "LegalCopyright",
            "© 2026 PRH IT — GPL-3.0 (code only, branding excluded)",
        );
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to embed Windows resources: {e}");
        }
    }
}
