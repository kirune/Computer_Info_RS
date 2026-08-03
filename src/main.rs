//! PRH Computer Info — Rust rewrite of the WPF Help Card
//! (https://github.com/kirune/PRH_Computer_Info).

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod info;
mod platform;
mod printers;
mod theme;

/// Logo staged by `build.rs` (real asset in CI, 1x1 placeholder locally).
static LOGO_PNG: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/prh_logo.png"));

fn main() -> eframe::Result {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([450.0, 780.0])
        .with_min_inner_size([450.0, 400.0])
        .with_max_inner_size([450.0, 800.0])
        .with_resizable(false)
        .with_maximize_button(false);

    // Window/taskbar icon from the logo when the real asset was embedded.
    if let Ok(img) = image::load_from_memory(LOGO_PNG) {
        if img.width() > 1 && img.height() > 1 {
            let rgba = img.to_rgba8();
            viewport = viewport.with_icon(egui::IconData {
                width: rgba.width(),
                height: rgba.height(),
                rgba: rgba.into_raw(),
            });
        }
    }

    let options = eframe::NativeOptions {
        viewport,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "PRH I.T. HELP CARD",
        options,
        Box::new(|cc| Ok(Box::new(app::HelpCardApp::new(cc, LOGO_PNG)))),
    )
}
