//! Minimal native Win32/GDI baseline for the first screenshot PoC.
//! No external crates; unsafe FFI calls are isolated here. Windows 11 validation is pending.
use crate::annotations::{self, Annotation, History, Tool};
use crate::geometry::{Point, Rect};
use crate::selection::Handle as ResizeHandle;
use crate::png;
use std::cell::RefCell;
use std::ffi::c_void;
use std::fs::{self, OpenOptions};
use std::io;
use std::mem::{size_of, zeroed};
use std::path::PathBuf;
use std::ptr::{null, null_mut};
use std::thread;
use std::time::Duration;

type Handle = isize;
type Hwnd = isize;
type Hdc = isize;
type Hobj = isize;
type Lresult = isize;
type Wparam = usize;
type Lparam = isize;
const SRCCOPY: u32 = 0x00CC0020;
const CAPTUREBLT: u32 = 0x40000000;
const DIB_RGB_COLORS: u32 = 0;
const BI_RGB: u32 = 0;
const WS_POPUP: u32 = 0x80000000;
const WS_EX_TOPMOST: u32 = 0x00000008;
const WS_EX_TOOLWINDOW: u32 = 0x00000080;
const SW_SHOW: i32 = 5;
const SW_HIDE: i32 = 0;
const WM_DESTROY: u32 = 0x0002;
const WM_PAINT: u32 = 0x000F;
const WM_CLOSE: u32 = 0x0010;
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_KEYDOWN: u32 = 0x0100;
const WM_HOTKEY: u32 = 0x0312;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_CONTEXTMENU: u32 = 0x007B;
const WM_APP_TRAY: u32 = 0x8000 + 12;
const VK_ESCAPE: usize = 27;
const VK_RETURN: usize = 13;
const VK_CONTROL: i32 = 17;
const VK_Z: usize = 0x5A;
const VK_Y: usize = 0x59;
const MOD_ALT: u32 = 0x0001;
const MOD_CONTROL: u32 = 0x0002;
const MOD_SHIFT: u32 = 0x0004;
const MOD_NOREPEAT: u32 = 0x4000;
const NIM_ADD: u32 = 0;
const NIM_DELETE: u32 = 2;
const NIF_MESSAGE: u32 = 0x0001;
const NIF_ICON: u32 = 0x0002;
const NIF_TIP: u32 = 0x0004;
const MF_STRING: u32 = 0;
const TPM_RIGHTBUTTON: u32 = 2;
const TPM_RETURNCMD: u32 = 0x0100;
const NULL_BRUSH: i32 = 5;
const PS_SOLID: i32 = 0;
const TRANSPARENT: i32 = 1;
const CF_DIB: u32 = 8;
const GMEM_MOVEABLE: u32 = 0x0002;
const ERROR_ALREADY_EXISTS: u32 = 183;
const MAX_CAPTURE_BYTES: usize = 512 * 1024 * 1024;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
struct WinRect { left: i32, top: i32, right: i32, bottom: i32 }
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct WinPoint { x: i32, y: i32 }
#[repr(C)]
#[derive(Clone, Copy)]
struct Msg { hwnd: Hwnd, message: u32, wparam: Wparam, lparam: Lparam, time: u32, pt: WinPoint, lprivate: u32 }
#[repr(C)]
#[derive(Clone, Copy)]
struct WndClass {
    style: u32,
    wndproc: Option<unsafe extern "system" fn(Hwnd, u32, Wparam, Lparam) -> Lresult>,
    cls_extra: i32,
    wnd_extra: i32,
    instance: Handle,
    icon: Handle,
    cursor: Handle,
    background: Handle,
    menu_name: *const u16,
    class_name: *const u16,
}
#[repr(C)]
struct PaintStruct {
    dc: Hdc,
    erase: i32,
    rc_paint: WinRect,
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
}
#[repr(C)]
#[derive(Clone, Copy)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    size_image: u32,
    x_pels_per_meter: i32,
    y_pels_per_meter: i32,
    clr_used: u32,
    clr_important: u32,
}
#[repr(C)]
struct BitmapInfo { header: BitmapInfoHeader, colors: [u32; 1] }
#[repr(C)]
#[derive(Clone, Copy)]
struct Guid { d1: u32, d2: u16, d3: u16, d4: [u8; 8] }
#[repr(C)]
struct NotifyIconData {
    cb_size: u32, hwnd: Hwnd, id: u32, flags: u32, callback: u32, icon: Handle,
    tip: [u16; 128], state: u32, state_mask: u32, info: [u16; 256],
    timeout: u32, info_title: [u16; 64], info_flags: u32,
    guid: Guid, balloon_icon: Handle,
}
#[repr(C)]
struct SystemTime { year: u16, month: u16, day_of_week: u16, day: u16, hour: u16, minute: u16, second: u16, milliseconds: u16 }

