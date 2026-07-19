//! Claude UI package theme — Claude Code dark prompt chrome on GrokNight base.
//!
//! Colors mirror Claude Code dark:
//! - `promptBorder` / `promptBorderShimmer` for input frame
//! - `userMessageBackground` for user-message / elevated fills
//!
//! Package behaviors (no shortcuts footer, Shift+Tab → effort) live on
//! [`super::ThemeKind::Claude`] — not in this palette alone.

use ratatui::style::{Color, Modifier};

use super::tokyonight::Theme;

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

// Start from the same gray ramp as GrokNight, then override chatbox chrome.
#[allow(dead_code)]
mod palette {
    use super::*;

    pub const BG: Color = rgb(10, 10, 10);
    pub const BG_DARK: Color = rgb(12, 12, 12);
    pub const BG_STORM_DARK: Color = rgb(17, 17, 17);
    pub const BG_STORM: Color = rgb(20, 20, 20);
    pub const BG_HIGHLIGHT: Color = rgb(36, 36, 36);

    pub const FG: Color = rgb(225, 225, 225);
    pub const FG_DARK: Color = rgb(200, 200, 200);
    pub const FG_GUTTER: Color = rgb(65, 65, 65);
    pub const COMMENT: Color = rgb(108, 108, 108);
    pub const DARK3: Color = rgb(90, 90, 90);
    pub const DARK5: Color = rgb(120, 120, 120);

    pub const BLUE: Color = rgb(122, 162, 247);
    pub const BLUE0: Color = rgb(61, 89, 161);
    pub const BLUE1: Color = rgb(58, 149, 171);
    pub const CYAN: Color = rgb(125, 207, 255);
    pub const GREEN: Color = rgb(158, 206, 106);
    pub const GREEN1: Color = rgb(115, 218, 202);
    pub const MAGENTA: Color = rgb(187, 154, 247);
    pub const ORANGE: Color = rgb(255, 158, 100);
    pub const PURPLE: Color = rgb(157, 124, 216);
    pub const RED: Color = rgb(247, 118, 142);
    pub const RED1: Color = rgb(219, 75, 75);
    pub const TEAL: Color = rgb(26, 188, 156);
    pub const YELLOW: Color = rgb(224, 175, 104);

    pub const RED_DARK: Color = rgb(66, 14, 20);
    pub const GREEN_DARK: Color = rgb(6, 56, 6);

    // Claude Code dark theme slots
    pub const USER_MSG_BG: Color = rgb(55, 55, 55);
    pub const USER_MSG_HOVER: Color = rgb(70, 70, 70);
    pub const PROMPT_BORDER: Color = rgb(136, 136, 136);
    pub const PROMPT_BORDER_ACTIVE: Color = rgb(166, 166, 166);
}
use palette::*;

impl Theme {
    /// Claude Code–inspired dark UI package palette.
    pub const fn claude() -> Self {
        Self {
            bg_base: BG_STORM,
            bg_light: USER_MSG_BG,
            bg_dark: rgb(28, 28, 28),
            bg_highlight: USER_MSG_BG,
            bg_hover: USER_MSG_HOVER,
            bg_terminal: BG,

            accent_user: FG_DARK,
            accent_assistant: MAGENTA,
            accent_thinking: MAGENTA,
            accent_tool: DARK5,
            accent_system: BLUE,
            accent_error: RED,
            accent_success: GREEN,
            accent_running: MAGENTA,
            accent_skill: BLUE,

            text_primary: FG,
            text_secondary: FG_DARK,

            gray_dim: rgb(88, 88, 88),
            gray: COMMENT,
            gray_bright: DARK5,

            command: YELLOW,
            path: ORANGE,
            running: CYAN,
            warning: YELLOW,

            fuzzy_accent: BLUE,

            accent_plan: rgb(255, 219, 141),
            accent_verify: rgb(187, 154, 247),
            accent_feedback: GREEN1,
            accent_remember: Color::Rgb(139, 195, 74),

            selection_border: rgb(60, 60, 65),
            prompt_border: PROMPT_BORDER,
            prompt_border_active: PROMPT_BORDER_ACTIVE,
            hover_border: rgb(30, 30, 34),

            accent_model: TEAL,

            scrollbar_bg: BG_STORM_DARK,
            scrollbar_fg: BG_HIGHLIGHT,

            diff_delete_bg: RED_DARK,
            diff_delete_fg: RED,
            diff_insert_bg: GREEN_DARK,
            diff_insert_fg: GREEN,
            diff_equal_fg: COMMENT,
            diff_gutter_fg: COMMENT,

            bg_visual: USER_MSG_BG,

            paste_bg: BG_STORM_DARK,
            paste_fg: FG_DARK,
            paste_dim: FG_GUTTER,

            md_heading_h1: TEAL,
            md_heading_h1_mod: Modifier::BOLD,
            md_heading_h2: BLUE,
            md_heading_h2_mod: Modifier::BOLD,
            md_heading_h3: PURPLE,
            md_heading_h3_mod: Modifier::BOLD,
            md_heading_h4: DARK5,
            md_heading_h4_mod: Modifier::BOLD,
            md_heading_h5: COMMENT,
            md_heading_h5_mod: Modifier::BOLD,
            md_heading_h6: DARK3,
            md_heading_h6_mod: Modifier::empty(),
            md_code: BLUE1,
            md_task_checked: GREEN,
            md_task_unchecked: FG_DARK,
            md_muted: COMMENT,
            md_code_bg: rgb(28, 28, 28),
            md_text: FG_DARK,
            link_fg: rgb(122, 166, 218),
        }
    }
}
