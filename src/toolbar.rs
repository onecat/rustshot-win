//! Screenshot toolbar geometry based on the user-provided WeChat PC captures.
//! The actual icon and hit rectangles share these reference positions so hover,
//! disabled buttons and clicks cannot drift. All units are physical pixels.
use crate::geometry::{Point, Rect};

// v0.7 readability pass: 1000px full and 668px compact bar, capped by desktop.
pub const FULL_WIDTH: i32 = 1000;
pub const FULL_HEIGHT: i32 = 68;
pub const COMPACT_WIDTH: i32 = 668;
pub const COMPACT_HEIGHT: i32 = 54;

// Center coordinates below were measured at these original reference widths.
// Slot bounds are scaled to the current bar, so paint and hit tests agree.
const FULL_REFERENCE_WIDTH: i32 = 824;
const COMPACT_REFERENCE_WIDTH: i32 = 556;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Rectangle, Ellipse, Emoji, Arrow, Pen, Mosaic, Text, Translate, Ocr,
    Scroll, Undo, Save, Pin, Share, Cancel, Finish,
}

#[derive(Clone, Copy, Debug)]
pub struct Item {
    pub glyph: Glyph,
    pub label: &'static str,
    // None is intentionally a disabled placeholder, never a fake click.
    pub action: Option<usize>,
}

const FULL: [Item; 16] = [
    Item { glyph: Glyph::Rectangle, label: "矩形", action: Some(2) },
    Item { glyph: Glyph::Ellipse, label: "椭圆", action: Some(3) },
    Item { glyph: Glyph::Emoji, label: "表情（暂未实现）", action: None },
    Item { glyph: Glyph::Arrow, label: "箭头", action: Some(4) },
    Item { glyph: Glyph::Pen, label: "画笔", action: Some(5) },
    Item { glyph: Glyph::Mosaic, label: "马赛克", action: Some(7) },
    Item { glyph: Glyph::Text, label: "文字", action: Some(6) },
    Item { glyph: Glyph::Translate, label: "翻译（暂未实现）", action: None },
    Item { glyph: Glyph::Ocr, label: "提取文字（暂未实现）", action: None },
    Item { glyph: Glyph::Scroll, label: "滚动截图（暂未实现）", action: None },
    Item { glyph: Glyph::Undo, label: "撤销 Ctrl+Z", action: Some(8) },
    Item { glyph: Glyph::Save, label: "保存并完成", action: Some(10) },
    Item { glyph: Glyph::Pin, label: "钉住（暂未实现）", action: None },
    Item { glyph: Glyph::Share, label: "分享（暂未实现）", action: None },
    Item { glyph: Glyph::Cancel, label: "取消 Esc", action: Some(11) },
    Item { glyph: Glyph::Finish, label: "完成 Enter", action: Some(10) },
];

const COMPACT: [Item; 12] = [
    FULL[0], FULL[1], FULL[2], FULL[3], FULL[4], FULL[5], FULL[6],
    FULL[10], FULL[13], FULL[11], FULL[14], FULL[15],
];

// Measured icon centers from the user's 824px and 556px reference bars.
// Scale positions uniformly, preserving group gaps and divider placement.
const FULL_CENTERS: [i32; 16] =
    [38, 84, 130, 178, 225, 272, 320, 386, 437, 485, 550, 597, 645, 691, 737, 784];
const COMPACT_CENTERS: [i32; 12] =
    [38, 80, 123, 168, 210, 252, 296, 339, 402, 444, 488, 530];
const FULL_SEPARATORS: [(usize, usize); 2] = [(6, 7), (9, 10)];
const COMPACT_SEPARATORS: [(usize, usize); 2] = [(6, 7), (7, 8)];

pub fn is_compact(bounds: Rect) -> bool {
    bounds.width() < 1050
}

pub fn items(bar: Rect) -> &'static [Item] {
    if bar.height() <= COMPACT_HEIGHT { &COMPACT } else { &FULL }
}

pub fn separators(bar: Rect) -> &'static [(usize, usize)] {
    if bar.height() <= COMPACT_HEIGHT { &COMPACT_SEPARATORS } else { &FULL_SEPARATORS }
}

