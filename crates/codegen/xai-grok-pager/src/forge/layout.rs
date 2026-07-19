//! Forge layout helpers shared by agent overlays.

use ratatui::layout::Rect;

/// Return the full overlay canvas, stopping above the shortcuts bar when it
/// exists. Hidden-shortcuts themes use an empty `Rect`, so deriving the bottom
/// edge from `shortcuts.y` alone would collapse every modal to zero height.
pub(crate) fn overlay_area_above_shortcuts(area: Rect, shortcuts: Rect, bottom_inset: u16) -> Rect {
    let bottom = if shortcuts.height > 0 {
        shortcuts.y
    } else {
        area.bottom()
    };
    Rect {
        height: bottom.saturating_sub(area.y).saturating_sub(bottom_inset),
        ..area
    }
}