#[link(name = "user32")]
extern "system" {
    fn SetProcessDpiAwarenessContext(value: isize) -> i32;
    fn RegisterClassW(cls: *const WndClass) -> u16;
    fn CreateWindowExW(exstyle: u32, class: *const u16, title: *const u16, style: u32,
        x: i32, y: i32, w: i32, h: i32, parent: Hwnd, menu: Handle, inst: Handle, param: *const c_void) -> Hwnd;
    fn DefWindowProcW(hwnd: Hwnd, msg: u32, w: Wparam, l: Lparam) -> Lresult;
    fn DestroyWindow(hwnd: Hwnd) -> i32;
    fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    fn UpdateWindow(hwnd: Hwnd) -> i32;
    fn SetForegroundWindow(hwnd: Hwnd) -> i32;
    fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> Lresult;
    fn PostQuitMessage(code: i32);
    fn RegisterHotKey(hwnd: Hwnd, id: i32, modifiers: u32, key: u32) -> i32;
    fn UnregisterHotKey(hwnd: Hwnd, id: i32) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn GetDC(hwnd: Hwnd) -> Hdc;
    fn ReleaseDC(hwnd: Hwnd, dc: Hdc) -> i32;
    fn BeginPaint(hwnd: Hwnd, paint: *mut PaintStruct) -> Hdc;
    fn EndPaint(hwnd: Hwnd, paint: *const PaintStruct) -> i32;
    fn InvalidateRect(hwnd: Hwnd, rect: *const WinRect, erase: i32) -> i32;
    fn SetCapture(hwnd: Hwnd) -> Hwnd;
    fn ReleaseCapture() -> i32;
    fn SetFocus(hwnd: Hwnd) -> Hwnd;
    fn GetKeyState(key: i32) -> i16;
    fn EnumWindows(cb: Option<unsafe extern "system" fn(Hwnd, Lparam) -> i32>, lparam: Lparam) -> i32;
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn IsIconic(hwnd: Hwnd) -> i32;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
    fn GetCursorPos(point: *mut WinPoint) -> i32;
    fn LoadIconW(inst: Handle, name: *const u16) -> Handle;
    fn LoadCursorW(inst: Handle, name: *const u16) -> Handle;
    fn CreatePopupMenu() -> Handle;
    fn AppendMenuW(menu: Handle, flags: u32, id: usize, text: *const u16) -> i32;
    fn TrackPopupMenu(menu: Handle, flags: u32, x: i32, y: i32, reserved: i32, hwnd: Hwnd, rect: *const WinRect) -> i32;
    fn DestroyMenu(menu: Handle) -> i32;
    fn MessageBoxW(hwnd: Hwnd, text: *const u16, title: *const u16, flags: u32) -> i32;
    fn FillRect(dc: Hdc, rect: *const WinRect, brush: Hobj) -> i32;
    fn OpenClipboard(hwnd: Hwnd) -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, handle: Handle) -> Handle;
    fn CloseClipboard() -> i32;
}
#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(dc: Hdc) -> Hdc;
    fn DeleteDC(dc: Hdc) -> i32;
    fn CreateDIBSection(dc: Hdc, info: *const BitmapInfo, usage: u32, bits: *mut *mut c_void, section: Handle, offset: u32) -> Hobj;
    fn SelectObject(dc: Hdc, obj: Hobj) -> Hobj;
    fn DeleteObject(obj: Hobj) -> i32;
    fn BitBlt(dest: Hdc, x: i32, y: i32, w: i32, h: i32, src: Hdc, sx: i32, sy: i32, rop: u32) -> i32;
    fn StretchDIBits(dc: Hdc, x: i32, y: i32, w: i32, h: i32, sx: i32, sy: i32, sw: i32, sh: i32,
        bits: *const c_void, info: *const BitmapInfo, usage: u32, rop: u32) -> i32;
    fn CreatePen(style: i32, width: i32, color: u32) -> Hobj;
    fn GetStockObject(index: i32) -> Hobj;
    fn Rectangle(dc: Hdc, l: i32, t: i32, r: i32, b: i32) -> i32;
    fn Ellipse(dc: Hdc, l: i32, t: i32, r: i32, b: i32) -> i32;
    fn MoveToEx(dc: Hdc, x: i32, y: i32, previous: *mut WinPoint) -> i32;
    fn LineTo(dc: Hdc, x: i32, y: i32) -> i32;
    fn SaveDC(dc: Hdc) -> i32;
    fn RestoreDC(dc: Hdc, saved: i32) -> i32;
    fn IntersectClipRect(dc: Hdc, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn CreateSolidBrush(color: u32) -> Hobj;
    fn SetTextColor(dc: Hdc, color: u32) -> u32;
    fn SetBkMode(dc: Hdc, mode: i32) -> i32;
    fn TextOutW(dc: Hdc, x: i32, y: i32, text: *const u16, len: i32) -> i32;
}
#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(command: u32, data: *mut NotifyIconData) -> i32;
    fn SHGetKnownFolderPath(id: *const Guid, flags: u32, token: Handle, path: *mut *mut u16) -> i32;
    fn ShellExecuteW(hwnd: Hwnd, op: *const u16, file: *const u16, params: *const u16, directory: *const u16, show: i32) -> Handle;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetLastError() -> u32;
    fn CreateMutexW(attributes: *const c_void, initial: i32, name: *const u16) -> Handle;
    fn CloseHandle(handle: Handle) -> i32;
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalLock(handle: Handle) -> *mut c_void;
    fn GlobalUnlock(handle: Handle) -> i32;
    fn GlobalFree(handle: Handle) -> Handle;
    fn GetLocalTime(time: *mut SystemTime);
    fn MoveFileW(existing: *const u16, new_path: *const u16) -> i32;
}
#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(hwnd: Hwnd, attr: u32, dest: *mut c_void, size: u32) -> i32;
}
#[link(name = "ole32")]
extern "system" { fn CoTaskMemFree(ptr: *mut c_void); }

