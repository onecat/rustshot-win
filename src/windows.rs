//! Minimal native Win32/GDI baseline for the first screenshot PoC.
//! No external crates; unsafe FFI calls are isolated here. Windows 11 validation is pending.
use crate::annotations::{self, Annotation, History, Stroke, Tool};
use std::os::windows::ffi::OsStrExt;
use crate::geometry::{Point, Rect};
use crate::selection::Handle as ResizeHandle;
use crate::toolbar;
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
const GM_ADVANCED: i32 = 2;
const BLACKNESS: u32 = 0x00000042;
const WS_POPUP: u32 = 0x80000000;
const WS_CLIPCHILDREN: u32 = 0x02000000;
const WS_EX_TOPMOST: u32 = 0x00000008;
const WS_EX_TOOLWINDOW: u32 = 0x00000080;
const SW_SHOW: i32 = 5;
const SW_HIDE: i32 = 0;
const WM_DESTROY: u32 = 0x0002;
const WM_PAINT: u32 = 0x000F;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_CLOSE: u32 = 0x0010;
const WM_DISPLAYCHANGE: u32 = 0x007E;
const WM_DPICHANGED: u32 = 0x02E0;
const WM_KEYDOWN: u32 = 0x0100;
const WM_IME_STARTCOMPOSITION: u32 = 0x010D;
const WM_IME_ENDCOMPOSITION: u32 = 0x010E;
const WS_CHILD: u32 = 0x40000000;
const WS_VISIBLE: u32 = 0x10000000;
const WS_BORDER: u32 = 0x00800000;
const ES_AUTOHSCROLL: u32 = 0x0080;
const MF_CHECKED: u32 = 0x0008;
const HKEY_CURRENT_USER: Handle = 0x80000001u32 as Handle;
const REG_SZ: u32 = 1;
const REG_DWORD: u32 = 4;
const KEY_QUERY_VALUE: u32 = 0x0001;
const KEY_SET_VALUE: u32 = 0x0002;
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
const VK_V: usize = 0x56;
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
const MAX_CONCURRENT_SAVES: usize = 2;

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
struct BlendFunction {
    blend_op: u8,
    blend_flags: u8,
    constant_alpha: u8,
    alpha_format: u8,
}
// GDI XFORM uses f32 and is layout-compatible with Win32's XFORM.
#[repr(C)]
#[derive(Clone, Copy)]
struct Xform {
    m11: f32,
    m12: f32,
    m21: f32,
    m22: f32,
    dx: f32,
    dy: f32,
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
    fn GetWindowTextLengthW(hwnd: Hwnd) -> i32;
    fn GetWindowTextW(hwnd: Hwnd, buf: *mut u16, capacity: i32) -> i32;
    fn EnumWindows(cb: Option<unsafe extern "system" fn(Hwnd, Lparam) -> i32>, lparam: Lparam) -> i32;
    fn IsWindowVisible(hwnd: Hwnd) -> i32;
    fn IsIconic(hwnd: Hwnd) -> i32;
    fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
    fn GetCursorPos(point: *mut WinPoint) -> i32;
    fn GetForegroundWindow() -> Hwnd;
    fn IsWindow(hwnd: Hwnd) -> i32;
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
    fn CreateCompatibleBitmap(dc: Hdc, width: i32, height: i32) -> Hobj;
    fn PatBlt(dc: Hdc, x: i32, y: i32, w: i32, h: i32, rop: u32) -> i32;
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
    fn RoundRect(dc: Hdc, l: i32, t: i32, r: i32, b: i32, ellipse_w: i32, ellipse_h: i32) -> i32;
    fn Ellipse(dc: Hdc, l: i32, t: i32, r: i32, b: i32) -> i32;
    fn MoveToEx(dc: Hdc, x: i32, y: i32, previous: *mut WinPoint) -> i32;
    fn LineTo(dc: Hdc, x: i32, y: i32) -> i32;
    fn SaveDC(dc: Hdc) -> i32;
    fn SetGraphicsMode(dc: Hdc, mode: i32) -> i32;
    fn SetWorldTransform(dc: Hdc, transform: *const Xform) -> i32;
    fn RestoreDC(dc: Hdc, saved: i32) -> i32;
    fn IntersectClipRect(dc: Hdc, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn CreateSolidBrush(color: u32) -> Hobj;
    fn SetTextColor(dc: Hdc, color: u32) -> u32;
    fn SetBkMode(dc: Hdc, mode: i32) -> i32;
    fn TextOutW(dc: Hdc, x: i32, y: i32, text: *const u16, len: i32) -> i32;
    fn GdiFlush() -> i32;
    fn CreateFontW(height: i32, width: i32, escapement: i32, orientation: i32,
        weight: i32, italic: u32, underline: u32, strikeout: u32, charset: u32,
        output_precision: u32, clip_precision: u32, quality: u32, pitch_family: u32,
        face: *const u16) -> Hobj;
}
#[link(name = "msimg32")]
extern "system" {
    fn AlphaBlend(destination: Hdc, x: i32, y: i32, width: i32, height: i32,
        source: Hdc, src_x: i32, src_y: i32, src_width: i32, src_height: i32,
        blend: BlendFunction) -> i32;
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
#[link(name = "advapi32")]
extern "system" {
    fn RegCreateKeyExW(root: Handle, subkey: *const u16, reserved: u32,
        class: *mut u16, options: u32, access: u32, security: *const c_void,
        result: *mut Handle, disposition: *mut u32) -> u32;
    fn RegQueryValueExW(key: Handle, name: *const u16, reserved: *mut u32,
        value_type: *mut u32, data: *mut u8, size: *mut u32) -> u32;
    fn RegSetValueExW(key: Handle, name: *const u16, reserved: u32, value_type: u32,
        data: *const u8, size: u32) -> u32;
    fn RegDeleteValueW(key: Handle, name: *const u16) -> u32;
    fn RegCloseKey(key: Handle) -> u32;
}
#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(hwnd: Hwnd, attr: u32, dest: *mut c_void, size: u32) -> i32;
}
#[link(name = "ole32")]
extern "system" { fn CoTaskMemFree(ptr: *mut c_void); }

fn utf16(s: &str) -> Vec<u16> { s.encode_utf16().chain(std::iter::once(0)).collect() }
fn low_word_signed(l: Lparam) -> i32 { l as u16 as i16 as i32 }
fn high_word_signed(l: Lparam) -> i32 { (l >> 16) as u16 as i16 as i32 }
fn bitmap_info(w: i32, h: i32) -> BitmapInfo {
    BitmapInfo { header: BitmapInfoHeader {
        size: size_of::<BitmapInfoHeader>() as u32, width: w, height: -h,
        planes: 1, bit_count: 32, compression: BI_RGB, size_image: 0,
        x_pels_per_meter: 0, y_pels_per_meter: 0, clr_used: 0, clr_important: 0,
    }, colors: [0] }
}

struct Frame { bounds: Rect, pixels: Vec<u8>, windows: Vec<Rect> }
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
    text_edit: Hwnd,
    text_anchor: Point,
    ime_composing: bool,
    tool: Tool,
    stroke: Stroke,
    color_index: usize,
    auto_save: bool,
    auto_copy: bool,
    toolbar_hover: Option<usize>,
    history: History,
    drag_anchor: Point,
    drag_initial: Rect,
    fallback_hotkey: bool,
    restore_focus: Hwnd,
    writers: Vec<thread::JoinHandle<()>>,
}
impl AppState {
    fn new() -> Self {
        Self { main: 0, overlay: 0, frame: None, hover: None, selected: None,
            dragging: false, moving: false, resizing: None, active_annotation: None, text_edit: 0, text_anchor: Point::default(),
            ime_composing: false,
            tool: Tool::Select, stroke: Stroke::default(), color_index:0, auto_save:true, auto_copy:true, toolbar_hover: None, history: History::default(), drag_anchor: Point::default(), drag_initial: Rect::default(), fallback_hotkey: false, restore_focus: 0, writers: Vec::new() }
    }
}
thread_local! { static APP: RefCell<AppState> = RefCell::new(AppState::new()); }