pub fn rect_for(selection: Rect, bounds: Rect) -> Rect {
    let small = is_compact(bounds);
    let standard_w = if small { COMPACT_WIDTH } else { FULL_WIDTH };
    let standard_h = if small { COMPACT_HEIGHT } else { FULL_HEIGHT };
    let width = standard_w.min(bounds.width().max(1));
    let height = standard_h.min(bounds.height().max(1));
    // The reference places the bar adjacent to the crop's bottom edge and
    // aligns its right edge to the crop's right edge when screen space allows.
    let x = selection.right.saturating_sub(width)
        .clamp(bounds.left, bounds.right.saturating_sub(width));
    let gap = if small { 4 } else { 12 };
    let below = selection.bottom.saturating_add(gap);
    let y = if below.saturating_add(height) <= bounds.bottom {
        below
    } else {
        selection.top.saturating_sub(height.saturating_add(8))
            .clamp(bounds.top, bounds.bottom.saturating_sub(height))
    };
    Rect { left: x, top: y, right: x + width, bottom: y + height }
}

pub fn slot_rect(bar: Rect, slot: usize) -> Option<Rect> {
    if bar.is_empty() { return None; }
    let small = bar.height() <= COMPACT_HEIGHT;
    let (centers, standard_width, cell_width): (&[i32], i32, i32) = if small {
        (&COMPACT_CENTERS, COMPACT_REFERENCE_WIDTH, 40)
    } else {
        (&FULL_CENTERS, FULL_REFERENCE_WIDTH, 46)
    };
    let center = *centers.get(slot)?;
    // Scale only for unusually narrow desktops, keeping exact pixels for
    // normal reference-sized bars.
    let left = bar.left + ((center - cell_width/2) as i64 * bar.width() as i64
        / standard_width as i64) as i32;
    let right = bar.left + ((center + cell_width/2) as i64 * bar.width() as i64
        / standard_width as i64) as i32;
    let inset = if bar.height() > 16 { 3 } else { 0 };
    let top = bar.top + inset;
    let bottom = bar.bottom - inset;
    (right > left && bottom > top).then_some(Rect { left, top, right, bottom })
}

pub fn slot_at(bar: Rect, point: Point) -> Option<usize> {
    if !bar.contains(point) { return None; }
    items(bar).iter().enumerate()
        .find(|(i, _)| slot_rect(bar, *i).is_some_and(|r| r.contains(point)))
        .map(|(i, _)| i)
}

// Secondary palette appears only for an active drawing/text tool.
// Returns a palette action: 30–34 = preset color, 35–37 = line 2/4/7 px.
pub const PALETTE_COLORS: usize = 5;
pub const PALETTE_OPTIONS: usize = 8;
pub fn palette_rect(bar: Rect, bounds: Rect) -> Rect {
    let width = 326.min(bounds.width().max(1));
    let height = 42.min(bounds.height().max(1));
    let left = bar.right.saturating_sub(width)
        .clamp(bounds.left, bounds.right.saturating_sub(width));
    let top = if bar.top.saturating_sub(height + 6) >= bounds.top {
        bar.top - height - 6
    } else {
        bar.bottom.saturating_add(6).min(bounds.bottom - height)
    };
    Rect { left, top, right:left+width, bottom:top+height }
}
pub fn palette_hit(bar: Rect, bounds: Rect, p: Point) -> Option<usize> {
    let r=palette_rect(bar,bounds);
    if !r.contains(p) { return None; }
    let cell = ((p.x - r.left) as i64 * PALETTE_OPTIONS as i64 / r.width() as i64) as usize;
    (cell < PALETTE_OPTIONS).then_some(cell)
}