fn utf16(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
fn win_rect(r: Rect) -> WinRect { WinRect { left: r.left, top: r.top, right: r.right, bottom: r.bottom } }
fn low_word_signed(l: Lparam) -> i32 { l as u16 as i16 as i32 }
fn high_word_signed(l: Lparam) -> i32 { (l >> 16) as u16 as i16 as i32 }
fn black_tint(data: &mut [u8]) {
    for p in data.as_chunks_mut::<4>().0 {
        for color in p.iter_mut().take(3) { *color = ((*color as u16 * 145) / 255) as u8; }
    }
}
fn bitmap_info(w: i32, h: i32) -> BitmapInfo {
    BitmapInfo { header: BitmapInfoHeader {
        size: size_of::<BitmapInfoHeader>() as u32, width: w, height: -h,
        planes: 1, bit_count: 32, compression: BI_RGB, size_image: 0,
        x_pels_per_meter: 0, y_pels_per_meter: 0, clr_used: 0, clr_important: 0,
    }, colors: [0] }
}

struct Frame { bounds: Rect, pixels: Vec<u8>, dark: Vec<u8>, windows: Vec<Rect> }
struct AppState {
    main: Hwnd,
    overlay: Hwnd,
    frame: Option<Frame>,
    hover: Option<Rect>,
    selected: Option<Rect>,
    dragging: bool,
    moving: bool,
    resizing: Option<ResizeHandle>,
    active_annotation: Option<Annotation>,
    tool: Tool,
    history: History,
    drag_anchor: Point,
    drag_initial: Rect,
    fallback_hotkey: bool,
    writers: Vec<thread::JoinHandle<()>>,
}
impl AppState {
    fn new() -> Self {
        Self { main: 0, overlay: 0, frame: None, hover: None, selected: None,
            dragging: false, moving: false, resizing: None, active_annotation: None,
            tool: Tool::Select, history: History::default(), drag_anchor: Point::default(), drag_initial: Rect::default(), fallback_hotkey: false, writers: Vec::new() }
    }
}
thread_local! { static APP: RefCell<AppState> = RefCell::new(AppState::new()); }

fn virtual_desktop() -> Rect {
    // SAFETY: GetSystemMetrics does not touch pointers and returns pixel dimensions on this DPI-aware thread.
    unsafe {
        let x = GetSystemMetrics(76);
        let y = GetSystemMetrics(77);
        Rect { left: x, top: y, right: x + GetSystemMetrics(78), bottom: y + GetSystemMetrics(79) }
    }
}

fn capture_desktop() -> Result<Frame, String> {
    let bounds = virtual_desktop();
    let width = bounds.width();
    let height = bounds.height();
    let count = (width as usize).checked_mul(height as usize).and_then(|n| n.checked_mul(4))
        .ok_or("Desktop image size overflow")?;
    if width <= 0 || height <= 0 || count > MAX_CAPTURE_BYTES { return Err("Unsupported virtual desktop size".into()); }
    // SAFETY: CreateDIBSection gives us a mapped pixel buffer while its HBITMAP exists.
    // Every created GDI object and acquired HDC is released on both success and error.
    unsafe {
        let src = GetDC(0);
        if src == 0 { return Err("GetDC failed".into()); }
        let dest = CreateCompatibleDC(src);
        if dest == 0 { ReleaseDC(0, src); return Err("CreateCompatibleDC failed".into()); }
        let mut bits: *mut c_void = null_mut();
        let info = bitmap_info(width, height);
        let bmp = CreateDIBSection(src, &info, DIB_RGB_COLORS, &mut bits, 0, 0);
        if bmp == 0 || bits.is_null() {
            if bmp != 0 { DeleteObject(bmp); }
            DeleteDC(dest); ReleaseDC(0, src);
            return Err("CreateDIBSection failed".into());
        }
        let old = SelectObject(dest, bmp);
        let copied = BitBlt(dest, 0, 0, width, height, src, bounds.left, bounds.top, SRCCOPY | CAPTUREBLT) != 0;
        let pixels = if copied { std::slice::from_raw_parts(bits as *const u8, count).to_vec() } else { Vec::new() };
        if old != 0 { SelectObject(dest, old); }
        DeleteObject(bmp);
        DeleteDC(dest);
        ReleaseDC(0, src);
        if !copied { return Err("BitBlt failed to capture desktop".into()); }
        let mut dark = pixels.clone();
        black_tint(&mut dark);
        Ok(Frame { bounds, pixels, dark, windows: enumerate_windows(bounds) })
    }
}

unsafe extern "system" fn enumerate_windows_callback(hwnd: Hwnd, value: Lparam) -> i32 {
    // SAFETY: EnumWindows invokes this callback synchronously; value points to a live Vec<Rect>.
    let list = &mut *(value as *mut Vec<Rect>);
    if IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 { return 1; }
    let mut cloaked: u32 = 0;
    if DwmGetWindowAttribute(hwnd, 14, &mut cloaked as *mut _ as *mut c_void, 4) == 0 && cloaked != 0 {
        return 1;
    }
    let mut rect: WinRect = zeroed();
    if DwmGetWindowAttribute(hwnd, 9, &mut rect as *mut _ as *mut c_void, size_of::<WinRect>() as u32) != 0
        && GetWindowRect(hwnd, &mut rect) == 0 { return 1; }
    let screen = Rect { left: rect.left, top: rect.top, right: rect.right, bottom: rect.bottom };
    if screen.width() >= 30 && screen.height() >= 30 { list.push(screen); }
    1
}
fn enumerate_windows(bounds: Rect) -> Vec<Rect> {
    let mut list = Vec::<Rect>::new();
    // SAFETY: Callback only holds pointer during this synchronous EnumWindows call.
    unsafe { EnumWindows(Some(enumerate_windows_callback), &mut list as *mut _ as Lparam); }
    list.into_iter().filter_map(|r| r.intersection(bounds)).collect()
}

fn hit_test(frame: &Frame, p: Point) -> Option<Rect> {
    frame.windows.iter().copied().find(|rect| rect.contains(p))
}
fn local_pos(frame: &Frame, l: Lparam) -> Point {
    Point { x: frame.bounds.left + low_word_signed(l), y: frame.bounds.top + high_word_signed(l) }
}
fn toolbar_for(selection: Rect, bounds: Rect) -> Rect {
    let width = 540.min(bounds.width().max(1));
    let height = 34.min(bounds.height().max(1));
    let x = (selection.right - width).clamp(bounds.left, bounds.right - width);
    let proposed = selection.bottom + 7;
    let y = if proposed + height <= bounds.bottom { proposed } else { (selection.top - 7 - height).max(bounds.top) };
    Rect { left: x, top: y, right: x + width, bottom: y + height }
}

// Preview uses GDI for low allocation while editing. Export uses the same vector
// records rendered into an independent BGRA crop in annotations::render.
unsafe fn preview_annotation(dc: Hdc, a: &Annotation, origin: Point) {
    if a.points.len() < 2 { return; }
    let first = a.points[0];
    let last = *a.points.last().unwrap_or(&first);
    let to_local = |p: Point| Point { x: p.x - origin.x, y: p.y - origin.y };
    let line = |from: Point, to: Point| {
        let start = to_local(from);
        let end = to_local(to);
        MoveToEx(dc, start.x, start.y, null_mut());
        LineTo(dc, end.x, end.y);
    };
    match a.tool {
        Tool::Rectangle => {
            let x = first.x.min(last.x) - origin.x;
            let y = first.y.min(last.y) - origin.y;
            let right = first.x.max(last.x) - origin.x;
            let bottom = first.y.max(last.y) - origin.y;
            Rectangle(dc, x, y, right, bottom);
        }
        Tool::Ellipse => {
            Ellipse(dc, first.x.min(last.x) - origin.x, first.y.min(last.y) - origin.y,
                first.x.max(last.x) - origin.x, first.y.max(last.y) - origin.y);
        }
        Tool::Arrow => {
            line(first, last);
            let angle = f64::from(last.y - first.y).atan2(f64::from(last.x - first.x));
            for sign in [-1.0_f64, 1.0] {
                let theta = angle + std::f64::consts::PI + sign * 0.52;
                let tip = Point { x: (f64::from(last.x) + 16.0 * theta.cos()) as i32,
                    y: (f64::from(last.y) + 16.0 * theta.sin()) as i32 };
                line(last, tip);
            }
        }
        Tool::Pen => {
            for segment in a.points.windows(2) { line(segment[0], segment[1]); }
        }
        Tool::Select => {}
    }
}

fn paint_annotations(dc: Hdc, origin: Point, selection: Rect, history: &History,
    current: Option<&Annotation>) {
    unsafe {
        let saved = SaveDC(dc);
        if saved == 0 { return; }
        let area = selection.translated(-origin.x, -origin.y);
        IntersectClipRect(dc, area.left, area.top, area.right, area.bottom);
        let pen = CreatePen(PS_SOLID, 3, 0x00E83027);
        if pen != 0 {
            let old_pen = SelectObject(dc, pen);
            let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
            for a in &history.committed { preview_annotation(dc, a, origin); }
            if let Some(a) = current { preview_annotation(dc, a, origin); }
            SelectObject(dc, old_brush);
            SelectObject(dc, old_pen);
            DeleteObject(pen);
        }
        RestoreDC(dc, saved);
    }
}

fn paint_overlay(hwnd: Hwnd) {
    // SAFETY: BeginPaint/EndPaint are paired; HDC is only used inside the paint call.
    unsafe {
        let mut ps: PaintStruct = zeroed();
        let dc = BeginPaint(hwnd, &mut ps);
        if dc == 0 { return; }
        APP.with(|cell| {
            let state = cell.borrow();
            let Some(frame) = &state.frame else { return; };
            let info = bitmap_info(frame.bounds.width(), frame.bounds.height());
            StretchDIBits(dc, 0, 0, frame.bounds.width(), frame.bounds.height(), 0, 0,
                frame.bounds.width(), frame.bounds.height(), frame.dark.as_ptr() as *const c_void,
                &info, DIB_RGB_COLORS, SRCCOPY);
            let active = state.selected.or(state.hover);
            if let Some(selection) = active.and_then(|s| s.intersection(frame.bounds)) {
                let rel = selection.translated(-frame.bounds.left, -frame.bounds.top);
                StretchDIBits(dc, rel.left, rel.top, rel.width(), rel.height(),
                    rel.left, rel.top, rel.width(), rel.height(),
                    frame.pixels.as_ptr() as *const c_void, &info, DIB_RGB_COLORS, SRCCOPY);
                let pen = CreatePen(PS_SOLID, 2, 0x0045C62A); // COLORREF BGR green
                let old_pen = SelectObject(dc, pen);
                let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
                Rectangle(dc, rel.left, rel.top, rel.right, rel.bottom);
                SelectObject(dc, old_brush);
                SelectObject(dc, old_pen);
                DeleteObject(pen);
                SetBkMode(dc, TRANSPARENT);
                SetTextColor(dc, 0x00FFFFFF);
                let title = format!("{} × {}", selection.width(), selection.height());
                let text = utf16(&title);
                TextOutW(dc, rel.left + 2, (rel.top - 20).max(0), text.as_ptr(), text.len() as i32 - 1);
                if state.selected.is_some() {
                    paint_annotations(dc, frame.bounds, selection, &state.history,
                        state.active_annotation.as_ref());
                    let white = CreateSolidBrush(0x00FFFFFF);
                    let green = CreatePen(PS_SOLID, 1, 0x0045C62A);
                    if white != 0 && green != 0 {
                        let old_pen = SelectObject(dc, green);
                        for handle in ResizeHandle::ALL {
                            let center = handle.center(selection);
                            let x = center.x - frame.bounds.left;
                            let y = center.y - frame.bounds.top;
                            let square = WinRect { left: x - 4, top: y - 4, right: x + 5, bottom: y + 5 };
                            FillRect(dc, &square, white);
                            Rectangle(dc, square.left, square.top, square.right, square.bottom);
                        }
                        SelectObject(dc, old_pen);
                    }
                    if white != 0 { DeleteObject(white); }
                    if green != 0 { DeleteObject(green); }

                    let bar = toolbar_for(selection, frame.bounds).translated(-frame.bounds.left, -frame.bounds.top);
                    let background = CreateSolidBrush(0x00FAFAFA);
                    if background != 0 {
                        FillRect(dc, &win_rect(bar), background);
                        DeleteObject(background);
                    }
                    let labels = [
                        Tool::LABELS[0], Tool::LABELS[1], Tool::LABELS[2],
                        Tool::LABELS[3], Tool::LABELS[4], "撤销", "重做", "完成", "取消",
                    ];
                    SetBkMode(dc, TRANSPARENT);
                    for (index, label) in labels.iter().enumerate() {
                        let left = bar.left + index as i32 * bar.width() / 9;
                        let right = bar.left + (index as i32 + 1) * bar.width() / 9;
                        if index < 5 && state.tool == [
                            Tool::Select, Tool::Rectangle, Tool::Ellipse, Tool::Arrow, Tool::Pen
                        ][index] {
                            let brush = CreateSolidBrush(0x00D5F4E4);
                            if brush != 0 {
                                FillRect(dc, &WinRect { left, top: bar.top, right,
                                    bottom: bar.bottom }, brush);
                                DeleteObject(brush);
                            }
                        }
                        SetTextColor(dc, if index == 7 { 0x002A9B21 } else { 0x00555555 });
                        let glyphs = utf16(label);
                        TextOutW(dc, left + 5, bar.top + 9, glyphs.as_ptr(), glyphs.len() as i32 - 1);
                    }
                }
            }
        });
        EndPaint(hwnd, &ps);
    }
}

fn alert(hwnd: Hwnd, message: &str) {
    let msg = utf16(message);
    let title = utf16("RustShot v0.1.0");
    // SAFETY: UTF-16 buffers are NUL terminated for the duration of MessageBoxW.
    unsafe { MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), 0x30); }
}

