//! PRH palette, lifted verbatim from the WPF app's XAML.

use egui::Color32;

/// Window background (`#6E9C78`).
pub const BACKGROUND: Color32 = Color32::from_rgb(0x6E, 0x9C, 0x78);
/// Info cards + primary buttons (`#005C27`).
pub const CARD: Color32 = Color32::from_rgb(0x00, 0x5C, 0x27);
/// Primary button hover (`#458357`).
pub const CARD_HOVER: Color32 = Color32::from_rgb(0x45, 0x83, 0x57);
/// Neutral button (`#4D4D4A`) — the "Done"/"Cancel" buttons.
pub const NEUTRAL: Color32 = Color32::from_rgb(0x4D, 0x4D, 0x4A);
/// Neutral button hover (`#7e7e7c`).
pub const NEUTRAL_HOVER: Color32 = Color32::from_rgb(0x7E, 0x7E, 0x7C);
/// Foreground text (`#f5f5f5`).
pub const TEXT: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);
/// Footer / dark text (`#231F20`).
pub const TEXT_DARK: Color32 = Color32::from_rgb(0x23, 0x1F, 0x20);
/// Dropdown item hover (`#CDE5D2`).
pub const ITEM_HOVER: Color32 = Color32::from_rgb(0xCD, 0xE5, 0xD2);
/// Light field background (`#f5f5f5`).
pub const FIELD_BG: Color32 = Color32::from_rgb(0xF5, 0xF5, 0xF5);

/// Corner radius used by cards (WPF `CornerRadius="8"`).
pub const CARD_ROUNDING: u8 = 8;
/// Corner radius used by buttons/fields (WPF `CornerRadius="6"`).
pub const BUTTON_ROUNDING: u8 = 6;
