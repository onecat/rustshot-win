//! Dependency-free non-destructive screenshot annotations.
//! Shapes are stored in desktop coordinates and rasterized only for export.
use crate::geometry::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rectangle,
    Ellipse,
    Arrow,
    Pen,
}

impl Tool {
    pub const LABELS: [&'static str; 5] = ["选区", "矩形", "椭圆", "箭头", "画笔"];
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub tool: Tool,
    pub points: Vec<Point>,
}

impl Annotation {
    pub fn begin(tool: Tool, point: Point) -> Self {
        Self { tool, points: vec![point, point] }
    }
    pub fn update(&mut self, point: Point) {
        if self.tool == Tool::Pen {
            if self.points.last() != Some(&point) { self.points.push(point); }
        } else if self.points.len() >= 2 {
            self.points[1] = point;
        }
    }
    pub fn meaningful(&self) -> bool {
        if self.tool == Tool::Pen {
            self.points.len() > 2
        } else {
            self.points.len() >= 2
                && (self.points[0].x - self.points[1].x).abs() +
                   (self.points[0].y - self.points[1].y).abs() >= 3
        }
    }
}

#[derive(Default)]
pub struct History {
    pub committed: Vec<Annotation>,
    pub undone: Vec<Annotation>,
}

impl History {
    pub fn push(&mut self, item: Annotation) {
        if item.meaningful() {
            self.committed.push(item);
            self.undone.clear();
        }
    }
    pub fn undo(&mut self) -> bool {
        if let Some(item) = self.committed.pop() {
            self.undone.push(item);
            true
        } else { false }
    }
    pub fn redo(&mut self) -> bool {
        if let Some(item) = self.undone.pop() {
            self.committed.push(item);
            true
        } else { false }
    }
    pub fn clear(&mut self) {
        self.committed.clear();
        self.undone.clear();
    }
}

const RED_BGRA: [u8; 4] = [39, 48, 232, 255];

struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: usize,
    height: usize,
    origin: Point,
}

impl Canvas<'_> {
    fn dot(&mut self, x: i32, y: i32) {
        let px = i64::from(x) - i64::from(self.origin.x);
        let py = i64::from(y) - i64::from(self.origin.y);
        if px < 0 || py < 0 || px >= self.width as i64 || py >= self.height as i64 {
            return;
        }
        let index = (py as usize * self.width + px as usize) * 4;
        if let Some(pixel) = self.pixels.get_mut(index..index + 4) {
            pixel.copy_from_slice(&RED_BGRA);
        }
    }

    fn thick_dot(&mut self, x: i32, y: i32) {
        for oy in -1..=1 {
            for ox in -1..=1 { self.dot(x.saturating_add(ox), y.saturating_add(oy)); }
        }
    }

    fn line(&mut self, a: Point, b: Point) {
        let (mut x, mut y) = (i64::from(a.x), i64::from(a.y));
        let (x2, y2) = (i64::from(b.x), i64::from(b.y));
        let dx = (x2 - x).abs();
        let dy = -(y2 - y).abs();
        let sx = if x < x2 { 1 } else { -1 };
        let sy = if y < y2 { 1 } else { -1 };
        let mut err = dx + dy;
        // Mouse input can stray offscreen. Bound CPU work to a few image diagonals.
        let max_steps = ((self.width + self.height).saturating_mul(4)).max(1);
        for _ in 0..max_steps {
            self.thick_dot(x as i32, y as i32);
            if x == x2 && y == y2 { break; }
            let e2 = 2 * err;
            if e2 >= dy { err += dy; x += sx; }
            if e2 <= dx { err += dx; y += sy; }
        }
    }
}

pub fn render(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    origin: Point,
    annotations: &[Annotation],
) {
    let (w, h) = (width as usize, height as usize);
    if w == 0 || h == 0 || w.checked_mul(h).and_then(|v| v.checked_mul(4)) != Some(pixels.len()) {
        return;
    }
    let mut canvas = Canvas { pixels, width: w, height: h, origin };
    for annotation in annotations {
        let Some(&a) = annotation.points.first() else { continue; };
        let b = *annotation.points.last().unwrap_or(&a);
        match annotation.tool {
            Tool::Rectangle => {
                canvas.line(a, Point { x: b.x, y: a.y });
                canvas.line(Point { x: b.x, y: a.y }, b);
                canvas.line(b, Point { x: a.x, y: b.y });
                canvas.line(Point { x: a.x, y: b.y }, a);
            }
            Tool::Ellipse => {
                let mid_x = (f64::from(a.x) + f64::from(b.x)) / 2.0;
                let mid_y = (f64::from(a.y) + f64::from(b.y)) / 2.0;
                let radius_x = (f64::from(a.x) - f64::from(b.x)).abs() / 2.0;
                let radius_y = (f64::from(a.y) - f64::from(b.y)).abs() / 2.0;
                let steps = (radius_x.max(radius_y) * 2.0).clamp(32.0, 2048.0) as usize;
                let mut previous = None;
                for i in 0..=steps {
                    let t = std::f64::consts::TAU * i as f64 / steps as f64;
                    let point = Point { x: (mid_x + radius_x * t.cos()) as i32,
                        y: (mid_y + radius_y * t.sin()) as i32 };
                    if let Some(last) = previous { canvas.line(last, point); }
                    previous = Some(point);
                }
            }
            Tool::Arrow => {
                canvas.line(a, b);
                let angle = f64::from(b.y - a.y).atan2(f64::from(b.x - a.x));
                for sign in [-1.0_f64, 1.0] {
                    let theta = angle + std::f64::consts::PI + sign * 0.52;
                    let tip = Point { x: (f64::from(b.x) + 16.0 * theta.cos()) as i32,
                        y: (f64::from(b.y) + 16.0 * theta.sin()) as i32 };
                    canvas.line(b, tip);
                }
            }
            Tool::Pen => {
                for pair in annotation.points.windows(2) { canvas.line(pair[0], pair[1]); }
            }
            Tool::Select => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_destructive_history_and_branching() {
        let mut h = History::default();
        let mut a = Annotation::begin(Tool::Rectangle, Point { x: -5, y: 3 });
        a.update(Point { x: 10, y: 20 });
        h.push(a.clone());
        assert!(h.undo());
        assert_eq!(h.undone.len(), 1);
        assert!(h.redo());
        assert_eq!(h.committed[0], a);
        assert!(h.undo());
        h.push(Annotation { tool: Tool::Pen, points: vec![
            Point { x: 0, y: 0 }, Point { x: 1, y: 1 }, Point { x: 2, y: 2 },
        ] });
        assert!(!h.redo());
    }

    #[test]
    fn rasterizes_and_clips_negative_desktop_coordinates() {
        let mut pixels = vec![255u8; 4 * 16 * 16];
        let mut a = Annotation::begin(Tool::Arrow, Point { x: -99, y: -98 });
        a.update(Point { x: -91, y: -90 });
        render(&mut pixels, 16, 16, Point { x: -100, y: -100 }, &[a]);
        assert!(pixels.chunks(4).any(|p| p == RED_BGRA));
    }
}