fn start_capture() {
    let existing = APP.with(|s| s.borrow().overlay != 0);
    if existing { return; }
    match capture_desktop() {
        Ok(frame) => {
            let bounds = frame.bounds;
            APP.with(|s| { s.borrow_mut().frame = Some(frame); });
            let name = utf16("RustShotOverlay");
            // SAFETY: The class is registered in run() and the module instance remains loaded.
            let hwnd = unsafe { CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                name.as_ptr(), name.as_ptr(), WS_POPUP,
                bounds.left, bounds.top, bounds.width(), bounds.height(), 0, 0, GetModuleHandleW(null()), null()) };
            if hwnd == 0 {
                APP.with(|s| s.borrow_mut().frame = None);
                alert(0, "Unable to create screenshot overlay.");
                return;
            }
            APP.with(|s| s.borrow_mut().overlay = hwnd);
            unsafe { ShowWindow(hwnd, SW_SHOW); UpdateWindow(hwnd); SetForegroundWindow(hwnd); SetFocus(hwnd); }
        }
        Err(err) => alert(0, &format!("Capture failed: {err}")),
    }
}

fn close_overlay() {
    let hwnd = APP.with(|cell| {
        let mut s = cell.borrow_mut();
        s.frame = None;
        s.selected = None;
        s.hover = None;
        s.dragging = false;
        s.moving = false;
        s.resizing = None;
        s.active_annotation = None;
        s.tool = Tool::Select;
        s.history.clear();
        std::mem::replace(&mut s.overlay, 0)
    });
    if hwnd != 0 { unsafe { ReleaseCapture(); DestroyWindow(hwnd); } }
}

