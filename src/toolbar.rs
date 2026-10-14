//! Pixel-aligned toolbar layout and hit testing, independent of Win32 / GDI.
//! We keep native physical coordinates and share the exact same cell
//! definitions between painting, hover highlights and click dispatch.
use crate::geometry::{Point, Rect};

pub const TOOLBAR_WIDTH: i32 = 428;
pub const TOOLBAR_HEIGHT: i32 = 44;
pub const BUTTON_COUNT: usize = 11;

// Visual order resembles the WeChat desktop screenshot editor:
// annotation actions, history, red cancel and green accept. These IDs preserve
// v0.4's internal Win32 actions; do not bind behavior to visual position.
pub const ACTIONS: [usize; BUTTON_COUNT] = [
    2, // rectangle
    3, // ellipse
    4, // arrow
    5, // freehand
    7, // mosaic
    6, // text
    1, // selection
    8, // undo
    9, // redo
    11, // cancel
    10, // finish
];

pub const TITLES: [&str; BUTTON_COUNT] = [
    "矩形", "椭圆", "箭头", "画笔", "马赛克", "文字", "选区",
    "撤销 (Ctrl+Z)", "重做 (Ctrl+Y)", "取消 (Esc)", "完成 (Enter)",
];

pub fn rect_for(selection: Rect, bounds: Rect) -> Rect {
    let width = TOOLBAR_WIDTH.min(bounds.width().max(1));
    let height = TOOLBAR_HEIGHT.min(bounds.height().max(1));
    let x = selection.right.saturating_sub(width)
        .clamp(bounds.left, bounds.right.saturating_sub(width));
    let proposed = selection.bottom.saturating_add(8);
    let y = if proposed.saturating_add(height) <= bounds.bottom {
        proposed
    } else {
        selection.top.saturating_sub(height.saturating_add(8))
            .clamp(bounds.top, bounds.bottom.saturating_sub(height))
    };
    Rect { left: x, top: y, right: x + width, bottom: y + height }
}

fn compact(bar: Rect) -> bool {
    bar.width() < TOOLBAR_WIDTH
}

pub fn slot_rect(bar: Rect, slot: usize) -> Option<Rect> {
    if slot >= BUTTON_COUNT || bar.is_empty() { return None; }
    let small = compact(bar);
    let padding = if small { 1 } else { 8 };
    let group_spacing = if small { 0 } else { 8 };
    let usable = bar.width() - 2 * padding - group_spacing * 2;
    let cell = (usable / BUTTON_COUNT as i32).max(1);
    let group = if slot >= 9 { 2 } else if slot >= 7 { 1 } else { 0 };
    let left = bar.left + padding + slot as i32 * cell + group * group_spacing;
    let right = (left + cell).min(bar.right);
    let top = bar.top + (if bar.height() > 16 { 4 } else { 0 });
    let bottom = bar.bottom - (if bar.height() > 16 { 4 } else { 0 });
    (right > left && bottom > top).then_some(Rect { left, top, right, bottom })
}

pub fn slot_at(bar: Rect, p: Point) -> Option<usize> {
    if !bar.contains(p) { return None; }
    (0..BUTTON_COUNT).find(|&slot| slot_rect(bar, slot).is_some_and(|r| r.contains(p)))
}

pub fn action_at(bar: Rect, p: Point) -> Option<usize> {
    slot_at(bar, p).map(|index| ACTIONS[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Rect {
        Rect { left: -1920, top: -200, right: 1920, bottom: 1080 }
    }

    #[test]
    fn action_mapping_matches_real_toolbar_behavior() {
        let bar = rect_for(Rect { left: -500, top: 20, right: 700, bottom: 350 }, bounds());
        assert_eq!(bar.width(), TOOLBAR_WIDTH);
        assert_eq!(bar.height(), TOOLBAR_HEIGHT);
        for i in 0..BUTTON_COUNT {
            let cell = slot_rect(bar, i).unwrap();
            let mid = Point { x: (cell.left + cell.right) / 2, y: (cell.top + cell.bottom) / 2 };
            assert_eq!(slot_at(bar, mid), Some(i));
            assert_eq!(action_at(bar, mid), Some(ACTIONS[i]));
        }
        assert_eq!(ACTIONS[9], 11); // red X cancels
        assert_eq!(ACTIONS[10], 10); // green check finishes
    }

    #[test]
    fn separators_are_not_clickable() {
        let bar = rect_for(Rect { left: 0, top: 0, right: 600, bottom: 200 }, bounds());
        let left = slot_rect(bar, 6).unwrap();
        let right = slot_rect(bar, 7).unwrap();
        assert!(right.left > left.right);
        assert_eq!(slot_at(bar, Point { x: left.right + 2, y: bar.top + 16 }), None);
    }

    #[test]
    fn above_bottom_edge_and_negative_screen_coordinates() {
        let bar = rect_for(Rect { left: -1900, top: 950, right: -1700, bottom: 1070 }, bounds());
        assert!(bar.top >= bounds().top);
        assert!(bar.bottom <= 950);
        assert!(bar.left >= bounds().left);
    }

    #[test]
    fn compact_virtual_desktop_is_bounded() {
        let virtual_screen = Rect { left: -90, top: -30, right: 90, bottom: 70 };
        let selection = Rect { left: -75, top: -20, right: 80, bottom: 35 };
        let bar = rect_for(selection, virtual_screen);
        assert_eq!(bar.width(), 180);
        for index in 0..BUTTON_COUNT {
            let rect = slot_rect(bar, index).unwrap();
            assert!(rect.left >= virtual_screen.left && rect.right <= virtual_screen.right);
        }
        assert_eq!(slot_at(bar, Point { x: bar.right, y: bar.bottom }), None);
    }
}
