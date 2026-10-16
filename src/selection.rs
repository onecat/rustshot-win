//! Eight-way selection handles in virtual-desktop physical pixels.
//! The rectangle is half-open. We don't allow flipped or zero-area selections.
use crate::geometry::{Point, Rect};

pub const MIN_SIZE: i32 = 4;
pub const HANDLE_RADIUS: i32 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    NorthWest, North, NorthEast, East, SouthEast, South, SouthWest, West,
}

impl Handle {
    pub const ALL: [Self; 8] = [
        Self::NorthWest, Self::North, Self::NorthEast, Self::East,
        Self::SouthEast, Self::South, Self::SouthWest, Self::West,
    ];

    pub fn center(self, r: Rect) -> Point {
        let mid_x = r.left + (r.right - r.left) / 2;
        let mid_y = r.top + (r.bottom - r.top) / 2;
        match self {
            Self::NorthWest => Point { x: r.left, y: r.top },
            Self::North => Point { x: mid_x, y: r.top },
            Self::NorthEast => Point { x: r.right, y: r.top },
            Self::East => Point { x: r.right, y: mid_y },
            Self::SouthEast => Point { x: r.right, y: r.bottom },
            Self::South => Point { x: mid_x, y: r.bottom },
            Self::SouthWest => Point { x: r.left, y: r.bottom },
            Self::West => Point { x: r.left, y: mid_y },
        }
    }
    pub fn hit_test(r: Rect, p: Point) -> Option<Self> {
        // Corners take precedence over edges when handles overlap in small selections.
        Self::ALL.into_iter().find(|h| {
            let c = h.center(r);
            (i64::from(c.x) - i64::from(p.x)).abs() <= i64::from(HANDLE_RADIUS)
                && (i64::from(c.y) - i64::from(p.y)).abs() <= i64::from(HANDLE_RADIUS)
        })
    }

    pub fn resize(self, original: Rect, p: Point, desktop: Rect) -> Rect {
        let min_w = MIN_SIZE.min(desktop.width()).max(1);
        let min_h = MIN_SIZE.min(desktop.height()).max(1);
        let mut out = original.clamp_inside(desktop);
        let x = p.x.clamp(desktop.left, desktop.right);
        let y = p.y.clamp(desktop.top, desktop.bottom);
        if matches!(self, Self::NorthWest | Self::West | Self::SouthWest) {
            out.left = x.min(out.right - min_w).max(desktop.left);
        }
        if matches!(self, Self::NorthEast | Self::East | Self::SouthEast) {
            out.right = x.max(out.left + min_w).min(desktop.right);
        }
        if matches!(self, Self::NorthWest | Self::North | Self::NorthEast) {
            out.top = y.min(out.bottom - min_h).max(desktop.top);
        }
        if matches!(self, Self::SouthWest | Self::South | Self::SouthEast) {
            out.bottom = y.max(out.top + min_h).min(desktop.bottom);
        }
        out
    }
}

/// Position a dragged screenshot crop using the original grab point.
/// Coordinate math saturates and the final result is bounded to the desktop.
pub fn move_crop(initial: Rect, anchor: Point, cursor: Point, desktop: Rect) -> Rect {
    initial.translated(
        cursor.x.saturating_sub(anchor.x),
        cursor.y.saturating_sub(anchor.y),
    ).clamp_inside(desktop)
}

/// Apply only the actual movement after desktop clamping to annotations.
pub fn crop_delta(previous: Rect, next: Rect) -> (i32, i32) {
    (next.left.saturating_sub(previous.left),
     next.top.saturating_sub(previous.top))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn desktop() -> Rect {
        Rect { left: -1920, top: -400, right: 2560, bottom: 1440 }
    }
    fn sample() -> Rect {
        Rect { left: -200, top: 50, right: 600, bottom: 450 }
    }

    #[test]
    fn finds_each_handle_on_negative_monitor() {
        for h in Handle::ALL {
            assert_eq!(Handle::hit_test(sample(), h.center(sample())), Some(h));
        }
        assert_eq!(Handle::hit_test(sample(), Point { x: 100, y: 200 }), None);
    }

    #[test]
    fn resize_corners_and_edges() {
        let r = sample();
        let new = Handle::NorthWest.resize(r, Point { x: -900, y: -300 }, desktop());
        assert_eq!(new, Rect { left: -900, top: -300, ..r });
        let new = Handle::East.resize(r, Point { x: 1000, y: 999 }, desktop());
        assert_eq!(new, Rect { right: 1000, ..r });
    }

    #[test]
    fn cannot_invert_or_escape_desktop() {
        let r = sample();
        let new = Handle::West.resize(r, Point { x: 9999, y: 0 }, desktop());
        assert_eq!(new.width(), MIN_SIZE);
        let new = Handle::SouthEast.resize(r, Point { x: i32::MAX, y: i32::MAX }, desktop());
        assert_eq!((new.right, new.bottom), (desktop().right, desktop().bottom));
        let new = Handle::NorthWest.resize(r, Point { x: i32::MIN, y: i32::MIN }, desktop());
        assert_eq!((new.left, new.top), (desktop().left, desktop().top));
    }

    #[test]
    fn tiny_rectangles_take_corner_priority() {
        let r = Rect { left: 0, top: 0, right: 4, bottom: 4 };
        assert_eq!(Handle::hit_test(r, Point { x: 0, y: 0 }), Some(Handle::NorthWest));
    }
    #[test]
    fn clamped_motion_delta_does_not_drift_annotations() {
        let bounds=Rect {left:-100,top:-40,right:500,bottom:300};
        let initial=Rect {left:250,top:100,right:450,bottom:260};
        let anchor=Point{x:260,y:110};
        let at_edge=move_crop(initial,anchor,Point{x:2000,y:1200},bounds);
        assert_eq!(at_edge,Rect {left:300,top:140,right:500,bottom:300});
        assert_eq!(crop_delta(initial,at_edge),(50,40));
        let beyond=move_crop(initial,anchor,Point{x:i32::MAX,y:i32::MAX},bounds);
        assert_eq!(crop_delta(at_edge,beyond),(0,0));
        let back=move_crop(initial,anchor,Point{x:100,y:80},bounds);
        assert_eq!(crop_delta(at_edge,back),(-200,-70));
    }

}