fn selected_image() -> Option<(u32, u32, Vec<u8>)> {
    APP.with(|cell| {
        let state = cell.borrow();
        let frame = state.frame.as_ref()?;
        let rect = state.selected?.intersection(frame.bounds)?;
        if rect.width() < 1 || rect.height() < 1 { return None; }
        let mut crop = Vec::with_capacity(rect.width() as usize * rect.height() as usize * 4);
        let left = (rect.left - frame.bounds.left) as usize;
        let top = (rect.top - frame.bounds.top) as usize;
        for y in top..top + rect.height() as usize {
            let offset = (y * frame.bounds.width() as usize + left) * 4;
            crop.extend_from_slice(&frame.pixels[offset..offset + rect.width() as usize * 4]);
        }
        annotations::render(&mut crop, rect.width() as u32, rect.height() as u32,
            Point { x: rect.left, y: rect.top }, &state.history.committed);
        Some((rect.width() as u32, rect.height() as u32, crop))
    })
}

fn put_clipboard(owner: Hwnd, width: u32, height: u32, data: &[u8]) -> Result<(), String> {
    let raw_row = width as usize * 4;
    let len = size_of::<BitmapInfoHeader>().checked_add(data.len()).ok_or("Clipboard size overflow")?;
    // SAFETY: GMEM_MOVEABLE is required for SetClipboardData. On success the system owns the memory.
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, len);
        if memory == 0 { return Err("GlobalAlloc failed".into()); }
        let bytes = GlobalLock(memory) as *mut u8;
        if bytes.is_null() { GlobalFree(memory); return Err("GlobalLock failed".into()); }
        let mut header = bitmap_info(width as i32, height as i32).header;
        header.height = height as i32; // CF_DIB uses bottom-up rows
        std::ptr::copy_nonoverlapping(&header as *const _ as *const u8, bytes, size_of::<BitmapInfoHeader>());
        for y in 0..height as usize {
            std::ptr::copy_nonoverlapping(data.as_ptr().add(y * raw_row),
                bytes.add(size_of::<BitmapInfoHeader>() + (height as usize - 1 - y) * raw_row), raw_row);
        }
        GlobalUnlock(memory);
        let mut opened = false;
        for _ in 0..5 {
            if OpenClipboard(owner) != 0 { opened = true; break; }
            thread::sleep(Duration::from_millis(20));
        }
        if !opened { GlobalFree(memory); return Err("Clipboard busy".into()); }
        if EmptyClipboard() == 0 {
            CloseClipboard(); GlobalFree(memory); return Err("EmptyClipboard failed".into());
        }
        let inserted = SetClipboardData(CF_DIB, memory) != 0;
        CloseClipboard();
        if !inserted { GlobalFree(memory); return Err("SetClipboardData failed".into()); }
        Ok(())
    }
}

