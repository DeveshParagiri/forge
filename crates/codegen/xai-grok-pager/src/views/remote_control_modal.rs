//! Compact, non-persistent Forge Remote pairing overlay.
//!
//! The gateway lifecycle is owned by the Forge bridge. This module owns only
//! transient terminal presentation: closing it never stops or revokes remote
//! access, and the secret-bearing URL never enters scrollback.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEventKind};
use qrcodegen::{QrCode, QrCodeEcc};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::forge::remote_control::{RemoteControlCommand, RemoteControlStatus};
use crate::theme::Theme;
use crate::views::modal_window::{
    self as mw, ModalSizing, ModalWindowConfig, ModalWindowState, Shortcut,
};

pub const COPY_REMOTE_URL_SHORTCUT: usize = 1;

pub struct RemoteControlModalState {
    pub window: ModalWindowState,
    pub command: RemoteControlCommand,
    pub session_label: String,
    pub status: Option<RemoteControlStatus>,
    qr_lines: Vec<String>,
    url_rects: Vec<Rect>,
}

impl RemoteControlModalState {
    pub fn loading(command: RemoteControlCommand, session_label: String) -> Self {
        Self {
            window: ModalWindowState::new(),
            command,
            session_label,
            status: None,
            qr_lines: Vec::new(),
            url_rects: Vec::new(),
        }
    }

    pub fn apply_status(&mut self, status: RemoteControlStatus) {
        self.qr_lines = status
            .remote_url
            .as_deref()
            .and_then(remote_qr_lines)
            .unwrap_or_default();
        self.status = Some(status);
    }

