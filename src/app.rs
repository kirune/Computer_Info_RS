//! The Help Card UI. Layout, palette, copy, and dialog behavior mirror the
//! WPF original (`MainWindow.xaml`, `AddPrinterWindow.xaml`, `AboutWindow.xaml`).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use egui::{Align2, Color32, CornerRadius, FontId, Frame, Margin, RichText, Sense, Vec2};
use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

use crate::info::SystemInfo;
use crate::platform;
use crate::printers::{self, PrinterInfo};
use crate::theme;

const EPIC_HELP_URL: &str =
    "https://spportal/SitePages/Employee-Education,-Training-and-how-tos.aspx?web=1";
const IT_EMAIL: &str = "it@pullmanregionalhospital.freshservice.com";
const REPO_URL: &str = "https://github.com/kirune/Computer_Info_RS";

/// Same escape set as .NET `Uri.EscapeDataString` (RFC 3986 unreserved kept).
const URI_ESCAPE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

/// Result of the background clipboard-retry thread.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CopyState {
    Idle,
    InProgress,
    /// Success — show "Copied!" until the stored instant.
    CopiedUntil(Instant),
    Failed,
}

/// Background printer-list load for the Add Printer dialog.
enum PrinterLoad {
    Loading,
    Ready(Vec<PrinterInfo>),
    Error(String),
}

/// Background `rundll32` mapping operation.
enum MapState {
    Idle,
    InProgress,
    Done(Result<(), String>),
}

struct AddPrinterDialog {
    off_domain: bool,
    load: Arc<Mutex<PrinterLoad>>,
    query: String,
    selected: Option<PrinterInfo>,
    set_default: bool,
    map: Arc<Mutex<MapState>>,
    /// Stacked warning shown over the dialog (title, message) — the WPF
    /// MessageBox-over-dialog equivalent.
    warn: Option<(String, String)>,
}

impl AddPrinterDialog {
    fn open(ctx: &egui::Context) -> Self {
        let off_domain = !platform::is_domain_joined();
        let load = Arc::new(Mutex::new(PrinterLoad::Loading));

        if off_domain {
            *load.lock().unwrap() = PrinterLoad::Ready(Vec::new());
        } else if let Some(cached) = printers::read_cache() {
            *load.lock().unwrap() = PrinterLoad::Ready(cached);
        } else {
            let load_bg = Arc::clone(&load);
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let result = match platform::query_shared_printers() {
                    Ok(list) => {
                        printers::write_cache(&list);
                        PrinterLoad::Ready(list)
                    }
                    Err(e) => PrinterLoad::Error(format!(
                        "Unable to query printers from PrintServer: {e}"
                    )),
                };
                *load_bg.lock().unwrap() = result;
                ctx.request_repaint();
            });
        }

        Self {
            off_domain,
            load,
            query: String::new(),
            selected: None,
            set_default: false,
            map: Arc::new(Mutex::new(MapState::Idle)),
            warn: None,
        }
    }
}

enum Dialog {
    AddPrinter(AddPrinterDialog),
    About,
    /// Simple message box replacement: (title, message).
    Message(String, String),
}

pub struct HelpCardApp {
    info: SystemInfo,
    /// Display override after mapping a new printer, e.g. `Foo (Default)`.
    printer_label: String,
    logo: Option<egui::TextureHandle>,
    logo_bytes: &'static [u8],
    copy_state: Arc<Mutex<CopyState>>,
    dialog: Option<Dialog>,
}

impl HelpCardApp {
    pub fn new(cc: &eframe::CreationContext<'_>, logo_bytes: &'static [u8]) -> Self {
        install_fonts(&cc.egui_ctx);
        let info = platform::load_system_info();
        let printer_label = info.default_printer.clone();
        Self {
            info,
            printer_label,
            logo: None,
            logo_bytes,
            copy_state: Arc::new(Mutex::new(CopyState::Idle)),
            dialog: None,
        }
    }