const FOLDERID_PICTURES: Guid = Guid {
    d1: 0x33E28130, d2: 0x4E1E, d3: 0x4676,
    d4: [0x83, 0x5A, 0x98, 0x39, 0x5C, 0x3B, 0xC3, 0xBB],
};
fn output_folder() -> io::Result<PathBuf> {
    // SAFETY: SHGetKnownFolderPath returns a CoTaskMemAlloc'ed string, freed once after copy.
    unsafe {
        let mut ptr: *mut u16 = null_mut();
        let status = SHGetKnownFolderPath(&FOLDERID_PICTURES, 0, 0, &mut ptr);
        if status < 0 || ptr.is_null() { return Err(io::Error::other("Pictures known folder unavailable")); }
        let mut len = 0;
        while *ptr.add(len) != 0 { len += 1; }
        let owned = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        CoTaskMemFree(ptr as *mut c_void);
        Ok(PathBuf::from(owned).join("RustShot"))
    }
}

fn save_png(width: u32, height: u32, pixels: &[u8]) -> io::Result<PathBuf> {
    let folder = output_folder()?;
    fs::create_dir_all(&folder)?;
    let mut now: SystemTime = unsafe { zeroed() };
    unsafe { GetLocalTime(&mut now); }
    let base = format!("Screenshot_{:04}-{:02}-{:02}_{:02}-{:02}-{:02}_{:03}",
        now.year, now.month, now.day, now.hour, now.minute, now.second, now.milliseconds);
    for n in 0..1000 {
        let stem = if n == 0 { base.clone() } else { format!("{base}_{n}") };
        let path = folder.join(format!("{stem}.png"));
        let temporary = folder.join(format!("{stem}.png.tmp"));
        // `create_new` reserves the temporary name, preventing writers from trampling one another.
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&temporary) {
            Ok(f) => f,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        };
        let result = png::encode_bgra(&mut file, width, height, pixels)
            .and_then(|()| file.sync_all());
        drop(file);
        if let Err(err) = result { let _ = fs::remove_file(&temporary); return Err(err); }
        // Win32 MoveFileW refuses to replace an existing destination, preventing a
        // cross-thread collision from overwriting an earlier screenshot.
        let from = utf16(&temporary.to_string_lossy());
        let to = utf16(&path.to_string_lossy());
        if unsafe { MoveFileW(from.as_ptr(), to.as_ptr()) } != 0 { return Ok(path); }
        let err = io::Error::last_os_error();
        let _ = fs::remove_file(&temporary);
        if err.kind() == io::ErrorKind::AlreadyExists { continue; }
        return Err(err);
    }
    Err(io::Error::new(io::ErrorKind::AlreadyExists, "Too many filename collisions"))
}

fn finish_capture() {
    let picture = selected_image();
    let owner = APP.with(|s| s.borrow().main);
    close_overlay();
    let Some((w, h, pixels)) = picture else { return; };
    let copied = put_clipboard(owner, w, h, &pixels);
    let worker = thread::spawn(move || {
        if let Err(err) = save_png(w, h, &pixels) {
            alert(0, &format!("Screenshot was not saved: {err}"));
        }
    });
    APP.with(|cell| {
        let mut app = cell.borrow_mut();
        let mut i = 0;
        while i < app.writers.len() {
            if app.writers[i].is_finished() { let finished = app.writers.remove(i); let _ = finished.join(); }
            else { i += 1; }
        }
        app.writers.push(worker);
    });
    if let Err(err) = copied { alert(owner, &format!("Clipboard copy failed; PNG save will still be attempted: {err}")); }
}

