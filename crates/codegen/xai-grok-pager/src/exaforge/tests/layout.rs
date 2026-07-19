use crate::exaforge::layout::overlay_area_above_shortcuts;
use ratatui::layout::Rect;

#[test]
fn hidden_shortcuts_use_the_screen_bottom_instead_of_zero() {
    let area = Rect::new(3, 5, 80, 30);
    let overlay = overlay_area_above_shortcuts(area, Rect::default(), 1);
    assert_eq!(overlay, Rect::new(3, 5, 80, 29));
}

#[test]
fn visible_shortcuts_still_bound_the_overlay() {
    let area = Rect::new(3, 5, 80, 30);
    let shortcuts = Rect::new(3, 32, 80, 1);
    let overlay = overlay_area_above_shortcuts(area, shortcuts, 1);
    assert_eq!(overlay, Rect::new(3, 5, 80, 26));
}