    fn logo_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if self.logo.is_none() {
            if let Ok(img) = image::load_from_memory(self.logo_bytes) {
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                // 1x1 means the build used the placeholder — hide the logo.
                if w > 1 && h > 1 {
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [w, h],
                        rgba.as_flat_samples().as_slice(),
                    );
                    self.logo =
                        Some(ctx.load_texture("prh_logo", image, egui::TextureOptions::LINEAR));
                }
            }
        }
        self.logo.clone()
    }

    // --- Actions -----------------------------------------------------------

    fn start_copy(&self, ctx: &egui::Context) {
        {
            let mut state = self.copy_state.lock().unwrap();
            if *state == CopyState::InProgress {
                return;
            }
            *state = CopyState::InProgress;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);

        let text = self.summary();
        let state = Arc::clone(&self.copy_state);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            // Same retry cadence as the WPF app: 10 tries, 80ms doubling.
            let mut delay = Duration::from_millis(80);
            let mut ok = false;
            for _ in 0..10 {
                if platform::set_clipboard_text(&text) {
                    ok = true;
                    break;
                }
                std::thread::sleep(delay);
                if delay < Duration::from_millis(800) {
                    delay *= 2;
                }
            }
            *state.lock().unwrap() = if ok {
                CopyState::CopiedUntil(Instant::now() + Duration::from_secs(2))
            } else {
                CopyState::Failed
            };
            ctx.request_repaint();
        });
    }

    fn contact_it(&mut self) {
        let subject = utf8_percent_encode("I.T. Help Request", URI_ESCAPE).to_string();
        let body_raw = format!("\n\n{}", self.summary());
        let body = utf8_percent_encode(&body_raw, URI_ESCAPE).to_string();
        let mailto = format!("mailto:{IT_EMAIL}?subject={subject}&body={body}");

        if platform::open_url(&mailto).is_err() {
            let _ = platform::set_clipboard_text(&body_raw);
            self.dialog = Some(Dialog::Message(
                "Email Error".into(),
                "Unable to open email client. Info copied to clipboard instead.".into(),
            ));
        }
    }

    fn epic_help(&mut self) {
        if let Err(e) = platform::open_url_prefer_chrome(EPIC_HELP_URL) {
            self.dialog = Some(Dialog::Message(
                "Error".into(),
                format!("Unable to open Epic help page: {e}"),
            ));
        }
    }

    fn summary(&self) -> String {
        let mut info = self.info.clone();
        info.default_printer = self.printer_label.clone();
        info.summary()
    }

    // --- UI helpers --------------------------------------------------------

    fn card_frame() -> Frame {
        Frame::new()
            .fill(theme::CARD)
            .corner_radius(CornerRadius::same(theme::CARD_ROUNDING))
            .inner_margin(Margin::same(4))
            .shadow(egui::epaint::Shadow {
                offset: [2, 2],
                blur: 6,
                spread: 0,
                color: Color32::from_black_alpha(102),
            })
    }

    fn info_card(ui: &mut egui::Ui, width: f32, text: &str, size: f32) {
        ui.vertical_centered(|ui| {
            Self::card_frame().show(ui, |ui| {
                ui.set_width(width);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(text).size(size).strong().color(theme::TEXT));
                });
            });
        });
        ui.add_space(2.0);
    }

    fn field_label(ui: &mut egui::Ui, text: &str) {
        ui.add_space(10.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(text).size(16.0).color(theme::TEXT));
        });
        ui.add_space(4.0);
    }

    /// Rounded button with per-button hover color, single- or multi-line label.
    fn pill_button(
        ui: &mut egui::Ui,
        size: Vec2,
        bg: Color32,
        hover: Color32,
        lines: &[(&str, f32)],
        enabled: bool,
    ) -> egui::Response {
        let sense = if enabled {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);
        if ui.is_rect_visible(rect) {
            let mut fill = if enabled && response.hovered() {
                hover
            } else {
                bg
            };
            if response.is_pointer_button_down_on() {
                fill = fill.gamma_multiply(0.8);
            }
            ui.painter()
                .rect_filled(rect, CornerRadius::same(theme::BUTTON_ROUNDING), fill);

            let total: f32 = lines.iter().map(|(_, s)| s + 4.0).sum();
            let mut y = rect.center().y - total / 2.0 + 2.0;
            for (text, sz) in lines {
                ui.painter().text(
                    egui::pos2(rect.center().x, y + sz / 2.0),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(*sz),
                    theme::TEXT,
                );
                y += sz + 4.0;
            }
        }
        if enabled && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        response
    }

    // --- Main card ---------------------------------------------------------

    fn main_ui(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(10.0);

            // Logo + title row, matching the WPF original's Grid: a 100px
            // logo column (image width capped at 80 after 10px margins on
            // each side, height following the image's aspect ratio rather
            // than a fixed height) with the title centered across the full
            // row width — not just the space left after the logo, which
            // would visibly shove the title off-center.
            const LOGO_MAX_WIDTH: f32 = 80.0;
            ui.horizontal(|ui| {
                let full_width = ui.available_width();
                let mut logo_w = 0.0;
                let mut row_h = 34.0;
                if let Some(logo) = self.logo_texture(ctx) {
                    let aspect = logo.size_vec2().x / logo.size_vec2().y.max(1.0);
                    let h = LOGO_MAX_WIDTH / aspect.max(0.01);
                    ui.add_space(10.0);
                    ui.add(egui::Image::new(&logo).fit_to_exact_size(Vec2::new(LOGO_MAX_WIDTH, h)));
                    logo_w = LOGO_MAX_WIDTH + 10.0;
                    row_h = h;
                }
                ui.allocate_ui_with_layout(
                    Vec2::new((full_width - 2.0 * logo_w).max(0.0), row_h),
                    egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        ui.label(
                            RichText::new("Need I.T. Help?")
                                .size(24.0)
                                .strong()
                                .color(theme::TEXT),
                        );
                    },
                );
            });
            ui.add_space(14.0);

            Self::field_label(ui, "Computer Name:");
            Self::info_card(ui, 260.0, &self.info.computer_name, 20.0);

            Self::field_label(ui, "IP Addresses:");
            if self.info.ip_addresses.is_empty() {
                Self::info_card(ui, 240.0, "None", 16.0);
            } else {
                for ip in self.info.ip_addresses.clone() {
                    Self::info_card(ui, 240.0, &ip, 16.0);
                }
            }

            Self::field_label(ui, "Current User:");
            Self::info_card(ui, 240.0, &self.info.current_user, 16.0);

            Self::field_label(ui, "Default Printer:");
            ui.vertical_centered(|ui| {
                let label = self.printer_label.clone();
                if Self::pill_button(
                    ui,
                    Vec2::new(300.0, 50.0),
                    theme::CARD,
                    theme::CARD_HOVER,
                    &[(label.as_str(), 16.0)],
                    true,
                )
                .clicked()
                {
                    self.dialog = Some(Dialog::AddPrinter(AddPrinterDialog::open(ctx)));
                }
            });

            Self::field_label(ui, "Operating System:");
            Self::info_card(ui, 280.0, &self.info.os, 16.0);

            Self::field_label(ui, "Last Reboot Time:");
            Self::info_card(ui, 280.0, &self.info.last_reboot, 16.0);

            Self::field_label(ui, "Need Epic help?");
            ui.vertical_centered(|ui| {
                if Self::pill_button(
                    ui,
                    Vec2::new(280.0, 70.0),
                    theme::CARD,
                    theme::CARD_HOVER,
                    &[
                        ("Providence: 855-415-8188", 16.0),
                        ("Informatics: 509-336-7677", 16.0),
                    ],
                    true,
                )
                .clicked()
                {
                    self.epic_help();
                }
            });

            // Action row.
            ui.add_space(20.0);
            let copy_state = *self.copy_state.lock().unwrap();
            ui.vertical_centered(|ui| {
                ui.horizontal(|ui| {
                    let row_w = 120.0 * 2.0 + 100.0 + 16.0 * 2.0;
                    ui.add_space((ui.available_width() - row_w).max(0.0) / 2.0);

                    let (copy_text, copy_enabled) = match copy_state {
                        CopyState::CopiedUntil(t) if Instant::now() < t => ("Copied!", false),
                        CopyState::InProgress => ("Copying…", false),
                        _ => ("Copy Info", true),
                    };
                    if Self::pill_button(
                        ui,
                        Vec2::new(120.0, 40.0),
                        theme::CARD,
                        theme::CARD_HOVER,
                        &[(copy_text, 18.0)],
                        copy_enabled,
                    )
                    .clicked()
                    {
                        self.start_copy(ctx);
                    }
                    ui.add_space(16.0);

                    if Self::pill_button(
                        ui,
                        Vec2::new(120.0, 40.0),
                        theme::CARD,
                        theme::CARD_HOVER,
                        &[("Contact IT", 18.0)],
                        true,
                    )
                    .clicked()
                    {
                        self.contact_it();
                    }
                    ui.add_space(16.0);

                    if Self::pill_button(
                        ui,
                        Vec2::new(100.0, 40.0),
                        theme::NEUTRAL,
                        theme::NEUTRAL_HOVER,
                        &[("Done", 18.0)],
                        true,
                    )
                    .clicked()
                    {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });

            // Keep repainting while the "Copied!" feedback is showing.
            match copy_state {
                CopyState::CopiedUntil(t) => {
                    let now = Instant::now();
                    if now >= t {
                        *self.copy_state.lock().unwrap() = CopyState::Idle;
                    } else {
                        ctx.request_repaint_after(t - now);
                    }
                }
                CopyState::Failed => {
                    *self.copy_state.lock().unwrap() = CopyState::Idle;
                    self.dialog = Some(Dialog::Message(
                        "Clipboard Error".into(),
                        "Unable to copy info to clipboard after several attempts.".into(),
                    ));
                }
                _ => {}
            }

            // Footer.
            ui.add_space(15.0);
            ui.horizontal(|ui| {
                ui.add_space(10.0);
                ui.label(
                    RichText::new(
                        "© 2026 PRH IT — Licensed under GPL-3.0 (code only, branding excluded)",
                    )
                    .size(11.0)
                    .color(theme::TEXT_DARK),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    if ui
                        .add(
                            egui::Label::new(
                                RichText::new("About")
                                    .size(10.0)
                                    .underline()
                                    .color(theme::TEXT_DARK),
                            )
                            .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        self.dialog = Some(Dialog::About);
                    }
                });
            });
            ui.add_space(10.0);
        });
    }

    // --- Dialogs -----------------------------------------------------------

    fn dialog_frame() -> Frame {
        Frame::new()
            .fill(theme::BACKGROUND)
            .corner_radius(CornerRadius::same(theme::CARD_ROUNDING))
            .inner_margin(Margin::same(20))
            .shadow(egui::epaint::Shadow {
                offset: [3, 3],
                blur: 12,
                spread: 0,
                color: Color32::from_black_alpha(140),
            })
    }

    fn show_dialog(&mut self, ctx: &egui::Context) {
        let Some(dialog) = self.dialog.take() else {
            return;
        };
        match dialog {
            Dialog::Message(title, message) => {
                let mut close = false;
                let modal = egui::Modal::new(egui::Id::new("message"))
                    .frame(Self::dialog_frame())
                    .show(ctx, |ui| {
                        ui.set_width(320.0);
                        ui.vertical_centered(|ui| {
                            ui.label(RichText::new(&title).size(18.0).strong().color(theme::TEXT));
                            ui.add_space(10.0);
                            ui.label(RichText::new(&message).size(14.0).color(theme::TEXT));
                            ui.add_space(16.0);
                            if Self::pill_button(
                                ui,
                                Vec2::new(100.0, 36.0),
                                theme::CARD,
                                theme::CARD_HOVER,
                                &[("OK", 16.0)],
                                true,
                            )
                            .clicked()
                            {
                                close = true;
                            }
                        });
                    });
                if !close && !modal.should_close() {
                    self.dialog = Some(Dialog::Message(title, message));
                }
            }
            Dialog::About => {
                let mut close = false;
                let modal = egui::Modal::new(egui::Id::new("about"))
                    .frame(Self::dialog_frame())
                    .show(ctx, |ui| {
                        ui.set_width(370.0);
                        ui.label(RichText::new("ComputerInfo").size(20.0).strong().color(theme::TEXT));
                        ui.add_space(10.0);
                        ui.label(
                            RichText::new(
                                "This tool displays basic system information and provides shortcuts for IT support.",
                            )
                            .size(14.0)
                            .color(theme::TEXT),
                        );
                        ui.add_space(12.0);
                        ui.label(RichText::new("Source Code:").size(14.0).strong().color(theme::TEXT));
                        ui.hyperlink_to(RichText::new(REPO_URL).size(13.0), REPO_URL);
                        ui.add_space(10.0);
                        ui.label(RichText::new("License:").size(14.0).strong().color(theme::TEXT));
                        ui.label(
                            RichText::new(
                                "This program is licensed under the GNU General Public License v3.0 (GPL-3.0).",
                            )
                            .size(14.0)
                            .color(theme::TEXT),
                        );
                        ui.add_space(10.0);
                        ui.label(RichText::new("Instructions:").size(14.0).strong().color(theme::TEXT));
                        ui.label(
                            RichText::new(
                                "• Copy system info: Click 'Copy Info'.\n\
                                 • Contact IT: Opens email with system info.\n\
                                 • Add Printer: Choose a shared printer, optionally set it as default.\n\
                                 • Epic Help: Opens Providence/Informatics support page.",
                            )
                            .size(14.0)
                            .color(theme::TEXT),
                        );
                        ui.add_space(14.0);
                        ui.vertical_centered(|ui| {
                            if Self::pill_button(
                                ui,
                                Vec2::new(100.0, 36.0),
                                theme::NEUTRAL,
                                theme::NEUTRAL_HOVER,
                                &[("Close", 16.0)],
                                true,
                            )
                            .clicked()
                            {
                                close = true;
                            }
                        });
                    });
                if !close && !modal.should_close() {
                    self.dialog = Some(Dialog::About);
                }
            }
            Dialog::AddPrinter(mut state) => {
                if self.show_add_printer(ctx, &mut state) {
                    self.dialog = Some(Dialog::AddPrinter(state));
                }
            }
        }
    }

    /// Returns `true` while the dialog should stay open.
    fn show_add_printer(&mut self, ctx: &egui::Context, state: &mut AddPrinterDialog) -> bool {
        // Handle a finished mapping first.
        let map_done = {
            let mut map = state.map.lock().unwrap();
            match &*map {
                MapState::Done(result) => {
                    let r = result.clone();
                    *map = MapState::Idle;
                    Some(r)
                }
                _ => None,
            }
        };
        if let Some(result) = map_done {
            match result {
                Ok(()) => {
                    if let Some(sel) = &state.selected {
                        self.printer_label = if state.set_default {
                            format!("{} (Default)", sel.display_name)
                        } else {
                            sel.display_name.clone()
                        };
                    }
                    return false; // close
                }
                Err(e) => {
                    // Message dialog replaces the printer dialog.
                    self.dialog = Some(Dialog::Message(
                        "Error".into(),
                        format!("Failed to map printer: {e}"),
                    ));
                    return false;
                }
            }
        }

        let mapping = matches!(*state.map.lock().unwrap(), MapState::InProgress);

        let mut keep_open = true;
        let warn_active = state.warn.is_some();

        let modal = egui::Modal::new(egui::Id::new("add_printer"))
            .frame(Self::dialog_frame())
            .show(ctx, |ui| {
                ui.set_width(350.0);

                let (instruction, printers): (String, Vec<PrinterInfo>) = {
                    let load = state.load.lock().unwrap();
                    match &*load {
                        _ if state.off_domain => {
                            ("Not on a domain — no printers available".into(), Vec::new())
                        }
                        PrinterLoad::Loading => ("Loading printers…".into(), Vec::new()),
                        PrinterLoad::Error(e) => (e.clone(), Vec::new()),
                        PrinterLoad::Ready(list) if list.is_empty() => {
                            ("No printers available to add".into(), Vec::new())
                        }
                        PrinterLoad::Ready(list) => {
                            ("Enter or select a printer:".into(), list.clone())
                        }
                    }
                };
                let have_printers = !printers.is_empty();

                ui.vertical_centered(|ui| {
                    ui.label(RichText::new(&instruction).size(16.0).color(theme::TEXT));
                });
                ui.add_space(8.0);

                // Live-search field + filtered list (the WPF editable ComboBox).
                ui.vertical_centered(|ui| {
                    ui.scope(|ui| {
                        ui.visuals_mut().extreme_bg_color = theme::FIELD_BG;
                        ui.visuals_mut().override_text_color = Some(theme::TEXT_DARK);
                        ui.add_enabled(
                            have_printers && !mapping,
                            egui::TextEdit::singleline(&mut state.query)
                                .desired_width(300.0)
                                .hint_text("Type to search…"),
                        );
                    });
                });
                ui.add_space(6.0);

                if have_printers {
                    let filtered: Vec<PrinterInfo> = printers::filter(&printers, &state.query)
                        .into_iter()
                        .cloned()
                        .collect();
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for printer in &filtered {
                                let is_selected = state
                                    .selected
                                    .as_ref()
                                    .is_some_and(|s| s.full_name == printer.full_name);
                                let (rect, resp) = ui.allocate_exact_size(
                                    Vec2::new(ui.available_width(), 26.0),
                                    Sense::click(),
                                );
                                let bg = if is_selected {
                                    theme::BACKGROUND
                                } else if resp.hovered() {
                                    theme::ITEM_HOVER
                                } else {
                                    theme::FIELD_BG
                                };
                                let fg = if is_selected {
                                    theme::TEXT
                                } else {
                                    theme::TEXT_DARK
                                };
                                ui.painter().rect_filled(
                                    rect.shrink(1.0),
                                    CornerRadius::same(theme::BUTTON_ROUNDING),
                                    bg,
                                );
                                ui.painter().text(
                                    egui::pos2(rect.left() + 8.0, rect.center().y),
                                    Align2::LEFT_CENTER,
                                    &printer.display_name,
                                    FontId::proportional(14.0),
                                    fg,
                                );
                                if resp.clicked() && !mapping {
                                    state.selected = Some(printer.clone());
                                    state.query = printer.display_name.clone();
                                }
                            }
                            if filtered.is_empty() {
                                ui.vertical_centered(|ui| {
                                    ui.label(
                                        RichText::new("No matches").size(13.0).color(theme::TEXT),
                                    );
                                });
                            }
                        });
                    ui.add_space(12.0);

                    ui.vertical_centered(|ui| {
                        ui.scope(|ui| {
                            ui.visuals_mut().override_text_color = Some(theme::TEXT);
                            ui.add_enabled(
                                !mapping,
                                egui::Checkbox::new(
                                    &mut state.set_default,
                                    RichText::new("Set as default printer").size(14.0),
                                ),
                            );
                        });
                    });
                }

                ui.add_space(14.0);
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        let row_w = 100.0 * 2.0 + 16.0;
                        ui.add_space((ui.available_width() - row_w).max(0.0) / 2.0);

                        let add_label = if mapping { "Adding…" } else { "Add" };
                        if Self::pill_button(
                            ui,
                            Vec2::new(100.0, 36.0),
                            theme::CARD,
                            theme::CARD_HOVER,
                            &[(add_label, 16.0)],
                            !mapping,
                        )
                        .clicked()
                        {
                            if state.off_domain || !have_printers {
                                keep_open = false; // mirrors WPF: close, no action
                            } else {
                                // Accept the clicked selection, or an exact typed match.
                                let chosen = state.selected.clone().or_else(|| {
                                    printers
                                        .iter()
                                        .find(|p| {
                                            p.display_name.eq_ignore_ascii_case(state.query.trim())
                                        })
                                        .cloned()
                                });
                                match chosen {
                                    None => {
                                        state.warn = Some((
                                            "Invalid Printer".into(),
                                            "Please select a valid printer from the list.".into(),
                                        ));
                                    }
                                    Some(printer) => {
                                        state.selected = Some(printer.clone());
                                        *state.map.lock().unwrap() = MapState::InProgress;
                                        let map = Arc::clone(&state.map);
                                        let set_default = state.set_default;
                                        let ctx2 = ctx.clone();
                                        std::thread::spawn(move || {
                                            let r = platform::map_printer(
                                                &printer.full_name,
                                                set_default,
                                            );
                                            *map.lock().unwrap() = MapState::Done(r);
                                            ctx2.request_repaint();
                                        });
                                    }
                                }
                            }
                        }
                        ui.add_space(16.0);
                        if Self::pill_button(
                            ui,
                            Vec2::new(100.0, 36.0),
                            theme::NEUTRAL,
                            theme::NEUTRAL_HOVER,
                            &[("Cancel", 16.0)],
                            !mapping,
                        )
                        .clicked()
                        {
                            keep_open = false;
                        }
                    });
                });
            });

        // Stacked warning over the dialog (mirrors the WPF MessageBox). Its
        // backdrop intercepts input, so the dialog beneath is inert while shown.
        if let Some((title, msg)) = state.warn.clone() {
            let mut clear = false;
            let warn_modal = egui::Modal::new(egui::Id::new("add_printer_warn"))
                .frame(Self::dialog_frame())
                .show(ctx, |ui| {
                    ui.set_width(300.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(&title).size(18.0).strong().color(theme::TEXT));
                        ui.add_space(10.0);
                        ui.label(RichText::new(&msg).size(14.0).color(theme::TEXT));
                        ui.add_space(16.0);
                        if Self::pill_button(
                            ui,
                            Vec2::new(100.0, 36.0),
                            theme::CARD,
                            theme::CARD_HOVER,
                            &[("OK", 16.0)],
                            true,
                        )
                        .clicked()
                        {
                            clear = true;
                        }
                    });
                });
            if clear || warn_modal.should_close() {
                state.warn = None;
            }
            return true; // dialog stays open beneath the warning
        }

        // While the warning was up this frame, ignore the lower modal's
        // close signals (ESC/backdrop belong to the warning modal).
        if warn_active {
            return true;
        }

        keep_open && (mapping || !modal.should_close())
    }
}

impl eframe::App for HelpCardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(Frame::new().fill(theme::BACKGROUND))
            .show(ctx, |ui| {
                self.main_ui(ctx, ui);
            });
        self.show_dialog(ctx);
    }
}

/// Load Trebuchet MS (Bold) if available, matching the WPF typography.
fn install_fonts(ctx: &egui::Context) {
    if let Some(bytes) = platform::load_ui_font() {
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "trebuchet".to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(bytes)),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "trebuchet".to_owned());
        ctx.set_fonts(fonts);
    }
}
