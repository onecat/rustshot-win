# RustShot

Lightweight **native Rust screenshot utility for Windows 11 x64**, with a workflow inspired by desktop WeChat screenshot selection. No Electron, WebView, cloud upload, telemetry, or administrator access is required.

> **Current status: v0.6.0 developer preview** — Windows x64 CI compiling, linting, and unit tests are passing; **Windows 11 interactive QA is still required**. This is not yet a complete WeChat screenshot replacement.

## Implemented through v0.6.0

- Single-instance notification area app, `Alt+A` global shortcut (falls back to `Ctrl+Shift+A` if occupied).
- Take a frozen desktop snapshot **before** displaying the darkened overlay; v0.4 uses a single full-desktop BGRA buffer plus an `AlphaBlend` 1×1 black source instead of keeping a second full-sized darkened BGRA image.
- Hover-detect top-level visible windows, click to select one, or drag to create an arbitrary selection.
- Drag the selected region to move; **8 handles** let you resize every edge and corner.
- **v0.5 icon-based native floating toolbar** visually inspired by desktop WeChat 4.x: white rounded background, thin gray outline and shadow, monochrome vector icons, section separators, subtle green active state, hover background and Chinese hover tips, red Cancel and green Finish. Buttons are aligned at the bottom-right of the selection with automatic above-selection fallback.
- 11 real toolbar actions: Rectangle / Ellipse / Arrow / Pen / Mosaic / Text / Select / Undo / Redo / Cancel / Finish. Tooltips are informational only; unsupported actions are intentionally not displayed.
- Draw annotation overlays with non-destructive history. `Ctrl+Z` and `Ctrl+Y` undo/redo.
- Text uses a native Unicode input control with Windows IME support: click within the selected area, type, press **Enter** to commit or **Esc** to cancel. Labels are drawn onto both PNG and clipboard output using GDI.
- Pixelated mosaic: drag a rectangle over sensitive details. 12×12 physical-pixel averaging; rectangles clip strictly inside the selected region.
- Per-user **Start with Windows** on/off option in the tray right-click menu, stored in `HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run` with no administrator permission required; **disabled by default**.
- On Finish or Enter, rasterize completed annotations into the cropped BGRA screenshot, copy `CF_DIB` to the Windows clipboard, and auto-save a timestamped PNG to **Pictures/RustShot**.
- Right-click or Esc cancels. Tray menu: capture, open output folder, exit.
- Full virtual-desktop signed coordinates, process Per-Monitor-V2 DPI awareness, clipping of selection/annotations, limited temporary files and atomic save.
- v0.4 uses 32-bit virtual-screen cursor coordinates from `GetCursorPos` instead of relying solely on truncated Win32 16-bit mouse `LPARAM` coordinates. The selection overlay closes on DPI/display changes and attempts to restore the previously active window after capture/cancel.
- PNG saves use a bounded-memory, dependency-free streamed zlib-stored-block writer with chunk CRC and full multi-block round-trip unit tests. It does **not** compress the image to a smaller file. This removes large temporary RGB and zlib buffers.
- Concurrent full-resolution PNG writers are capped at **two**. If screenshots are taken faster than they can be saved, the next save waits for the oldest rather than accumulating unbounded pixel buffers.
- Dependency-free, native Win32/GDI implementation, no third-party Rust crates.

### Keyboard / mouse quick start

1. Extract the **single** `RustShot.exe` from the workflow's `RustShot-v0.6.0-win-x64-portable` ZIP (or build it yourself) and start it.
2. Press `Alt+A` (or fallback `Ctrl+Shift+A`).
3. Drag a screenshot rectangle or click a highlighted window.
4. Drag the green handles to resize; drag inside the selected area to move.
5. Use the responsive **icon toolbar** beside the selection (V returns to selection mode). Hover for Chinese tooltips; left group contains drawing tools, middle contains undo/redo, last two buttons are red Cancel and green Finish. For text, click once inside the selected area and enter text. `Ctrl+Z`/`Ctrl+Y` for history.
6. Press Enter or select **完成**. PNG is saved automatically and clipboard is populated independently. Esc/right-click cancels.

**Note:** Moving or resizing the selection currently clears annotation history to prevent annotations being silently repositioned outside their original screen coordinates. Save your desired crop before drawing.

## Building and testing

