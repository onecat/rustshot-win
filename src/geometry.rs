//! Signed physical-pixel geometry, independent of any Win32 handle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn from_points(a: Point, b: Point) -> Self {
        Self {
            left: a.x.min(b.x),
            top: a.y.min(b.y),
            right: a.x.max(b.x),
            bottom: a.y.max(b.y),
        }
    }

    pub fn width(self) -> i32 {
        self.right.saturating_sub(self.left)
    }

    pub fn height(self) -> i32 {
        self.bottom.saturating_sub(self.top)
    }

    pub fn is_empty(self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    pub fn contains(self, p: Point) -> bool {
        p.x >= self.left && p.x < self.right && p.y >= self.top && p.y < self.bottom
    }

    pub fn intersection(self, other: Rect) -> Option<Self> {
        let rect = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (!rect.is_empty() && rect.right > rect.left && rect.bottom > rect.top).then_some(rect)
    }

    pub fn translated(self, dx: i32, dy: i32) -> Self {
        Self {
            left: self.left.saturating_add(dx),
            right: self.right.saturating_add(dx),
            top: self.top.saturating_add(dy),
            bottom: self.bottom.saturating_add(dy),
        }
    }

    pub fn clamp_inside(self, bounds: Rect) -> Self {
        let width = self.width().min(bounds.width());
        let height = self.height().min(bounds.height());
        let left = self.left.clamp(bounds.left, bounds.right.saturating_sub(width));
        let top = self.top.clamp(bounds.top, bounds.bottom.saturating_sub(height));
        Self { left, top, right: left + width, bottom: top + height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_monitor_coords_and_reverse_drag() {
        let r = Rect::from_points(Point { x: 100, y: 250 }, Point { x: -750, y: -100 });
        assert_eq!(r, Rect { left: -750, top: -100, right: 100, bottom: 250 });
        assert_eq!(r.width(), 850);
        assert!(r.contains(Point { x: -10, y: 0 }));
    }

    #[test]
    fn intersections_and_edges() {
        let a = Rect { left: -100, top: -100, right: 100, bottom: 100 };
        let b = Rect { left: 90, top: 50, right: 300, bottom: 200 };
        assert_eq!(a.intersection(b).unwrap(), Rect { left: 90, top: 50, right: 100, bottom: 100 });
        assert_eq!(a.intersection(Rect { left: 100, top: 0, right: 200, bottom: 200 }), None);
    }

    #[test]
    fn moving_stays_inside_virtual_desktop() {
        let bounds = Rect { left: -1920, top: -50, right: 1920, bottom: 1080 };
        let result = Rect { left: 1800, top: 900, right: 2100, bottom: 1300 }.clamp_inside(bounds);
        assert_eq!((result.right, result.bottom), (1920, 1080));
        assert_eq!((result.width(), result.height()), (300, 400));
    }
}
