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
    Text,
    Mosaic,
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stroke {
    /// Windows DIB BGRA bytes. Alpha always opaque.
    pub bgra: [u8; 4],
    pub width: u8,
}
impl Stroke {
    pub const COLORS: [[u8; 4]; 5] = [
        [39, 48, 232, 255],    // red
        [224, 116, 52, 255],   // blue
        [71, 166, 37, 255],    // green
        [37, 190, 245, 255],   // yellow
        [20, 20, 20, 255],     // black
    ];
    pub fn preset(index: usize, width: u8) -> Self {
        Self { bgra: Self::COLORS[index.min(4)], width: width.clamp(1, 9) }
    }
}
impl Default for Stroke {
    fn default() -> Self { Self::preset(0, 4) }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Annotation {
    pub tool: Tool,
    pub points: Vec<Point>,
    pub text: String,
    pub stroke: Stroke,
}

impl Annotation {
    pub fn begin(tool: Tool, point: Point) -> Self {
        Self { tool, points: vec![point, point], text: String::new(), stroke: Stroke::default() }
    }
    pub fn text_at(point: Point, text: String) -> Self {
        Self { tool: Tool::Text, points: vec![point], text, stroke: Stroke::default() }
    }
    pub fn with_stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = stroke;
        self
    }
    pub fn update(&mut self, point: Point) {
        if self.tool == Tool::Pen {
            if self.points.last() != Some(&point) { self.points.push(point); }
        } else if self.points.len() >= 2 {
            self.points[1] = point;
        }
    }
    pub fn meaningful(&self) -> bool {
        if self.tool == Tool::Text {
            !self.text.trim().is_empty() && !self.points.is_empty()
        } else if self.tool == Tool::Pen {
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

#[cfg(test)]
const RED_BGRA: [u8; 4] = Stroke::COLORS[0];

struct Canvas<'a> {
    pixels: &'a mut [u8],
    width: usize,
    height: usize,
    origin: Point,
    stroke: Stroke,
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
            pixel.copy_from_slice(&self.stroke.bgra);
        }
    }

    fn thick_dot(&mut self, x: i32, y: i32) {
        let radius = i32::from(self.stroke.width.saturating_sub(1)) / 2;
        let trailing = i32::from(self.stroke.width) - radius - 1;
        for oy in -radius..=trailing {
            for ox in -radius..=trailing {
                self.dot(x.saturating_add(ox), y.saturating_add(oy));
            }
        }
    }

    // Pixelate a clipped rectangle in fixed-size blocks. Each tile is confined
    // to the annotation bounds; never leak obscured content through the edge.
    fn mosaic(&mut self, a: Point, b: Point) {
        let left = (i64::from(a.x.min(b.x)) - i64::from(self.origin.x))
            .clamp(0, self.width as i64) as usize;
        let right = (i64::from(a.x.max(b.x)) - i64::from(self.origin.x))
            .clamp(0, self.width as i64) as usize;
        let top = (i64::from(a.y.min(b.y)) - i64::from(self.origin.y))
            .clamp(0, self.height as i64) as usize;
        let bottom = (i64::from(a.y.max(b.y)) - i64::from(self.origin.y))
            .clamp(0, self.height as i64) as usize;
        const BLOCK: usize = 12;
        for tile_y in (top..bottom).step_by(BLOCK) {
            for tile_x in (left..right).step_by(BLOCK) {
                let end_y = (tile_y + BLOCK).min(bottom);
                let end_x = (tile_x + BLOCK).min(right);
                let mut sum = [0u64; 3];
                let mut count = 0u64;
                for y in tile_y..end_y {
                    for x in tile_x..end_x {
                        let n = (y * self.width + x) * 4;
                        for (c, value) in sum.iter_mut().enumerate() { *value += u64::from(self.pixels[n + c]); }
                        count += 1;
                    }
                }
                if count == 0 { continue; }
                let avg = [(sum[0] / count) as u8, (sum[1] / count) as u8,
                           (sum[2] / count) as u8];
                for y in tile_y..end_y {
                    for x in tile_x..end_x {
                        let n = (y * self.width + x) * 4;
                        self.pixels[n..n + 3].copy_from_slice(&avg);
                    }
                }
            }
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
    let mut canvas = Canvas { pixels, width: w, height: h, origin, stroke: Stroke::default() };
    for annotation in annotations {
        canvas.stroke = annotation.stroke;
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
            Tool::Mosaic => canvas.mosaic(a, b),
            Tool::Select | Tool::Text => {}
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
        ], text: String::new(), stroke: Stroke::default() });
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
    #[test]
    fn mosaic_changes_only_region_and_keeps_other_pixels() {
        let mut pixels = vec![0u8; 6 * 6 * 4];
        for y in 0..6usize {
            for x in 0..6usize {
                let p = (y * 6 + x) * 4;
                pixels[p..p+4].copy_from_slice(&[(x*31) as u8, (y*21) as u8, 100, 255]);
            }
        }
        let original = pixels.clone();
        let mut a = Annotation::begin(Tool::Mosaic, Point {x: -12, y: -6});
        a.update(Point {x: -10, y: -4});
        render(&mut pixels, 6, 6, Point {x: -12, y: -6}, &[a]);
        assert_ne!(&pixels[0..3], &original[0..3]);
        assert_eq!(&pixels[(5*6+5)*4..], &original[(5*6+5)*4..]);
    }
    #[test]
    fn text_history_supports_chinese_ime_content() {
        let mut history = History::default();
        let annotation = Annotation::text_at(Point{x:-40,y:30}, "截图 文字 ✓".into());
        assert!(annotation.meaningful());
        history.push(annotation.clone());
        assert_eq!(history.committed[0].text, "截图 文字 ✓");
        assert!(history.undo());
        assert!(history.redo());
        assert_eq!(history.committed[0], annotation);
        history.push(Annotation::text_at(Point {x:0,y:0}, " ".into()));
        assert_eq!(history.committed.len(), 1);
    }

    #[test]
    fn each_annotation_keeps_its_own_color_and_thickness_after_undo() {
        let red = Stroke::preset(0, 3);
        let blue = Stroke::preset(1, 7);
        let mut history = History::default();
        let a = Annotation::begin(Tool::Rectangle, Point {x: 1, y: 1}).with_stroke(red);
        let mut a = a; a.update(Point {x: 12, y: 12});
        history.push(a);
        let mut b = Annotation::begin(Tool::Pen, Point {x: 16, y: 16}).with_stroke(blue);
        b.update(Point {x: 17, y: 17}); b.update(Point {x: 18, y: 18});
        history.push(b);
        assert!(history.undo());
        assert!(history.redo());
        assert_eq!(history.committed[0].stroke, red);
        assert_eq!(history.committed[1].stroke, blue);
        let mut pixels = vec![255u8; 24*24*4];
        render(&mut pixels, 24, 24, Point{x:0,y:0}, &history.committed);
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p == &blue.bgra));
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p == &red.bgra));
    }

    #[test]
    fn thickness_is_clamped() {
        assert_eq!(Stroke::preset(4, 0).width, 1);
        assert_eq!(Stroke::preset(3, u8::MAX).width, 9);
    }

}
