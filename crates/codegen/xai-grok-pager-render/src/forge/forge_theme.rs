//! Forge UI package theme — Oscura Midnight purple accents on a deep dark base.
//!
//! The purple, code, and background colors follow the `oscura-midnight` theme
//! so Forge keeps the same visual language as the rest of the pager.
//!
//! The package default that hides the shortcuts footer lives in the sibling
//! Forge policy module; keyboard behavior is intentionally theme-independent.

use ratatui::style::{Color, Modifier};

use crate::theme::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

// Forge uses the Oscura Midnight palette for its core surfaces, purple brand
// accents, and markdown/code rendering. Keep these values aligned with
// theme/oscura.rs rather than maintaining a second visual direction.
#[allow(dead_code)]
mod palette {
    use super::*;

    // Oscura Midnight backgrounds.
    pub const BASE: Color = rgb(3, 3, 4);
    pub const SURFACE: Color = rgb(4, 5, 7);
    pub const ELEVATED: Color = rgb(15, 18, 22);
    pub const HIGHLIGHT_MED: Color = rgb(36, 32, 52);
    pub const HIGHLIGHT_HIGH: Color = rgb(52, 48, 72);

    // Oscura Midnight text and muted ramp.
    pub const TEXT: Color = rgb(228, 228, 228);
    pub const TEXT_DIM: Color = rgb(190, 190, 190);
    pub const MUTED: Color = rgb(129, 134, 143);
    pub const SUBTLE: Color = rgb(94, 100, 108);

    // Oscura Midnight semantic/accent colors.
    pub const GOLD: Color = rgb(235, 217, 110);
    pub const RED: Color = rgb(220, 90, 100);
    pub const TEAL: Color = rgb(80, 180, 140);
    pub const AMBER: Color = rgb(241, 189, 0);
    pub const PURPLE: Color = rgb(155, 126, 206);
    pub const PURPLE_DIM: Color = rgb(110, 90, 154);
    pub const PURPLE_BRIGHT: Color = rgb(196, 167, 231);
    pub const CYAN: Color = rgb(125, 207, 223);

    pub const RED_DARK: Color = rgb(45, 15, 25);
    pub const GREEN_DARK: Color = rgb(10, 35, 30);
}
use palette::*;

impl Theme {
    /// Forge dark UI package palette, matched to Oscura Midnight.
    pub const fn forge() -> Self {
        Self {
            bg_base: BASE,
            bg_light: ELEVATED,
            bg_dark: SURFACE,
            bg_highlight: ELEVATED,
            bg_hover: HIGHLIGHT_MED,
            bg_terminal: BASE,

            accent_user: PURPLE_BRIGHT,
            accent_assistant: PURPLE,
            accent_thinking: PURPLE_BRIGHT,
            accent_tool: SUBTLE,
            accent_system: CYAN,
            accent_error: RED,
            accent_success: TEAL,
            accent_running: PURPLE_DIM,
            accent_skill: PURPLE,

            text_primary: TEXT,
            text_secondary: TEXT_DIM,

            gray_dim: SUBTLE,
            gray: MUTED,
            gray_bright: TEXT_DIM,

            command: GOLD,
            path: AMBER,
            running: CYAN,
            warning: GOLD,

            fuzzy_accent: PURPLE_BRIGHT,
            accent_plan: GOLD,
            accent_verify: PURPLE,
            accent_remember: rgb(139, 195, 74),

            selection_border: HIGHLIGHT_HIGH,
            prompt_border: HIGHLIGHT_MED,
            prompt_border_active: HIGHLIGHT_HIGH,
            hover_border: HIGHLIGHT_MED,

            accent_model: CYAN,

            scrollbar_bg: rgb(18, 16, 28),
            scrollbar_fg: HIGHLIGHT_HIGH,

            diff_delete_bg: RED_DARK,
            diff_delete_fg: RED,
            diff_insert_bg: GREEN_DARK,
            diff_insert_fg: TEAL,
            diff_equal_fg: MUTED,
            diff_gutter_fg: MUTED,

            bg_visual: HIGHLIGHT_MED,

            paste_bg: SURFACE,
            paste_fg: TEXT_DIM,
            paste_dim: MUTED,

            md_heading_h1: TEXT,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: PURPLE_BRIGHT,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: PURPLE,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: TEAL,
            md_heading_h4_mod: Modifier::BOLD.union(Modifier::ITALIC),
            md_heading_h5: GOLD,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: CYAN,
            md_heading_h6_mod: Modifier::BOLD,
            md_code: CYAN,
            md_task_checked: TEAL,
            md_task_unchecked: TEXT_DIM,
            md_muted: MUTED,
            md_code_bg: SURFACE,
            md_text: TEXT,
            link_fg: CYAN,
        }
    }
}