fn mouse_down(hwnd: Hwnd, l: Lparam) {
    let action = APP.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(frame) = &state.frame else { return 0usize; };
        let point = local_pos(frame, l);
        let bounds = frame.bounds;
        if let Some(selection) = state.selected {
            let bar = toolbar_for(selection, bounds);
            if bar.contains(point) {
                return ((i64::from(point.x - bar.left) * 9 / i64::from(bar.width())) as usize)
                    .min(8) + 1;
            }
        }

        state.drag_anchor = point;
        state.resizing = None;
        state.moving = false;
        if let Some(selection) = state.selected {
            if let Some(handle) = ResizeHandle::hit_test(selection, point) {
                state.resizing = Some(handle);
                state.drag_initial = selection;
                state.history.clear();
                state.tool = Tool::Select;
            } else if selection.contains(point) {
                if state.tool == Tool::Select {
                    state.moving = true;
                    state.drag_initial = selection;
                    state.history.clear();
                } else {
                    state.active_annotation = Some(Annotation::begin(state.tool, point));
                }
            } else {
                state.selected = None;
                state.history.clear();
                state.tool = Tool::Select;
            }
        }
        state.dragging = true;
        0
    });
    match action {
        1..=5 => {
            APP.with(|cell| {
                cell.borrow_mut().tool = match action {
                    1 => Tool::Select, 2 => Tool::Rectangle, 3 => Tool::Ellipse,
                    4 => Tool::Arrow, _ => Tool::Pen,
                };
            });
            unsafe { InvalidateRect(hwnd, null(), 0); }
        }
        6 => {
            APP.with(|cell| { cell.borrow_mut().history.undo(); });
            unsafe { InvalidateRect(hwnd, null(), 0); }
        }
        7 => {
            APP.with(|cell| { cell.borrow_mut().history.redo(); });
            unsafe { InvalidateRect(hwnd, null(), 0); }
        }
        8 => finish_capture(),
        9 => close_overlay(),
        _ => { unsafe { SetCapture(hwnd); } }
    }
}

fn mouse_move(hwnd: Hwnd, l: Lparam) {
    APP.with(|cell| {
        let mut s = cell.borrow_mut();
        let Some(frame) = &s.frame else { return; };
        let point = local_pos(frame, l);
        let bounds = frame.bounds;
        if s.dragging {
            if let Some(annotation) = &mut s.active_annotation {
                annotation.update(point);
            } else if let Some(handle) = s.resizing {
                s.selected = Some(handle.resize(s.drag_initial, point, bounds));
            } else if s.moving {
                s.selected = Some(s.drag_initial.translated(
                    point.x - s.drag_anchor.x, point.y - s.drag_anchor.y
                ).clamp_inside(bounds));
            } else {
                s.selected = Rect::from_points(s.drag_anchor, point).intersection(bounds);
            }
        } else if s.selected.is_none() {
            s.hover = hit_test(frame, point);
        }
    });
    unsafe { InvalidateRect(hwnd, null(), 0); }
}

fn mouse_up(hwnd: Hwnd, l: Lparam) {
    APP.with(|cell| {
        let mut s = cell.borrow_mut();
        if !s.dragging { return; }
        s.dragging = false;
        let adjusting = s.moving || s.resizing.is_some();
        s.moving = false;
        s.resizing = None;
        if let Some(annotation) = s.active_annotation.take() {
            s.history.push(annotation);
        } else if !adjusting {
            let Some(frame) = &s.frame else { return; };
            let point = local_pos(frame, l);
            if s.selected.map(|r| r.width() < 4 || r.height() < 4).unwrap_or(true) {
                s.selected = hit_test(frame, point);
            }
        }
    });
    unsafe { ReleaseCapture(); InvalidateRect(hwnd, null(), 0); }
}