pub fn action_at(bar: Rect, point: Point) -> Option<usize> {
    slot_at(bar, point).and_then(|slot| items(bar)[slot].action)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desktop(width: i32) -> Rect {
        Rect { left: -100, top: 0, right: width - 100, bottom: 1000 }
    }

    #[test]
    fn full_layout_matches_user_image_and_actions() {
        let bounds = desktop(1920);
        let sel = Rect { left: 100, top: 40, right: 1000, bottom: 500 };
        let bar = rect_for(sel, bounds);
        assert_eq!((bar.width(), bar.height(), bar.top), (1000, 68, 512));
        assert_eq!(items(bar).len(), 16);
        for (i, item) in items(bar).iter().enumerate() {
            let cell = slot_rect(bar, i).unwrap();
            let mid = Point { x: (cell.left+cell.right)/2, y: (cell.top+cell.bottom)/2 };
            assert_eq!(slot_at(bar, mid), Some(i));
            assert_eq!(action_at(bar, mid), item.action);
        }
        assert_eq!(items(bar)[14].glyph, Glyph::Cancel);
        assert_eq!(items(bar)[15].action, Some(10));
        assert_eq!(items(bar)[8].action, None);
    }

    #[test]
    fn compact_layout_matches_narrow_reference_and_has_real_functions() {
        let bounds = desktop(638);
        let sel = Rect { left: 20, top: 30, right: 505, bottom: 334 };
        let bar = rect_for(sel, bounds);
        assert_eq!(bar.width(), 638);
        assert_eq!(bar.height(), 54);
        assert_eq!(bar.top, 338);
        assert_eq!(items(bar).len(), 12);
        for (i, item) in items(bar).iter().enumerate() {
            let rect = slot_rect(bar, i).unwrap();
            let center = Point { x: (rect.left+rect.right)/2, y: (rect.top+rect.bottom)/2 };
            assert_eq!(action_at(bar, center), item.action);
        }
        assert_eq!(items(bar)[8].glyph, Glyph::Share);
        assert_eq!(items(bar)[9].glyph, Glyph::Save);
    }

    #[test]
    fn no_action_from_disabled_or_separator_gap() {
        let bar = rect_for(Rect { left: 10, top: 0, right: 900, bottom: 300 }, desktop(1920));
        let x = slot_rect(bar, 2).unwrap();
        assert_eq!(action_at(bar, Point { x: (x.left+x.right)/2, y: x.top+5 }), None);
        let before = slot_rect(bar, 6).unwrap();
        let after = slot_rect(bar, 7).unwrap();
        assert!(after.left > before.right);
        let gap = Point { x: (before.right+after.left)/2, y: (bar.top+bar.bottom)/2 };
        assert_eq!(action_at(bar, gap), None);
    }

    #[test]
    fn negative_coordinates_and_above_screen_bottom() {
        let bounds = Rect { left: -1920, top: -300, right: 2560, bottom: 1080 };
        let selected = Rect { left: -1200, top: 900, right: -400, bottom: 1060 };
        let bar = rect_for(selected, bounds);
        assert!(bar.top < selected.top);
        assert!(bar.left >= bounds.left && bar.right <= bounds.right);
    }

    #[test]
    fn tiny_desktop_still_keeps_hit_rectangles_bounded() {
        let bounds = Rect { left: -90, top: -30, right: 90, bottom: 70 };
        let selected = Rect { left: -85, top: -20, right: 80, bottom: 25 };
        let bar = rect_for(selected, bounds);
        assert_eq!(bar.width(), 180);
        for i in 0..items(bar).len() {
            let item = slot_rect(bar, i).unwrap();
            assert!(item.left >= bounds.left && item.right <= bounds.right);
        }
    }
    #[test]
    fn enlarged_toolbar_retains_hit_targets_on_screen_edges() {
        let bounds = desktop(1920);
        let selection = Rect { left: 1150, top: 100, right: 1800, bottom: 980 };
        let bar = rect_for(selection, bounds);
        assert_eq!((bar.width(), bar.height()), (1000, 68));
        assert!(bar.left >= bounds.left && bar.right <= bounds.right);
        assert!(bar.top < selection.top || bar.bottom <= bounds.bottom);
        for (index, item) in items(bar).iter().enumerate() {
            let hit = slot_rect(bar, index).unwrap();
            assert!(hit.width() >= 51, "icon hit target should grow with the bar");
            assert!(hit.left >= bar.left && hit.right <= bar.right);
            let center = Point { x: (hit.left + hit.right) / 2, y: (hit.top + hit.bottom) / 2 };
            assert_eq!(action_at(bar, center), item.action);
        }
    }

    #[test]
    fn palette_items_work_at_virtual_desktop_edges() {
        let bounds = Rect {left:-1400, top:-500, right:1920, bottom:1080};
        let selection=Rect {left:-1300, top:200, right:-300, bottom:1060};
        let bar=rect_for(selection,bounds);
        let palette=palette_rect(bar,bounds);
        assert!(palette.left >= bounds.left);
        assert!(palette.right <= bounds.right);
        for slot in 0..PALETTE_OPTIONS {
            let center=Point {
                x:palette.left + ((slot as i32 * 2 + 1)*palette.width())/(2*PALETTE_OPTIONS as i32),
                y:palette.top + palette.height()/2,
            };
            assert_eq!(palette_hit(bar,bounds,center), Some(slot));
        }
    }

}