fn virtual_desktop() -> Rect {
    // SAFETY: GetSystemMetrics does not touch pointers and returns pixel dimensions on this DPI-aware thread.
    unsafe {
        let x = GetSystemMetrics(76);
        let y = GetSystemMetrics(77);
        Rect { left: x, top: y, right: x.saturating_add(GetSystemMetrics(78)), bottom: y.saturating_add(GetSystemMetrics(79)) }
    }
}

fn capture_desktop() -> Result<Frame, String> {
    let bounds = virtual_desktop();
    let width = bounds.width();
    let height = bounds.height();
    if width <= 0 || height <= 0 { return Err("Unsupported virtual desktop size".into()); }
    let count = (width as usize).checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4)).ok_or("Desktop image size overflow")?;
    if count > MAX_CAPTURE_BYTES { return Err("Desktop exceeds 512 MiB screenshot limit".into()); }
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
        Ok(Frame { bounds, pixels, windows: enumerate_windows(bounds) })
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
    // LPARAM packs signed 16-bit client coordinates and truncates desktop spans
    // wider than 32767 pixels. GetCursorPos supplies 32-bit virtual-desktop
    // screen coordinates in this Per-Monitor-V2 DPI-aware process.
    let mut cursor = WinPoint::default();
    if unsafe { GetCursorPos(&mut cursor) } != 0 {
        Point { x: cursor.x, y: cursor.y }
    } else {
        Point { x: frame.bounds.left.saturating_add(low_word_signed(l)),
            y: frame.bounds.top.saturating_add(high_word_signed(l)) }
    }
}
fn toolbar_for(selection: Rect, bounds: Rect) -> Rect {
    toolbar::rect_for(selection, bounds)
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
        Tool::Text => {
            let words = utf16(&a.text);
            if words.len() > 1 {
                TextOutW(dc, first.x - origin.x, first.y - origin.y,
                    words.as_ptr(), words.len() as i32 - 1);
            }
        }
        Tool::Mosaic | Tool::Select => {}
    }
}

fn paint_annotations(dc: Hdc, frame: &Frame, selection: Rect, history: &History,
    current: Option<&Annotation>) {
    let origin = Point { x: frame.bounds.left, y: frame.bounds.top };
    unsafe {
        let saved = SaveDC(dc);
        if saved == 0 { return; }
        let area = selection.translated(-origin.x, -origin.y);
        IntersectClipRect(dc, area.left, area.top, area.right, area.bottom);
        let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        SetBkMode(dc, TRANSPARENT);
        for a in history.committed.iter().chain(current) {
            if a.tool == Tool::Mosaic {
                preview_mosaic(dc, frame, selection, a);
                continue;
            }
            let color = u32::from_le_bytes([a.stroke.bgra[2], a.stroke.bgra[1], a.stroke.bgra[0], 0]);
            let pen = CreatePen(PS_SOLID, i32::from(a.stroke.width), color);
            if pen == 0 { continue; }
            let old_pen = SelectObject(dc, pen);
            SetTextColor(dc, color);
            let font = create_annotation_font(a.stroke.width);
            let old_font = if font != 0 { SelectObject(dc, font) } else { 0 };
            preview_annotation(dc, a, origin);
            if font != 0 { SelectObject(dc, old_font); DeleteObject(font); }
            SelectObject(dc, old_pen);
            DeleteObject(pen);
        }
        SelectObject(dc, old_brush);
        RestoreDC(dc, saved);
    }
}


fn create_annotation_font(width: u8) -> Hobj {
    let face = utf16("Microsoft YaHei UI");
    let height = -(26 + i32::from(width.saturating_sub(1)) * 2);
    unsafe {
        CreateFontW(height, 0, 0, 0, 600, 0, 0, 0, 1, 0, 0, 4, 0, face.as_ptr())
    }
}

// Render text into the pixel buffer used for both clipboard and PNG. Keep
// non-text raster operations before/after each label in original history order.
fn render_text_to_pixels(pixels: &mut [u8], width: u32, height: u32,
    origin: Point, annotation: &Annotation) {
    if annotation.tool != Tool::Text || annotation.text.is_empty() ||
        width == 0 || height == 0 || width > i32::MAX as u32 ||
        height > i32::MAX as u32 { return; }
    unsafe {
        let screen = GetDC(0);
        if screen == 0 { return; }
        let dc = CreateCompatibleDC(screen);
        if dc == 0 { ReleaseDC(0, screen); return; }
        let mut bits: *mut c_void = null_mut();
        let info = bitmap_info(width as i32, height as i32);
        let bmp = CreateDIBSection(screen, &info, DIB_RGB_COLORS, &mut bits, 0, 0);
        if bmp == 0 || bits.is_null() {
            if bmp != 0 { DeleteObject(bmp); }
            DeleteDC(dc);
            ReleaseDC(0, screen);
            return;
        }
        let old_bmp = SelectObject(dc, bmp);
        std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
        let font = create_annotation_font(annotation.stroke.width);
        let old_font = if font != 0 { SelectObject(dc, font) } else { 0 };
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, u32::from_le_bytes([
            annotation.stroke.bgra[2], annotation.stroke.bgra[1],
            annotation.stroke.bgra[0], 0
        ]));
        let string = utf16(&annotation.text);
        let position = annotation.points[0];
        TextOutW(dc, position.x - origin.x, position.y - origin.y,
            string.as_ptr(), string.len() as i32 - 1);
        GdiFlush();
        std::ptr::copy_nonoverlapping(bits as *const u8, pixels.as_mut_ptr(), pixels.len());
        if font != 0 { SelectObject(dc, old_font); DeleteObject(font); }
        if old_bmp != 0 { SelectObject(dc, old_bmp); }
        DeleteObject(bmp);
        DeleteDC(dc);
        ReleaseDC(0, screen);
    }
}

// Draw the exact pixel blocks from the captured frame, not an ornamental
// checkerboard. Use small temporary buffers only for the obscured region.
unsafe fn preview_mosaic(dc: Hdc, frame: &Frame, selection: Rect, a: &Annotation) {
    if a.points.len() < 2 { return; }
    let clipped = Rect::from_points(a.points[0], a.points[1])
        .intersection(selection)
        .and_then(|r| r.intersection(frame.bounds));
    let Some(rect) = clipped else { return; };
    let w = rect.width() as usize;
    let h = rect.height() as usize;
    let Some(bytes) = w.checked_mul(h).and_then(|v| v.checked_mul(4)) else { return; };
    if bytes == 0 || bytes > MAX_CAPTURE_BYTES { return; }
    let mut crop = Vec::with_capacity(bytes);
    for y in 0..h {
        let frame_y = (rect.top - frame.bounds.top) as usize + y;
        let frame_x = (rect.left - frame.bounds.left) as usize;
        let offset = (frame_y * frame.bounds.width() as usize + frame_x) * 4;
        crop.extend_from_slice(&frame.pixels[offset..offset + w * 4]);
    }
    annotations::render(&mut crop, w as u32, h as u32,
        Point { x: rect.left, y: rect.top }, std::slice::from_ref(a));
    let info = bitmap_info(rect.width(), rect.height());
    StretchDIBits(dc, rect.left - frame.bounds.left, rect.top - frame.bounds.top,
        rect.width(), rect.height(), 0, 0, rect.width(), rect.height(),
        crop.as_ptr() as *const c_void, &info, DIB_RGB_COLORS, SRCCOPY);
}