Prerequisites: Windows 11 x64, Rust stable MSVC target, Windows SDK and MSVC C++ build tools.

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --target x86_64-pc-windows-msvc
.\target\x86_64-pc-windows-msvc\release\rustshot.exe
```

GitHub Actions builds with a `windows-2022` runner. A green CI run proves build, lint, and unit-test status; GitHub hosted runners cannot verify that global hotkeys, window enumeration, tray state, or screenshot rendering work on your Windows 11 desktop.




### v0.6.0 user-reference UI reconstruction

Rebuilt the visible overlay from **three screenshots provided by the user**: a 941×757 full-toolbar reference, an enlarged toolbar closeup and a 638×398 compact-toolbar reference. This version is intended to look much closer to WeChat for Windows, but it has **not yet been pixel-diff validated on the user's Windows 11 installation**.

- Two size profiles rather than one generic bar: **824×52 pixels** when the virtual desktop is at least 900 physical pixels wide; **556×41 pixels** on narrower screens. Both clamp against virtual-desktop boundaries and flip above the selection if there is insufficient room beneath.
- Reference-aligned fixed icon centers, gaps and separators; rectangular and elliptical outline icons, emoji, arrow, pen, mosaic, framed T, translation, OCR scan, scroll capture, undo, save, pin, share, red cancel X and green confirm.
- The **16-item expanded** sequence matches the main reference: rectangle, ellipse, emoji, arrow, pen, mosaic, text, translation, OCR, scroll, undo, save, pin, share, cancel, finish. The **12-item compact** sequence omits secondary unavailable tools.
- Green crop outline and **eight solid green squares**, with an upper-left `width x height` label, inspired by the supplied examples.
- Disabled placeholders: emoji, translation, OCR, scrolling capture, pin and share. They are visibly gray, show a **暂未实现** tooltip and **do not capture the mouse** or alter selection state. The save icon is a real shortcut for the existing **save-and-complete** flow, not a separate export mode.
- There is no visible Select icon in the WeChat reference. RustShot retains the selection/move tool by default and supports **V** to switch back to it after choosing a drawing tool. Ctrl+Y still redoes annotations even though the reference toolbar does not show a redo button.
- The **v0.4.1 double-buffered, no-flicker overlay** and PNG/clipboard export code remain in place.

This is an independently drawn interface; no copyrighted WeChat code or assets are packaged. It aims to reproduce the layout without promising exact typography or OS compositor/shadow pixels.

### v0.5.0 toolbar update and fidelity boundary

The former plain Chinese-text button row has been replaced with a compact **428×44 physical-pixel** custom-drawn toolbar on standard-sized screens; it becomes narrower on constrained desktops. Geometry, hover hit testing and click actions now share a single tested layout module (`src/toolbar.rs`). There is **no Electron/WebView** and no brand assets copied from WeChat.

Visual references are taken from published desktop WeChat 4.x screenshots, **not** from a pixel-perfect image of the user's installed version. Spacing, exact vector strokes, user-configurable line/color palette, emoji, scrolling capture and newer WeChat-only actions need further design and implementation. The non-flickering v0.4.1 double-buffered screenshot overlay is retained unchanged.

### v0.4.1 flicker hotfix

- Fix black/white flashing when dragging or moving a screenshot selection: all GDI layers are drawn in a reusable off-screen compatible bitmap, then presented with one `BitBlt` on each `WM_PAINT` instead of painting separate intermediate layers directly to the screen.
- Handle `WM_ERASEBKGND` without erasing and add `WS_CLIPCHILDREN` to avoid interfering with the native text-entry control.
- Only invalidate the overlay when a mouse move changes the selection, hovered window, or active annotation; no redraw for identical visual state.
- The backbuffer is allocated once per overlay and explicitly freed on capture or cancel. At 3840×2160×32bpp it uses roughly 31.6 MiB extra GDI bitmap memory **while screenshot mode is active**; it is **not** resident while idle in the tray.
- Windows 11 real-device QA is required: CI cannot simulate display composition, flicker, remote desktop, mixed DPI, or Intel/NVIDIA/AMD driver variants.

## Not finished / known limitations

- The runtime capture backend remains **GDI**. WGC/DXGI API capabilities and tradeoffs have been evaluated in [ADR-002](docs/ADR-002-capture-backends-v04.md), but neither is integrated as a replacement yet. HDR, protected-window behavior, and physical multi-display testing remain open.
- **Color/line-width selection, screenshot pinning, OCR, scrolling screenshots, and custom hotkey settings** are not implemented yet.
- This first text field accepts a single line (up to 512 UTF-16 code units). Text color and size are fixed; multi-line editing and resize handles are planned.
- Screenshot text preview/export uses GDI rather than DirectWrite. Manually validate font metrics and CJK rendering on Windows 11.
- Overlapping mosaic annotations use a crop-based preview, which may differ slightly from export when pixelation overlaps earlier shapes. Review in real-device QA.
- Startup uses the current executable path. Moving the portable EXE after enabling autostart requires re-enabling it from the new location.
- Clipboard writes and PNG writes are independent, but clipboard conflicts can still occur with third-party applications.
- A GDI screenshot may not capture some protected content, HDR surfaces, or games.
- No installer, in-app configuration, signature, or automated updater yet.
- The new PNG exporter streams valid *stored* PNG/zlib blocks using bounded buffers; output files remain larger than typical compressed PNG images. This is a peak-memory optimization, **not a file-size optimization**.
- The new `AlphaBlend`-based black overlay requires Windows 11 driver testing; unsupported drivers may show a poorly dimmed image.
- Windows 11 manual smoke testing and memory benchmarking remain required before labeling a release as stable.

## Architecture

`src/windows.rs` — Win32 hotkey/tray, GDI capture, overlay painting, window detection and lifecycle.  
`src/geometry.rs` — signed half-open rectangle geometry.  
`src/selection.rs` — eight resize handles and bounded resizing.  
`src/annotations.rs` — annotation tools, history, mosaic and independent BGRA rasterization.  
`src/png.rs` — minimal streamed-format PNG encoder.

Project requirements and test checklist: [development plan](docs/development-plan.md), [UI specification](docs/UI-spec.md), [capture backend ADR](docs/ADR-001-capture-backend.md), [v0.4 backend decision](docs/ADR-002-capture-backends-v04.md), [v0.4 Windows acceptance tests](docs/v0.4-qa-matrix.md).

## License

MIT. RustShot is not affiliated with Tencent or WeChat; no WeChat assets, code, or private interfaces are used.