    pub fn remote_url(&self) -> Option<&str> {
        self.status.as_ref()?.remote_url.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteControlModalOutcome {
    CopyUrl,
    Unchanged,
}

pub fn handle_remote_control_modal_key(
    state: &RemoteControlModalState,
    key: &KeyEvent,
) -> RemoteControlModalOutcome {
    if key.modifiers.intersects(
        KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::SHIFT,
    ) {
        return RemoteControlModalOutcome::Unchanged;
    }
    match key.code {
        KeyCode::Char('c') if state.remote_url().is_some() => RemoteControlModalOutcome::CopyUrl,
        _ => RemoteControlModalOutcome::Unchanged,
    }
}

pub fn handle_remote_control_modal_mouse(
    state: &RemoteControlModalState,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> RemoteControlModalOutcome {
    if matches!(
        kind,
        MouseEventKind::Down(crossterm::event::MouseButton::Left)
    ) && state.url_rects.iter().any(|rect| {
        column >= rect.x
            && column < rect.x + rect.width
            && row >= rect.y
            && row < rect.y + rect.height
    }) {
        RemoteControlModalOutcome::CopyUrl
    } else {
        RemoteControlModalOutcome::Unchanged
    }
}

pub fn render_remote_control_modal(
    buf: &mut Buffer,
    area: Rect,
    state: &mut RemoteControlModalState,
    compact: bool,
    theme: &Theme,
) {
    state.url_rects.clear();
    let has_url = state.remote_url().is_some();
    let mut shortcuts = Vec::new();
    if has_url {
        shortcuts.push(Shortcut {
            label: "c copy link",
            clickable: true,
            id: COPY_REMOTE_URL_SHORTCUT,
        });
    }
    shortcuts.push(Shortcut {
        label: "Esc close",
        clickable: false,
        id: 0,
    });

    let sizing = ModalSizing {
        width_pct: 0.72,
        max_width: 110,
        min_width: 70,
        v_margin: 2,
        h_pad: 2,
        v_pad: 1,
        footer_lines: 2,
    }
    .with_compact(compact);

    let desired_height = desired_popup_height(area, &sizing, state);
    let outer_height = desired_height
        .saturating_add(sizing.v_margin.saturating_mul(2))
        .min(area.height);
    let modal_host = if area.height > outer_height {
        Rect {
            x: area.x,
            y: area.y + (area.height - outer_height) / 2,
            width: area.width,
            height: outer_height,
        }
    } else {
        area
    };
    let config = ModalWindowConfig {
        title: "Forge Remote",
        tabs: None,
        shortcuts: &shortcuts,
        sizing,
        fold_info: None,
    };
    let Some(mca) = mw::render_modal_window(buf, modal_host, &mut state.window, &config, theme)
    else {
        return;
    };

    match state.status.clone() {
        None => render_loading(buf, mca.content, state.command, &state.session_label, theme),
        Some(status) if status.remote_url.is_some() => {
            render_live(buf, mca.content, state, &status, theme)
        }
        Some(status) => render_non_live(buf, mca.content, state, &status, theme),
    }
}

fn render_loading(
    buf: &mut Buffer,
    area: Rect,
    command: RemoteControlCommand,
    session_label: &str,
    theme: &Theme,
) {
    let heading = match command {
        RemoteControlCommand::Start => "Starting phone access…",
        RemoteControlCommand::Status => "Checking phone access…",
        RemoteControlCommand::Stop => "Stopping phone access…",
    };
    render_wrapped_lines(
        buf,
        area,
        &[
            (heading, primary_bold(theme)),
            (session_label, Style::default().fg(theme.gray)),
            (
                "Checking Tailscale and this session’s scoped route.",
                Style::default().fg(theme.gray),
            ),
        ],
    );
}

fn render_non_live(
    buf: &mut Buffer,
    area: Rect,
    state: &RemoteControlModalState,
    status: &RemoteControlStatus,
    theme: &Theme,
) {
    let heading = match status.state.as_str() {
        "inactive" => "Not active",
        "route_unavailable" => "Private route unavailable",
        "tailscale_unavailable" => "Tailscale unavailable",
        "stopped" => "Stopped",
        _ => "Could not start phone access",
    };
    render_wrapped_lines(
        buf,
        area,
        &[
            (heading, primary_bold(theme)),
            (
                state.session_label.as_str(),
                Style::default().fg(theme.gray),
            ),
            (status.message.as_str(), Style::default().fg(theme.gray)),
        ],
    );
}

fn render_live(
    buf: &mut Buffer,
    area: Rect,
    state: &mut RemoteControlModalState,
    status: &RemoteControlStatus,
    theme: &Theme,
) {
    let Some(url) = status.remote_url.as_deref() else {
        return;
    };
    let qr_width = state
        .qr_lines
        .first()
        .map(|line| line.width() as u16)
        .unwrap_or(0);
    let qr_height = state.qr_lines.len() as u16;
    let split =
        qr_width > 0 && qr_height <= area.height && area.width >= qr_width.saturating_add(31);

    if split {
        let qr_area = Rect {
            x: area.x,
            y: area.y,
            width: qr_width,
            height: area.height,
        };
        render_qr(buf, qr_area, &state.qr_lines, theme);
        let info = Rect {
            x: area.x + qr_width + 3,
            y: area.y,
            width: area.width.saturating_sub(qr_width + 3),
            height: area.height,
        };
        render_live_info(buf, info, state, url, theme);
        return;
    }

    let info_rows = live_info_height(area.width, url).min(area.height);
    let info = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: info_rows,
    };
    render_live_info(buf, info, state, url, theme);
    let qr_y = area.y + info_rows.saturating_add(1);
    let remaining = area.y + area.height;
    if qr_width > 0 && qr_width <= area.width && qr_y.saturating_add(qr_height) <= remaining {
        render_qr(
            buf,
            Rect {
                x: area.x,
                y: qr_y,
                width: area.width,
                height: remaining.saturating_sub(qr_y),
            },
            &state.qr_lines,
            theme,
        );
    } else if info_rows < area.height {
        set_line(
            buf,
            area.x,
            area.y + info_rows,
            area.width,
            Line::from(Span::styled(
                "Enlarge the terminal to show the QR; press c to copy the link.",
                Style::default().fg(theme.gray),
            )),
        );
    }
}

fn render_live_info(
    buf: &mut Buffer,
    area: Rect,
    state: &mut RemoteControlModalState,
    url: &str,
    theme: &Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let mut y = area.y;
    let bottom = area.y + area.height;
    render_plain_line(buf, area, bottom, &mut y, "Ready", primary_bold(theme));
    render_plain_line(
        buf,
        area,
        bottom,
        &mut y,
        &state.session_label,
        Style::default().fg(theme.gray),
    );
    if y < bottom {
        y += 1;
    }
    render_plain_line(
        buf,
        area,
        bottom,
        &mut y,
        "Open on your phone",
        primary_bold(theme),
    );

    let wrapped_url = wrap_text(url, area.width);
    for line in wrapped_url {
        if y >= bottom {
            break;
        }
        let visible_width = (line.width() as u16).min(area.width);
        set_line(
            buf,
            area.x,
            y,
            area.width,
            Line::from(Span::styled(
                line,
                Style::default()
                    .fg(theme.accent_user)
                    .add_modifier(Modifier::UNDERLINED),
            )),
        );
        state.url_rects.push(Rect {
            x: area.x,
            y,
            width: visible_width,
            height: 1,
        });
        y += 1;
    }
}

fn render_plain_line(
    buf: &mut Buffer,
    area: Rect,
    bottom: u16,
    y: &mut u16,
    text: &str,
    style: Style,
) {
    if *y < bottom {
        set_line(
            buf,
            area.x,
            *y,
            area.width,
            Line::from(Span::styled(text, style)),
        );
        *y += 1;
    }
}

fn render_qr(buf: &mut Buffer, area: Rect, lines: &[String], _theme: &Theme) {
    if lines.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let qr_width = lines[0].width() as u16;
    let start_x = area.x + area.width.saturating_sub(qr_width) / 2;
    // QR decoders expect dark modules on a light field. Keep this independent
    // of the terminal theme: inverted light-on-dark codes are legal in theory
    // but are silently ignored by some iOS camera/scanner paths.
    let style = Style::default().fg(Color::Black).bg(Color::White);
    for (row, line) in lines.iter().take(area.height as usize).enumerate() {
        buf.set_string(start_x, area.y + row as u16, line, style);
    }
}

fn render_wrapped_lines(buf: &mut Buffer, area: Rect, sections: &[(&str, Style)]) {
    let mut y = area.y;
    let bottom = area.y + area.height;
    for (index, (text, style)) in sections.iter().enumerate() {
        if index > 0 && y < bottom {
            y += 1;
        }
        for line in wrap_text(text, area.width) {
            if y >= bottom {
                return;
            }
            set_line(
                buf,
                area.x,
                y,
                area.width,
                Line::from(Span::styled(line, *style)),
            );
            y += 1;
        }
    }
}

fn set_line(buf: &mut Buffer, x: u16, y: u16, width: u16, line: Line<'_>) {
    if width > 0 {
        buf.set_line(x, y, &line, width);
    }
}

fn primary_bold(theme: &Theme) -> Style {
    Style::default()
        .fg(theme.text_primary)
        .add_modifier(Modifier::BOLD)
}

fn live_info_height(width: u16, url: &str) -> u16 {
    4u16.saturating_add(wrap_text(url, width).len() as u16)
}

fn desired_popup_height(area: Rect, sizing: &ModalSizing, state: &RemoteControlModalState) -> u16 {
    let Some(url) = state.remote_url() else {
        return 12.min(area.height);
    };
    let qr_width = state
        .qr_lines
        .first()
        .map(|line| line.width() as u16)
        .unwrap_or(0);
    let qr_height = state.qr_lines.len() as u16;
    let max_width = area.width.saturating_sub(4).min(sizing.max_width);
    let preferred_width = (area.width as f32 * sizing.width_pct) as u16;
    let modal_width = preferred_width
        .min(max_width)
        .max(sizing.min_width)
        .min(area.width);
    let content_width = modal_width
        .saturating_sub(2)
        .saturating_sub(sizing.h_pad.saturating_mul(2));
    let split = content_width >= qr_width.saturating_add(31);
    let content_height = if split {
        qr_height.max(live_info_height(
            content_width.saturating_sub(qr_width + 3),
            url,
        ))
    } else {
        live_info_height(content_width, url)
            .saturating_add(1)
            .saturating_add(qr_height)
    };
    // Border (2), top pad (1), and footer (2).
    content_height.saturating_add(5).min(area.height)
}

fn wrap_text(text: &str, width: u16) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    textwrap::wrap(
        text,
        textwrap::Options::new(width as usize).break_words(true),
    )
    .into_iter()
    .map(|line| line.into_owned())
    .collect()
}

fn remote_qr_lines(url: &str) -> Option<Vec<String>> {
    let qr = QrCode::encode_text(url, QrCodeEcc::Medium).ok()?;
    let border = 4;
    let mut lines = Vec::new();
    for y in (-border..qr.size() + border).step_by(2) {
        let mut line = String::new();
        for x in -border..qr.size() + border {
            let upper = x >= 0 && y >= 0 && x < qr.size() && y < qr.size() && qr.get_module(x, y);
            let lower_y = y + 1;
            let lower = x >= 0
                && lower_y >= 0
                && x < qr.size()
                && lower_y < qr.size()
                && qr.get_module(x, lower_y);
            line.push(match (upper, lower) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        lines.push(line);
    }
    Some(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn live_state() -> RemoteControlModalState {
        let mut state = RemoteControlModalState::loading(
            RemoteControlCommand::Start,
            "Session: terminal fixes".into(),
        );
        state.apply_status(RemoteControlStatus::live(
            format!(
                "https://forge-mac.example-tailnet.ts.net/forge/{}/",
                "ab".repeat(32)
            ),
            std::time::Instant::now() + std::time::Duration::from_secs(8 * 60 * 60),
        ));
        state
    }

    #[test]
    fn qr_is_half_height_with_a_full_quiet_zone() {
        let state = live_state();
        let url = state.remote_url().unwrap();
        let encoded = QrCode::encode_text(url, QrCodeEcc::Medium).unwrap();
        assert_eq!(
            state.qr_lines.len(),
            ((encoded.size() + 8 + 1) / 2) as usize
        );
        assert!(
            state
                .qr_lines
                .iter()
                .all(|line| line.width() == (encoded.size() + 8) as usize)
        );

        for (row, line) in state.qr_lines.iter().enumerate() {
            let upper_y = -4 + row as i32 * 2;
            for (column, character) in line.chars().enumerate() {
                let x = -4 + column as i32;
                let expected_module = |y| {
                    x >= 0
                        && y >= 0
                        && x < encoded.size()
                        && y < encoded.size()
                        && encoded.get_module(x, y)
                };
                let actual = match character {
                    '█' => (true, true),
                    '▀' => (true, false),
                    '▄' => (false, true),
                    ' ' => (false, false),
                    other => panic!("unexpected QR cell {other:?}"),
                };
                assert_eq!(
                    actual,
                    (expected_module(upper_y), expected_module(upper_y + 1)),
                    "module mismatch at {column}, {row}",
                );
            }
        }
    }

    #[test]
    fn qr_renders_black_modules_on_white_including_quiet_zone() {
        let area = Rect::new(0, 0, 100, 40);
        let mut buf = Buffer::empty(area);
        let state = live_state();
        let qr_width = state.qr_lines[0].width() as u16;
        let qr_height = state.qr_lines.len() as u16;
        render_qr(&mut buf, area, &state.qr_lines, &Theme::current());
        let start_x = (area.width - qr_width) / 2;

        let quiet = &buf[(start_x, 0)];
        assert_eq!(quiet.symbol(), " ");
        assert_eq!(quiet.style().fg, Some(Color::Black));
        assert_eq!(quiet.style().bg, Some(Color::White));

        let dark = (0..qr_height)
            .flat_map(|y| (0..qr_width).map(move |x| (x, y)))
            .find(|(x, y)| buf[(start_x + *x, *y)].symbol() != " ")
            .expect("QR has a dark module");
        let dark_cell = &buf[(start_x + dark.0, dark.1)];
        assert_eq!(dark_cell.style().fg, Some(Color::Black));
        assert_eq!(dark_cell.style().bg, Some(Color::White));
    }

    #[test]
    fn render_is_a_centered_overlay_without_explanatory_copy() {
        let area = Rect::new(0, 0, 150, 50);
        let mut buf = Buffer::empty(area);
        let mut state = live_state();
        let url = state.remote_url().unwrap().to_owned();
        render_remote_control_modal(&mut buf, area, &mut state, false, &Theme::current());
        let text = (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        for needle in ["Forge Remote", "Ready", "copy link"] {
            assert!(text.contains(needle), "missing {needle:?} in:\n{text}");
        }
        for removed in [
            "Tailnet only; no public tunnel.",
            "Expires in",
            "`/rc stop` revokes access.",
            "Esc closes this window; the terminal stays active.",
        ] {
            assert!(!text.contains(removed), "found {removed:?} in:\n{text}");
        }
        assert!(text.contains('█') || text.contains('▀') || text.contains('▄'));
        assert_eq!(state.remote_url(), Some(url.as_str()));
        assert!(!state.url_rects.is_empty());
        let popup = state.window.popup_area.expect("popup rendered");
        assert!(popup.height < area.height);
        assert_eq!(popup.y, (area.height - popup.height) / 2);
    }

    #[test]
    fn copy_key_requires_a_live_url() {
        let loading =
            RemoteControlModalState::loading(RemoteControlCommand::Start, "Current session".into());
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(
            handle_remote_control_modal_key(&loading, &key),
            RemoteControlModalOutcome::Unchanged
        );
        assert_eq!(
            handle_remote_control_modal_key(&live_state(), &key),
            RemoteControlModalOutcome::CopyUrl
        );
    }
}