// A stretched, constant-alpha black bitmap dims the frozen screenshot using
// only a 1x1 GDI bitmap. No second full-desktop BGRA buffer is retained.
fn draw_dimming_layer(target: Hdc, width: i32, height: i32) {
    if width <= 0 || height <= 0 { return; }
    unsafe {
        let source = CreateCompatibleDC(target);
        if source == 0 { return; }
        let bitmap = CreateCompatibleBitmap(target, 1, 1);
        if bitmap == 0 {
            DeleteDC(source);
            return;
        }
        let previous = SelectObject(source, bitmap);
        if previous != 0 {
            PatBlt(source, 0, 0, 1, 1, BLACKNESS);
            AlphaBlend(target, 0, 0, width, height, source, 0, 0, 1, 1,
                BlendFunction { blend_op: 0, blend_flags: 0,
                    constant_alpha: 136, alpha_format: 0 });
            SelectObject(source, previous);
        }
        DeleteObject(bitmap);
        DeleteDC(source);
    }
}


// The overlay is always composited away from the screen, then presented as one
// BitBlt. Painting several GDI layers directly to the window caused visible
// black/white flashes while moving or resizing the selection.
struct PaintSurface {
    dc: Hdc,
    bitmap: Hobj,
    previous: Hobj,
    width: i32,
    height: i32,
}

impl PaintSurface {
    fn new(screen: Hdc, width: i32, height: i32) -> Option<Self> {
        if screen == 0 || width <= 0 || height <= 0 { return None; }
        // SAFETY: compatible DC and selected bitmap are owned by this object.
        unsafe {
            let dc = CreateCompatibleDC(screen);
            if dc == 0 { return None; }
            let bitmap = CreateCompatibleBitmap(screen, width, height);
            if bitmap == 0 {
                DeleteDC(dc);
                return None;
            }
            let previous = SelectObject(dc, bitmap);
            if previous == 0 || previous == -1 {
                DeleteObject(bitmap);
                DeleteDC(dc);
                return None;
            }
            Some(Self { dc, bitmap, previous, width, height })
        }
    }
}