unsafe extern "system" fn overlay_proc(hwnd: Hwnd, message: u32, w: Wparam, l: Lparam) -> Lresult {
    match message {
        WM_PAINT => { paint_overlay(hwnd); 0 }
        WM_MOUSEMOVE => { mouse_move(hwnd, l); 0 }
        WM_LBUTTONDOWN => { mouse_down(hwnd, l); 0 }
        WM_LBUTTONUP => { mouse_up(hwnd, l); 0 }
        WM_KEYDOWN if w == VK_RETURN => { finish_capture(); 0 }
        WM_KEYDOWN if (w == VK_Z || w == VK_Y) && GetKeyState(VK_CONTROL) < 0 => {
            APP.with(|cell| {
                if w == VK_Z { cell.borrow_mut().history.undo(); }
                else { cell.borrow_mut().history.redo(); }
            });
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_KEYDOWN if w == VK_ESCAPE => { close_overlay(); 0 }
        WM_RBUTTONUP | WM_CLOSE | WM_DISPLAYCHANGE => { close_overlay(); 0 }
        _ => DefWindowProcW(hwnd, message, w, l),
    }
}

fn tray_data(main: Hwnd, fallback: bool) -> NotifyIconData {
    let mut data: NotifyIconData = unsafe { zeroed() };
    data.cb_size = size_of::<NotifyIconData>() as u32;
    data.hwnd = main;
    data.id = 1;
    data.flags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.callback = WM_APP_TRAY;
    data.icon = unsafe { LoadIconW(0, 32512usize as *const u16) };
    let tooltip = if fallback { "RustShot - Ctrl+Shift+A" } else { "RustShot - Alt+A" };
    for (target, c) in data.tip.iter_mut().zip(tooltip.encode_utf16()) { *target = c; }
    data
}
fn show_tray_menu(hwnd: Hwnd) {
    // SAFETY: Created menu is destroyed once; TrackPopupMenu returns the selected ID.
    unsafe {
        let menu = CreatePopupMenu();
        if menu == 0 { return; }
        let take = utf16("截图 / Capture");
        let folder = utf16("打开截图目录 / Open folder");
        let quit = utf16("退出 / Exit");
        AppendMenuW(menu, MF_STRING, 1, take.as_ptr());
        AppendMenuW(menu, MF_STRING, 2, folder.as_ptr());
        AppendMenuW(menu, MF_STRING, 3, quit.as_ptr());
        let mut point = WinPoint::default();
        GetCursorPos(&mut point);
        SetForegroundWindow(hwnd);
        let selection = TrackPopupMenu(menu, TPM_RIGHTBUTTON | TPM_RETURNCMD, point.x, point.y, 0, hwnd, null());
        DestroyMenu(menu);
        match selection {
            1 => start_capture(),
            2 => match output_folder() {
                Ok(folder) => {
                    let _ = fs::create_dir_all(&folder);
                    let path = utf16(&folder.to_string_lossy());
                    ShellExecuteW(hwnd, null(), path.as_ptr(), null(), null(), SW_SHOW);
                }
                Err(err) => alert(hwnd, &format!("Cannot open screenshot folder: {err}")),
            },
            3 => { DestroyWindow(hwnd); },
            _ => {}
        }
    }
}

unsafe extern "system" fn main_proc(hwnd: Hwnd, message: u32, _w: Wparam, l: Lparam) -> Lresult {
    match message {
        WM_HOTKEY => { start_capture(); 0 }
        WM_APP_TRAY if l as u32 == WM_RBUTTONUP || l as u32 == WM_CONTEXTMENU => { show_tray_menu(hwnd); 0 }
        WM_APP_TRAY if l as u32 == WM_LBUTTONUP => { start_capture(); 0 }
        WM_DISPLAYCHANGE => { close_overlay(); 0 }
        WM_DESTROY => {
            close_overlay();
            UnregisterHotKey(hwnd, 1);
            let fallback = APP.with(|s| s.borrow().fallback_hotkey);
            let mut data = tray_data(hwnd, fallback);
            Shell_NotifyIconW(NIM_DELETE, &mut data);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, _w, l),
    }
}

pub fn run() {
    // SAFETY: This process is single-threaded until after GUI initialization; GDI handles stay local.
    unsafe {
        // Per-Monitor V2 DPI awareness; -4 is DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2.
        SetProcessDpiAwarenessContext(-4);
        let mutex_name = utf16("Local\\RustShot.Windows11.Prototype.v1");
        let mutex = CreateMutexW(null(), 0, mutex_name.as_ptr());
        if mutex == 0 { alert(0, "Cannot create single-instance mutex."); return; }
        if GetLastError() == ERROR_ALREADY_EXISTS { CloseHandle(mutex); return; }
        let inst = GetModuleHandleW(null());
        let main_class = utf16("RustShotMain");
        let overlay_class = utf16("RustShotOverlay");
        let main_wc = WndClass { style: 0, wndproc: Some(main_proc), cls_extra: 0, wnd_extra: 0,
            instance: inst, icon: 0, cursor: LoadCursorW(0, 32512usize as *const u16), background: 0,
            menu_name: null(), class_name: main_class.as_ptr() };
        let overlay_wc = WndClass { wndproc: Some(overlay_proc), cursor: LoadCursorW(0, 32515usize as *const u16),
            class_name: overlay_class.as_ptr(), ..main_wc };
        if RegisterClassW(&main_wc) == 0 || RegisterClassW(&overlay_wc) == 0 {
            alert(0, "Win32 RegisterClassW failed."); CloseHandle(mutex); return;
        }
        let hwnd = CreateWindowExW(0, main_class.as_ptr(), main_class.as_ptr(), 0,
            0, 0, 0, 0, 0, 0, inst, null());
        if hwnd == 0 { alert(0, "Cannot initialize RustShot."); CloseHandle(mutex); return; }
        APP.with(|s| s.borrow_mut().main = hwnd);
        // Hotkey conflicts are not overridden: fall back to Ctrl+Shift+A.
        let fallback = if RegisterHotKey(hwnd, 1, MOD_ALT | MOD_NOREPEAT, b'A' as u32) == 0 {
            if RegisterHotKey(hwnd, 1, MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, b'A' as u32) == 0 {
                alert(hwnd, "Alt+A and Ctrl+Shift+A are both occupied. RustShot will exit.");
                DestroyWindow(hwnd); CloseHandle(mutex); return;
            }
            true
        } else { false };
        APP.with(|s| s.borrow_mut().fallback_hotkey = fallback);
        let mut data = tray_data(hwnd, fallback);
        if Shell_NotifyIconW(NIM_ADD, &mut data) == 0 {
            alert(hwnd, "Unable to add system tray icon.");
            DestroyWindow(hwnd); CloseHandle(mutex); return;
        }
        ShowWindow(hwnd, SW_HIDE);
        let mut msg: Msg = zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        // Let in-flight PNG writes finish before the process exits.
        APP.with(|cell| {
            let mut app = cell.borrow_mut();
            for writer in app.writers.drain(..) { let _ = writer.join(); }
        });
        CloseHandle(mutex);
    }
}