impl Drop for PaintSurface {
    fn drop(&mut self) {
        // SAFETY: restore the stock bitmap before deleting the allocated one.
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

thread_local! {
    // Owned only while the screenshot overlay is active. It is released on
    // cancel, export, DPI change, or overlay destruction.
    static PAINT_SURFACE: RefCell<Option<PaintSurface>> = const { RefCell::new(None) };
}

// Native, self-drawn line icons: no bundled Tencent/WeChat assets.
// The 11 action IDs map to existing RustShot tools, preserving behavior.
unsafe fn toolbar_line(dc: Hdc, x1: i32, y1: i32, x2: i32, y2: i32) {
    MoveToEx(dc, x1, y1, null_mut());
    LineTo(dc, x2, y2);
}

unsafe fn toolbar_rounded(dc: Hdc, rect: Rect, radius: i32, fill: u32, outline: u32) {
    let brush = CreateSolidBrush(fill);
    let pen = CreatePen(PS_SOLID, 1, outline);
    if brush != 0 && pen != 0 {
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(dc, rect.left, rect.top, rect.right, rect.bottom, radius, radius);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
    }
    if pen != 0 { DeleteObject(pen); }
    if brush != 0 { DeleteObject(brush); }
}

fn tool_for_action(action: usize) -> Option<Tool> {
    match action {
        1 => Some(Tool::Select),
        2 => Some(Tool::Rectangle),
        3 => Some(Tool::Ellipse),
        4 => Some(Tool::Arrow),
        5 => Some(Tool::Pen),
        6 => Some(Tool::Text),
        7 => Some(Tool::Mosaic),
        _ => None,
    }
}

unsafe fn draw_toolbar_icon(dc: Hdc, bounds: Rect, item: toolbar::Item) {
    use toolbar::Glyph as G;
    let cx = bounds.left + bounds.width() / 2;
    let cy = bounds.top + bounds.height() / 2;
    // COLORREF is 0x00BBGGRR, matching the supplied screenshot's muted ink.
    let color = if item.action.is_none() { 0x00B0B0B0 } else {
        match item.glyph {
            G::Cancel => 0x004B4BF0, // WeChat-style red X
            G::Finish => 0x006BC65C, // WeChat-style green check
            _ => 0x00353535,
        }
    };
    // Slightly enlarge every vector glyph (including text and pen strokes),
    // centered inside its resized hit target. Win32 advanced GDI mode scales
    // existing drawing commands without changing the screenshot image itself.
    let saved = SaveDC(dc);
    if saved != 0 && SetGraphicsMode(dc, GM_ADVANCED) != 0 {
        let scale = if bounds.height() >= 44 { 1.35_f32 } else { 1.32_f32 };
        let transform = Xform {
            m11: scale, m12: 0.0, m21: 0.0, m22: scale,
            dx: (1.0-scale) * cx as f32,
            dy: (1.0-scale) * cy as f32,
        };
        // Failure falls back to unscaled painting, never an incomplete icon.
        SetWorldTransform(dc, &transform);
    }
    let size = if bounds.height() >= 44 { 10 } else { 8 };
    let pen = CreatePen(PS_SOLID, if bounds.height() >= 44 { 2 } else { 1 }, color);
    if pen == 0 {
        if saved != 0 { RestoreDC(dc, saved); }
        return;
    }
    let old_pen = SelectObject(dc, pen);
    let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
    match item.glyph {
        G::Rectangle => { Rectangle(dc, cx-size+1, cy-size+1, cx+size, cy+size); }
        G::Ellipse => { Ellipse(dc, cx-size, cy-size, cx+size+1, cy+size+1); }
        G::Emoji => {
            Ellipse(dc, cx-size, cy-size, cx+size+1, cy+size+1);
            let dot=CreateSolidBrush(color);
            if dot!=0 {
                for x in [cx-4, cx+3] {
                    let r=WinRect {left:x,top:cy-4,right:x+3,bottom:cy-1};
                    FillRect(dc,&r,dot);
                }
                DeleteObject(dot);
            }
            toolbar_line(dc,cx-6,cy+3,cx-3,cy+6);
            toolbar_line(dc,cx-3,cy+6,cx+3,cy+6);
            toolbar_line(dc,cx+3,cy+6,cx+6,cy+3);
        }
        G::Arrow => {
            toolbar_line(dc,cx-size,cy+size,cx+size-1,cy-size+1);
            toolbar_line(dc,cx-1,cy-size+1,cx+size-1,cy-size+1);
            toolbar_line(dc,cx+size-1,cy-size+1,cx+size-1,cy);
        }
        G::Pen => {
            toolbar_line(dc,cx-9,cy+8,cx+6,cy-7);
            toolbar_line(dc,cx-7,cy+10,cx+8,cy-5);
            toolbar_line(dc,cx+6,cy-7,cx+8,cy-5);
        }
        G::Mosaic => {
            Rectangle(dc,cx-9,cy-9,cx+10,cy+10);
            let tile=CreateSolidBrush(color);
            if tile!=0 {
                for y in 0..3 {
                    for x in 0..3 {
                        if (x+y)%2==0 {
                            let tx=cx-7+x*6;
                            let ty=cy-7+y*6;
                            let r=WinRect {left:tx,top:ty,right:tx+5,bottom:ty+5};
                            FillRect(dc,&r,tile);
                        }
                    }
                }
                DeleteObject(tile);
            }
        }
        G::Text => {
            Rectangle(dc,cx-9,cy-9,cx+10,cy+10);
            let letter=utf16("T");
            let face=utf16("Segoe UI");
            let font=CreateFontW(17,0,0,0,500,0,0,0,1,0,0,4,0,face.as_ptr());
            let old_font=if font!=0 {SelectObject(dc,font)}else{0};
            SetTextColor(dc,color);
            SetBkMode(dc,TRANSPARENT);
            TextOutW(dc,cx-5,cy-9,letter.as_ptr(),1);
            if font!=0 {SelectObject(dc,old_font);DeleteObject(font);}
        }
        G::Translate => {
            Rectangle(dc,cx-10,cy-3,cx+2,cy+9);
            Rectangle(dc,cx-1,cy-10,cx+11,cy+3);
            let font_face=utf16("Segoe UI");
            let font=CreateFontW(12,0,0,0,500,0,0,0,1,0,0,4,0,font_face.as_ptr());
            let old_font=if font!=0 {SelectObject(dc,font)}else{0};
            SetTextColor(dc,color);
            SetBkMode(dc,TRANSPARENT);
            let a=utf16("A");
            let wen=utf16("文");
            TextOutW(dc,cx+1,cy-9,a.as_ptr(),1);
            TextOutW(dc,cx-9,cy-2,wen.as_ptr(),1);
            if font!=0 {SelectObject(dc,old_font);DeleteObject(font);}
        }
        G::Ocr => {
            // Four scan brackets around A, as in the supplied WeChat reference.
            for (sx,sy) in [(-10,-10),(10,-10),(-10,10),(10,10)] {
                let inward=if sx<0 {4}else{-4};
                let vert=if sy<0 {4}else{-4};
                toolbar_line(dc,cx+sx,cy+sy,cx+sx+inward,cy+sy);
                toolbar_line(dc,cx+sx,cy+sy,cx+sx,cy+sy+vert);
            }
            let a=utf16("A");
            let face=utf16("Segoe UI");
            let font=CreateFontW(16,0,0,0,500,0,0,0,1,0,0,4,0,face.as_ptr());
            let old=if font!=0{SelectObject(dc,font)}else{0};
            SetTextColor(dc,color);
            SetBkMode(dc,TRANSPARENT);
            TextOutW(dc,cx-5,cy-8,a.as_ptr(),1);
            if font!=0{SelectObject(dc,old);DeleteObject(font);}
        }
        G::Scroll => {
            for v in [-10,10] {
                toolbar_line(dc,cx+v,cy-9,cx+v,cy-4);
                toolbar_line(dc,cx+v,cy+4,cx+v,cy+9);
            }
            for v in [-9,9] {
                toolbar_line(dc,cx-10,cy+v,cx-4,cy+v);
                toolbar_line(dc,cx+4,cy+v,cx+10,cy+v);
            }
            toolbar_line(dc,cx,cy-7,cx,cy+7);
            toolbar_line(dc,cx-4,cy-3,cx,cy-7);
            toolbar_line(dc,cx+4,cy-3,cx,cy-7);
            toolbar_line(dc,cx-4,cy+3,cx,cy+7);
            toolbar_line(dc,cx+4,cy+3,cx,cy+7);
        }
        G::Undo => {
            toolbar_line(dc,cx-9,cy-2,cx-3,cy-8);
            toolbar_line(dc,cx-9,cy-2,cx-1,cy-2);
            toolbar_line(dc,cx-9,cy-2,cx-4,cy+2);
            toolbar_line(dc,cx-3,cy-8,cx+5,cy-6);
            toolbar_line(dc,cx+5,cy-6,cx+9,cy);
            toolbar_line(dc,cx+9,cy,cx+7,cy+7);
            toolbar_line(dc,cx+7,cy+7,cx+2,cy+8);
        }
        G::Save => {
            toolbar_line(dc,cx-8,cy-1,cx-8,cy+9);
            toolbar_line(dc,cx-8,cy+9,cx+8,cy+9);
            toolbar_line(dc,cx+8,cy+9,cx+8,cy-1);
            toolbar_line(dc,cx,cy-11,cx,cy+4);
            toolbar_line(dc,cx-5,cy-1,cx,cy+4);
            toolbar_line(dc,cx+5,cy-1,cx,cy+4);
        }
        G::Pin => {
            toolbar_line(dc,cx-5,cy-10,cx+5,cy-10);
            toolbar_line(dc,cx-5,cy-10,cx-3,cy-3);
            toolbar_line(dc,cx+5,cy-10,cx+3,cy-3);
            toolbar_line(dc,cx-3,cy-3,cx-9,cy+4);
            toolbar_line(dc,cx+3,cy-3,cx+9,cy+4);
            toolbar_line(dc,cx-9,cy+4,cx+9,cy+4);
            toolbar_line(dc,cx,cy+4,cx,cy+11);
        }
        G::Share => {
            toolbar_line(dc,cx-8,cy+8,cx-8,cy+1);
            toolbar_line(dc,cx-8,cy+1,cx-1,cy-4);
            toolbar_line(dc,cx-1,cy-4,cx+5,cy-4);
            toolbar_line(dc,cx+5,cy-4,cx+5,cy-10);
            toolbar_line(dc,cx+5,cy-10,cx+11,cy-2);
            toolbar_line(dc,cx+11,cy-2,cx+5,cy+5);
            toolbar_line(dc,cx+5,cy+5,cx+5,cy);
            toolbar_line(dc,cx+5,cy,cx,cy);
            toolbar_line(dc,cx,cy,cx-8,cy+8);
        }
        G::Cancel => {
            toolbar_line(dc,cx-8,cy-8,cx+8,cy+8);
            toolbar_line(dc,cx+8,cy-8,cx-8,cy+8);
        }
        G::Finish => {
            toolbar_line(dc,cx-10,cy,cx-3,cy+7);
            toolbar_line(dc,cx-3,cy+7,cx+10,cy-8);
        }
    }
    SelectObject(dc,old_brush);
    SelectObject(dc,old_pen);
    DeleteObject(pen);
    if saved != 0 { RestoreDC(dc, saved); }
}

fn paint_stroke_palette(dc: Hdc, selection: Rect, desktop: Rect,
    color_index: usize, stroke_width: u8) {
    let bar=toolbar_for(selection,desktop);
    let palette=toolbar::palette_rect(bar,desktop);
    let p=palette.translated(-desktop.left,-desktop.top);
    unsafe {
        let saved=SaveDC(dc);
        if saved==0 {return;}
        toolbar_rounded(dc,p,10,0x00FFFFFF,0x00DCDCDC);
        for index in 0..toolbar::PALETTE_OPTIONS {
            let left=p.left + index as i32*p.width()/toolbar::PALETTE_OPTIONS as i32;
            let right=p.left + (index as i32+1)*p.width()/toolbar::PALETTE_OPTIONS as i32;
            let cx=(left+right)/2;
            let cy=(p.top+p.bottom)/2;
            if index<toolbar::PALETTE_COLORS {
                let col=Stroke::COLORS[index];
                let rgb=u32::from_le_bytes([col[2],col[1],col[0],0]);
                let b=CreateSolidBrush(rgb);
                if b!=0 {
                    let old=SelectObject(dc,b);
                    let pen=CreatePen(PS_SOLID,if index==color_index {3}else{1},
                        if index==color_index {0x00666666}else{0x00EEEEEE});
                    if pen!=0 {
                        let old_pen=SelectObject(dc,pen);
                        Ellipse(dc,cx-9,cy-9,cx+10,cy+10);
                        SelectObject(dc,old_pen);DeleteObject(pen);
                    }
                    SelectObject(dc,old);DeleteObject(b);
                }
            }else{
                let w=[2u8,4,7][index-5];
                let pen=CreatePen(PS_SOLID,i32::from(w),0x00444444);
                if pen!=0 {
                    let old=SelectObject(dc,pen);
                    MoveToEx(dc,cx-11,cy,null_mut()); LineTo(dc,cx+11,cy);
                    SelectObject(dc,old);DeleteObject(pen);
                }
                if w==stroke_width {
                    let mark=CreatePen(PS_SOLID,1,0x00C5B24B);
                    if mark!=0 {
                        let old=SelectObject(dc,mark);
                        let previous=SelectObject(dc,GetStockObject(NULL_BRUSH));
                        RoundRect(dc,left+3,p.top+3,right-3,p.bottom-3,7,7);
                        SelectObject(dc,previous);SelectObject(dc,old);DeleteObject(mark);
                    }
                }
            }
        }
        RestoreDC(dc,saved);
    }
}

fn draw_toolbar(dc: Hdc, selection: Rect, desktop: Rect, active: Tool,
    hover: Option<usize>, can_undo: bool, _can_redo: bool) {
    let bar = toolbar_for(selection, desktop);
    let origin = Point { x: desktop.left, y: desktop.top };
    let local = bar.translated(-origin.x, -origin.y);
    unsafe {
        let saved = SaveDC(dc);
        if saved == 0 { return; }
        let radius = if toolbar::is_compact(desktop) { 11 } else { 16 };
        // Soft one-pixel outline and low elevation, matching supplied images.
        toolbar_rounded(dc, Rect {
            left: local.left + 1, top: local.top + 3,
            right: local.right + 1, bottom: local.bottom + 3,
        }, radius, 0x00C9C9C9, 0x00C9C9C9);
        toolbar_rounded(dc, local, radius, 0x00FFFFFF, 0x00E8E8E8);
        let all = toolbar::items(bar);
        for (slot, item) in all.iter().copied().enumerate() {
            let Some(cell) = toolbar::slot_rect(bar, slot) else { continue; };
            let rect = cell.translated(-origin.x, -origin.y);
            let chosen = item.action.and_then(tool_for_action).is_some_and(|tool| tool == active);
            let disabled = item.action.is_none() ||
                (item.glyph == toolbar::Glyph::Undo && !can_undo);
            if chosen || (hover == Some(slot) && !disabled) {
                let padded = Rect { left: rect.left + 2, top: rect.top + 1,
                    right: rect.right - 2, bottom: rect.bottom - 1 };
                if padded.right > padded.left && padded.bottom > padded.top {
                    let fill = if chosen { 0x00EAF7E8 } else { 0x00F6F6F6 };
                    toolbar_rounded(dc, padded, radius, fill, fill);
                }
            }
            // The undo action is disabled while empty even though still visible.
            let mut icon = item;
            if item.glyph == toolbar::Glyph::Undo && !can_undo {
                icon.action = None;
            }
            draw_toolbar_icon(dc, rect, icon);
        }

        for &(left,right) in toolbar::separators(bar) {
            let (Some(a), Some(b)) =
                (toolbar::slot_rect(bar, left), toolbar::slot_rect(bar, right))
            else { continue; };
            let x = (a.right + b.left)/2 - origin.x;
            let brush = CreateSolidBrush(0x00DCDCDC);
            if brush != 0 {
                let line = WinRect { left: x, right: x+1,
                    top: local.top + 12, bottom: local.bottom - 12 };
                FillRect(dc, &line, brush);
                DeleteObject(brush);
            }
        }

        // WeChat displays labels under the bar, unless there is no room.
        if let Some(index) = hover {
            if let Some(cell) = toolbar::slot_rect(bar, index) {
                let label = all[index].label;
                // Chinese glyphs are much wider than ASCII at -16 character
                // height; avoid clipping longer disabled-action tooltips.
                let text_width: i32 = label.chars()
                    .map(|ch| if ch.is_ascii() { 9 } else { 17 }).sum();
                let width = (text_width + 35).clamp(72, 300).min(desktop.width());
                let left = (cell.left + cell.width()/2 - width/2)
                    .clamp(desktop.left, desktop.right.saturating_sub(width));
                let y = if bar.bottom.saturating_add(39) <= desktop.bottom {
                    bar.bottom + 4
                } else {
                    bar.top.saturating_sub(37).max(desktop.top)
                };
                let tip = Rect { left: left-origin.x, top: y-origin.y,
                    right: left+width-origin.x, bottom: y+32-origin.y };
                toolbar_rounded(dc, tip, 5, 0x00484848, 0x00484848);
                let glyphs = utf16(label);
                let face = utf16("Microsoft YaHei UI");
                let font = CreateFontW(-19,0,0,0,500,0,0,0,1,0,0,4,0,face.as_ptr());
                let old = if font != 0 { SelectObject(dc, font) } else { 0 };
                SetTextColor(dc,0x00FFFFFF);
                SetBkMode(dc,TRANSPARENT);
                TextOutW(dc,tip.left + 9,tip.top + 5,glyphs.as_ptr(),glyphs.len() as i32-1);
                if font != 0 {SelectObject(dc,old);DeleteObject(font);}
            }
        }
        RestoreDC(dc,saved);
    }
}

fn paint_overlay(hwnd: Hwnd) {
    // SAFETY: BeginPaint/EndPaint are paired; HDC is only used inside the paint call.
    unsafe {
        let mut ps: PaintStruct = zeroed();
        let screen_dc = BeginPaint(hwnd, &mut ps);
        if screen_dc == 0 { return; }
        APP.with(|cell| {
            let state = cell.borrow();
            let Some(frame) = &state.frame else { return; };
            PAINT_SURFACE.with(|cell| {
                let mut surface = cell.borrow_mut();
                let width = frame.bounds.width();
                let height = frame.bounds.height();
                if surface.as_ref().is_none_or(|p| p.width != width || p.height != height) {
                    *surface = PaintSurface::new(screen_dc, width, height);
                }
                // In allocation failure, paint directly so the app remains
                // functional; normal operation always uses the backbuffer.
                let dc = surface.as_ref().map_or(screen_dc, |p| p.dc);
                let info = bitmap_info(width, height);
                StretchDIBits(dc, 0, 0, frame.bounds.width(), frame.bounds.height(), 0, 0,
                frame.bounds.width(), frame.bounds.height(), frame.pixels.as_ptr() as *const c_void,
                &info, DIB_RGB_COLORS, SRCCOPY);
            draw_dimming_layer(dc, frame.bounds.width(), frame.bounds.height());
            let active = state.selected.or(state.hover);
            if let Some(selection) = active.and_then(|s| s.intersection(frame.bounds)) {
                let rel = selection.translated(-frame.bounds.left, -frame.bounds.top);
                StretchDIBits(dc, rel.left, rel.top, rel.width(), rel.height(),
                    rel.left, rel.top, rel.width(), rel.height(),
                    frame.pixels.as_ptr() as *const c_void, &info, DIB_RGB_COLORS, SRCCOPY);
                // Match the reference green crop stroke and eight small solid
                // edge handles; the old white squares were visibly incorrect.
                let border_color = if toolbar::is_compact(frame.bounds) {
                    0x0019AD1A // RGB(26,173,25): compact reference
                } else {
                    0x0080BC5F // RGB(95,188,128): wide reference
                };
                let pen = CreatePen(PS_SOLID, 2, border_color);
                if pen != 0 {
                    let old_pen = SelectObject(dc, pen);
                    let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
                    Rectangle(dc, rel.left, rel.top, rel.right, rel.bottom);
                    SelectObject(dc, old_brush);
                    SelectObject(dc, old_pen);
                    DeleteObject(pen);
                }
                // Enlarge the top-left screenshot dimensions from v0.6, keeping the same anchor.
                SetBkMode(dc, TRANSPARENT);
                SetTextColor(dc, 0x00FFFFFF);
                let caption = format!("{} x {}", selection.width(), selection.height());
                let label = utf16(&caption);
                let face = utf16("Segoe UI");
                let font = CreateFontW(-23,0,0,0,600,0,0,0,1,0,0,4,0,face.as_ptr());
                let old_font = if font != 0 { SelectObject(dc, font) } else { 0 };
                TextOutW(dc, rel.left + 1, rel.top.saturating_sub(31).max(0),
                    label.as_ptr(), label.len() as i32 - 1);
                if font != 0 { SelectObject(dc, old_font); DeleteObject(font); }
                if state.selected.is_some() {
                    paint_annotations(dc, frame, selection, &state.history,
                        state.active_annotation.as_ref());
                    let handle_brush = CreateSolidBrush(border_color);
                    if handle_brush != 0 {
                        for handle in ResizeHandle::ALL {
                            let center = handle.center(selection);
                            let x = center.x - frame.bounds.left;
                            let y = center.y - frame.bounds.top;
                            let mark = WinRect { left: x - 3, top: y - 3,
                                right: x + 4, bottom: y + 4 };
                            FillRect(dc, &mark, handle_brush);
                        }
                        DeleteObject(handle_brush);
                    }

                    draw_toolbar(dc, selection, frame.bounds, state.tool,
                        state.toolbar_hover, !state.history.committed.is_empty(),
                        !state.history.undone.is_empty());
                    if state.tool != Tool::Select {
                        paint_stroke_palette(dc,selection,frame.bounds,
                            state.color_index,state.stroke.width);
                    }
                }
            }
            if surface.is_some() {
                    // BeginPaint clips the destination to rc_paint. Present
                    // only invalidated pixels after ALL rendering is complete.
                    let update = ps.rc_paint;
                    let w = update.right.saturating_sub(update.left);
                    let h = update.bottom.saturating_sub(update.top);
                    if w > 0 && h > 0 {
                        BitBlt(screen_dc, update.left, update.top, w, h, dc,
                            update.left, update.top, SRCCOPY);
                    }
                }
            });
        });
        EndPaint(hwnd, &ps);
    }
}

fn alert(hwnd: Hwnd, message: &str) {
    let msg = utf16(message);
    let title = utf16("RustShot v0.8.0");
    // SAFETY: UTF-16 buffers are NUL terminated for the duration of MessageBoxW.
    unsafe { MessageBoxW(hwnd, msg.as_ptr(), title.as_ptr(), 0x30); }
}

fn start_capture() {
    let existing = APP.with(|s| s.borrow().overlay != 0);
    if existing { return; }
    // Snapshot the previously active window only when it belongs to another
    // process. Tray menu deliberately foregrounds the hidden main window.
    let previous = unsafe { GetForegroundWindow() };
    APP.with(|cell| {
        let mut state = cell.borrow_mut();
        state.restore_focus = if previous == state.main { 0 } else { previous };
    });
    match capture_desktop() {
        Ok(frame) => {
            let bounds = frame.bounds;
            APP.with(|s| { s.borrow_mut().frame = Some(frame); });
            let name = utf16("RustShotOverlay");
            // SAFETY: The class is registered in run() and the module instance remains loaded.
            let hwnd = unsafe { CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                name.as_ptr(), name.as_ptr(), WS_POPUP | WS_CLIPCHILDREN,
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
    cancel_text_edit();
    // Release the selected bitmap/DC before freeing the captured frame.
    PAINT_SURFACE.with(|cell| { *cell.borrow_mut() = None; });
    let hwnd = APP.with(|cell| {
        let mut s = cell.borrow_mut();
        s.frame = None;
        s.selected = None;
        s.hover = None;
        s.dragging = false;
        s.moving = false;
        s.resizing = None;
        s.active_annotation = None;
        s.text_edit = 0;
        s.ime_composing = false;
        s.tool = Tool::Select;
        s.toolbar_hover = None;
        s.history.clear();
        let previous = std::mem::replace(&mut s.restore_focus, 0);
        (std::mem::replace(&mut s.overlay, 0), previous)
    });
    let (hwnd, previous) = hwnd;
    if hwnd != 0 {
        unsafe {
            ReleaseCapture();
            DestroyWindow(hwnd);
            if previous != 0 && IsWindow(previous) != 0 { SetForegroundWindow(previous); }
        }
    }
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
        for a in &state.history.committed {
            if a.tool == Tool::Text {
                render_text_to_pixels(&mut crop, rect.width() as u32, rect.height() as u32,
                    Point { x: rect.left, y: rect.top }, a);
            } else {
                annotations::render(&mut crop, rect.width() as u32, rect.height() as u32,
                    Point { x: rect.left, y: rect.top }, std::slice::from_ref(a));
            }
        }
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
    commit_text_edit();
    let picture = selected_image();
    let (owner,auto_copy,auto_save) = APP.with(|cell| {
        let s=cell.borrow();
        (s.main,s.auto_copy,s.auto_save)
    });
    close_overlay();
    let Some((w,h,pixels))=picture else {return};
    let copied = if auto_copy {Some(put_clipboard(owner,w,h,&pixels))} else {None};
    if auto_save {
        // Release finished writers; never queue more than two full BGRA crops.
        let oldest=APP.with(|cell| {
            let mut app=cell.borrow_mut();
            let mut i=0;
            while i<app.writers.len() {
                if app.writers[i].is_finished() {
                    let finished=app.writers.remove(i);
                    let _=finished.join();
                } else {i+=1;}
            }
            if app.writers.len()>=MAX_CONCURRENT_SAVES {
                Some(app.writers.remove(0))
            } else {None}
        });
        if let Some(worker)=oldest {let _=worker.join();}
        let worker=thread::spawn(move || {
            if let Err(err)=save_png(w,h,&pixels) {
                alert(0,&format!("Screenshot PNG save failed: {err}"));
            }
        });
        APP.with(|cell| cell.borrow_mut().writers.push(worker));
    }
    if let Some(Err(err))=copied {
        alert(owner,&format!("Clipboard copy failed; automatic PNG saving setting is independent: {err}"));
    }
}

fn start_text_edit(overlay: Hwnd) {
    cancel_text_edit();
    let (position, bounds) = APP.with(|cell| {
        let state = cell.borrow();
        let bounds = state.frame.as_ref().map(|f| f.bounds).unwrap_or_default();
        (state.drag_anchor, bounds)
    });
    if bounds.width() <= 0 || bounds.height() <= 0 { return; }
    let x = (position.x - bounds.left).clamp(0, (bounds.width() - 1).max(0));
    let y = (position.y - bounds.top).clamp(0, (bounds.height() - 1).max(0));
    let width = 300.min((bounds.width() - x).max(1));
    let height = 40.min((bounds.height() - y).max(1));
    let class = utf16("EDIT");
    unsafe {
        let edit = CreateWindowExW(0, class.as_ptr(), utf16("").as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_BORDER | ES_AUTOHSCROLL,
            x, y, width, height, overlay, 0, GetModuleHandleW(null()), null());
        if edit == 0 { alert(overlay, "Unable to open the text editor"); return; }
        APP.with(|cell| {
            let mut state = cell.borrow_mut();
            state.text_edit = edit;
            state.text_anchor = position;
            state.ime_composing = false;
        });
        SetFocus(edit);
    }
}

fn commit_text_edit() {
    let (edit, position, overlay) = APP.with(|cell| {
        let mut state = cell.borrow_mut();
        let edit = std::mem::replace(&mut state.text_edit, 0);
        state.ime_composing = false;
        (edit, state.text_anchor, state.overlay)
    });
    if edit == 0 { return; }
    let words = unsafe {
        // Limit input to 512 UTF-16 units, preventing pathological label sizes.
        let count = GetWindowTextLengthW(edit).clamp(0, 512) as usize;
        let mut buffer = vec![0u16; count + 1];
        let read = GetWindowTextW(edit, buffer.as_mut_ptr(), buffer.len() as i32);
        DestroyWindow(edit);
        String::from_utf16_lossy(&buffer[..read.max(0) as usize])
    };
    if !words.trim().is_empty() {
        APP.with(|cell| {
            let mut state = cell.borrow_mut();
            let annotation = Annotation::text_at(position, words).with_stroke(state.stroke);
            state.history.push(annotation);
        });
    }
    if overlay != 0 {
        unsafe { SetFocus(overlay); InvalidateRect(overlay, null(), 0); }
    }
}

fn cancel_text_edit() {
    let (edit, overlay) = APP.with(|cell| {
        let mut state = cell.borrow_mut();
        {
            state.ime_composing = false;
            (std::mem::replace(&mut state.text_edit, 0), state.overlay)
        }
    });
    if edit != 0 { unsafe { DestroyWindow(edit); } }
    if overlay != 0 {
        unsafe { SetFocus(overlay); InvalidateRect(overlay, null(), 0); }
    }
}

// Called before TranslateMessage, because standard EDIT child windows own
// WM_KEYDOWN and would otherwise consume Enter/Escape before overlay_proc.
fn handle_text_edit_key(message: &Msg) -> bool {
    let edit = APP.with(|cell| cell.borrow().text_edit);
    if edit == 0 || message.hwnd != edit { return false; }
    // Let the IME consume Enter/Escape while the user is selecting a CJK
    // candidate. The composition messages must continue to DispatchMessageW.
    match message.message {
        WM_IME_STARTCOMPOSITION => {
            APP.with(|cell| cell.borrow_mut().ime_composing = true);
            false
        }
        WM_IME_ENDCOMPOSITION => {
            APP.with(|cell| cell.borrow_mut().ime_composing = false);
            false
        }
        WM_KEYDOWN => {
            if APP.with(|cell| cell.borrow().ime_composing) { return false; }
            match message.wparam {
                VK_RETURN => { commit_text_edit(); true }
                VK_ESCAPE => { cancel_text_edit(); true }
                _ => false,
            }
        }
        _ => false,
    }
}

fn mouse_down(hwnd: Hwnd, l: Lparam) {
    let action = APP.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(frame) = &state.frame else { return 0usize; };
        let point = local_pos(frame, l);
        let bounds = frame.bounds;
        if let Some(selection) = state.selected {
            let bar = toolbar_for(selection, bounds);
            if state.tool != Tool::Select {
                if let Some(slot) = toolbar::palette_hit(bar, bounds, point) {
                    return 30 + slot;
                }
            }
            if bar.contains(point) {
                // The gaps between WeChat-style button groups are inert.
                return toolbar::action_at(bar, point).unwrap_or(21);
            }
        }

        state.drag_anchor = point;
        state.resizing = None;
        state.moving = false;
        if let Some(selection) = state.selected {
            if let Some(handle) = ResizeHandle::hit_test(selection, point) {
                state.resizing = Some(handle);
                state.drag_initial = selection;
                // Resize only changes the crop boundary. Previous annotations
                // remain in desktop coordinates and are clipped to the new crop.
                state.tool = Tool::Select;
            } else if selection.contains(point) {
                if state.tool == Tool::Select {
                    state.moving = true;
                    state.drag_initial = selection;
                    // The annotations move with the crop on mousemove, instead
                    // of being discarded when the user repositions a capture.
                } else if state.tool == Tool::Text {
                    return 20;
                } else {
                    state.active_annotation = Some(Annotation::begin(state.tool, point).with_stroke(state.stroke));
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
        1..=7 => {
            commit_text_edit();
            APP.with(|cell| {
                cell.borrow_mut().tool = match action {
                    1 => Tool::Select, 2 => Tool::Rectangle, 3 => Tool::Ellipse,
                    4 => Tool::Arrow, 5 => Tool::Pen, 6 => Tool::Text,
                    _ => Tool::Mosaic,
                };
            });
            unsafe { InvalidateRect(hwnd, null(), 0); }
        }
        8 => {
            commit_text_edit();
            APP.with(|cell| { cell.borrow_mut().history.undo(); });
            unsafe { InvalidateRect(hwnd, null(), 0); }
        }
        9 => {
            commit_text_edit();
            APP.with(|cell| { cell.borrow_mut().history.redo(); });
            unsafe { InvalidateRect(hwnd, null(), 0); }
        }
        10 => finish_capture(),
        11 => close_overlay(),
        20 => start_text_edit(hwnd),
        21 => {}, // disabled toolbar items / group separators are inert
        30..=37 => {
            APP.with(|cell| {
                let mut state = cell.borrow_mut();
                if action <= 34 {
                    state.color_index = action - 30;
                    state.stroke = Stroke::preset(state.color_index, state.stroke.width);
                } else {
                    let widths=[2u8,4,7];
                    state.stroke = Stroke::preset(state.color_index, widths[action-35]);
                }
            });
            unsafe { InvalidateRect(hwnd, null(), 0); }
        },
        _ => { unsafe { SetCapture(hwnd); } }
    }
}

fn mouse_move(hwnd: Hwnd, l: Lparam) {
    let changed = APP.with(|cell| {
        let mut s = cell.borrow_mut();
        let Some(frame) = &s.frame else { return false; };
        let point = local_pos(frame, l);
        let bounds = frame.bounds;
        let before = (s.selected, s.hover, s.toolbar_hover,
            s.active_annotation.as_ref().and_then(|a| a.points.last().copied()));
        if s.dragging {
            if let Some(annotation) = &mut s.active_annotation {
                annotation.update(point);
            } else if let Some(handle) = s.resizing {
                s.selected = Some(handle.resize(s.drag_initial, point, bounds));
            } else if s.moving {
                let moved = s.drag_initial.translated(
                    point.x.saturating_sub(s.drag_anchor.x),
                    point.y.saturating_sub(s.drag_anchor.y)
                ).clamp_inside(bounds);
                if let Some(previous) = s.selected {
                    // Delta must use the CLAMPED crop, otherwise annotations
                    // drift at monitor boundaries and after repeated drags.
                    s.history.translate(
                        moved.left.saturating_sub(previous.left),
                        moved.top.saturating_sub(previous.top));
                }
                s.selected = Some(moved);
            } else {
                s.selected = Rect::from_points(s.drag_anchor, point).intersection(bounds);
            }
        } else if s.selected.is_none() {
            s.hover = hit_test(frame, point);
        }
        s.toolbar_hover = if let Some(selection) = s.selected {
            if !s.dragging {
                toolbar::slot_at(toolbar_for(selection, bounds), point)
            } else { None }
        } else { None };
        let after = (s.selected, s.hover, s.toolbar_hover,
            s.active_annotation.as_ref().and_then(|a| a.points.last().copied()));
        before != after
    });
    // Moving inside the same hovered window is not a visual change.
    // Skip full-desktop redraws when neither selection nor annotation changed.
    if changed { unsafe { InvalidateRect(hwnd, null(), 0); } }
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
        WM_ERASEBKGND => 1, // compositor owns every pixel; never erase to white/black
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
        WM_KEYDOWN if w == VK_V => {
            APP.with(|cell| cell.borrow_mut().tool = Tool::Select);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_KEYDOWN if w == VK_ESCAPE => { close_overlay(); 0 }
        WM_RBUTTONUP | WM_CLOSE | WM_DISPLAYCHANGE | WM_DPICHANGED => { close_overlay(); 0 }
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

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_NAME: &str = "RustShot";

// HKCU Run entry avoids elevation and leaves other users untouched.
fn startup_command() -> Result<Vec<u16>, String> {
    let executable = std::env::current_exe()
        .map_err(|e| format!("Cannot locate RustShot executable: {e}"))?;
    let mut command = Vec::from([b'"' as u16]);
    command.extend(executable.as_os_str().encode_wide());
    command.push(b'"' as u16);
    command.push(0);
    Ok(command)
}

fn open_run_key() -> Result<Handle, String> {
    let mut key: Handle = 0;
    let name = utf16(RUN_KEY);
    let code = unsafe {
        RegCreateKeyExW(HKEY_CURRENT_USER, name.as_ptr(), 0, null_mut(), 0,
            KEY_QUERY_VALUE | KEY_SET_VALUE, null(), &mut key, null_mut())
    };
    if code != 0 { Err(format!("Cannot access HKCU Run registry: {code}")) }
    else { Ok(key) }
}

fn startup_enabled() -> Result<bool, String> {
    let expected = startup_command()?;
    let key = open_run_key()?;
    let value = utf16(RUN_NAME);
    let mut bytes = 0u32;
    let mut value_type = 0u32;
    let status = unsafe {
        RegQueryValueExW(key, value.as_ptr(), null_mut(),
            &mut value_type, null_mut(), &mut bytes)
    };
    if status != 0 || value_type != REG_SZ || bytes == 0 || bytes > 32768 {
        unsafe { RegCloseKey(key); }
        return Ok(false);
    }
    let mut data = vec![0u8; bytes as usize];
    let code = unsafe {
        RegQueryValueExW(key, value.as_ptr(), null_mut(), &mut value_type,
            data.as_mut_ptr(), &mut bytes)
    };
    unsafe { RegCloseKey(key); }
    if code != 0 || value_type != REG_SZ || bytes as usize > data.len() ||
        !bytes.is_multiple_of(2) { return Ok(false); }
    let stored: Vec<u16> = data[..bytes as usize].as_chunks::<2>().0.iter()
        .map(|v| u16::from_le_bytes(*v)).collect();
    Ok(stored == expected)
}

fn set_startup(enabled: bool) -> Result<(), String> {
    let key = open_run_key()?;
    let name = utf16(RUN_NAME);
    let code = if enabled {
        let command = startup_command()?;
        let size = command.len().checked_mul(2).ok_or("Registry value overflow")?;
        unsafe { RegSetValueExW(key, name.as_ptr(), 0, REG_SZ,
            command.as_ptr() as *const u8, size as u32) }
    } else {
        let status = unsafe { RegDeleteValueW(key, name.as_ptr()) };
        // ERROR_FILE_NOT_FOUND: already disabled
        if status == 2 { 0 } else { status }
    };
    unsafe { RegCloseKey(key); }
    if code == 0 { Ok(()) }
    else { Err(format!("Unable to change Windows startup registry value: {code}")) }
}

const SETTINGS_KEY: &str = "Software\\RustShot";
fn open_settings_key() -> Result<Handle, String> {
    let mut handle=0;
    let key=utf16(SETTINGS_KEY);
    let result=unsafe {RegCreateKeyExW(HKEY_CURRENT_USER,key.as_ptr(),0,null_mut(),
        0,KEY_QUERY_VALUE|KEY_SET_VALUE,null(),&mut handle,null_mut())};
    if result==0 {Ok(handle)} else {Err(format!("Settings registry access: {result}"))}
}
fn read_setting(name: &str, fallback: bool) -> bool {
    let Ok(key)=open_settings_key() else {return fallback};
    let value=utf16(name);
    let mut raw=[0u8;4];
    let mut len=4u32;
    let mut kind=0u32;
    let result=unsafe {RegQueryValueExW(key,value.as_ptr(),null_mut(),&mut kind,
        raw.as_mut_ptr(),&mut len)};
    unsafe {RegCloseKey(key);}
    if result==0 && kind==REG_DWORD && len==4 {
        u32::from_le_bytes(raw)!=0
    } else {fallback}
}
fn set_setting(name: &str, enabled: bool) -> Result<(),String> {
    let key=open_settings_key()?;
    let value=utf16(name);
    let raw=(enabled as u32).to_le_bytes();
    let result=unsafe {RegSetValueExW(key,value.as_ptr(),0,REG_DWORD,
        raw.as_ptr(),raw.len() as u32)};
    unsafe {RegCloseKey(key);}
    if result==0 {Ok(())} else {Err(format!("Cannot update {name}: {result}"))}
}

fn show_tray_menu(hwnd: Hwnd) {
    // SAFETY: Created menu is destroyed once; TrackPopupMenu returns the selected ID.
    unsafe {
        let menu = CreatePopupMenu();
        if menu == 0 { return; }
        let take = utf16("截图 / Capture");
        let folder = utf16("打开截图目录 / Open folder");
        let startup = startup_enabled().unwrap_or(false);
        let toggle = utf16(if startup { "开机启动：已开启 / Disable" }
                           else { "开机启动：已关闭 / Enable" });
        let (auto_save,auto_copy)=APP.with(|cell| {
            let state=cell.borrow();
            (state.auto_save,state.auto_copy)
        });
        let save_text=utf16("截图自动保存 PNG / Auto save");
        let copy_text=utf16("截图自动复制 / Auto copy");
        let quit = utf16("退出 / Exit");
        AppendMenuW(menu, MF_STRING, 1, take.as_ptr());
        AppendMenuW(menu, MF_STRING, 2, folder.as_ptr());
        AppendMenuW(menu, MF_STRING | if startup { MF_CHECKED } else { 0 },
            3, toggle.as_ptr());
        AppendMenuW(menu,MF_STRING|if auto_save{MF_CHECKED}else{0},5,save_text.as_ptr());
        AppendMenuW(menu,MF_STRING|if auto_copy{MF_CHECKED}else{0},6,copy_text.as_ptr());
        AppendMenuW(menu, MF_STRING, 4, quit.as_ptr());
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
            3 => {
                if let Err(err) = set_startup(!startup) {
                    alert(hwnd, &format!("Startup setting failed: {err}"));
                }
            },
            5 | 6 => {
                let (name,new_value)=if selection==5 {
                    ("AutoSave",!auto_save)
                } else {
                    ("AutoCopy",!auto_copy)
                };
                match set_setting(name,new_value) {
                    Ok(()) => APP.with(|cell| {
                        let mut s=cell.borrow_mut();
                        if selection==5 {s.auto_save=new_value;} else {s.auto_copy=new_value;}
                    }),
                    Err(err)=>alert(hwnd,&err),
                }
            },
            4 => { DestroyWindow(hwnd); },
            _ => {}
        }
    }
}

unsafe extern "system" fn main_proc(hwnd: Hwnd, message: u32, _w: Wparam, l: Lparam) -> Lresult {
    match message {
        WM_HOTKEY => { start_capture(); 0 }
        WM_APP_TRAY if l as u32 == WM_RBUTTONUP || l as u32 == WM_CONTEXTMENU => { show_tray_menu(hwnd); 0 }
        WM_APP_TRAY if l as u32 == WM_LBUTTONUP => { start_capture(); 0 }
        WM_DISPLAYCHANGE | WM_DPICHANGED => { close_overlay(); 0 }
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
        APP.with(|cell| {
            let mut s=cell.borrow_mut();
            s.main=hwnd;
            s.auto_save=read_setting("AutoSave",true);
            s.auto_copy=read_setting("AutoCopy",true);
        });
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
            if handle_text_edit_key(&msg) { continue; }
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
