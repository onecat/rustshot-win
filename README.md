# RustShot

Lightweight **native Rust screenshot utility for Windows 11 x64**, with a workflow inspired by desktop WeChat screenshot selection. No Electron, WebView, cloud upload, telemetry, or administrator access is required.

> **Current status: v0.4.0 developer preview** — Windows x64 CI compiling, linting, and unit tests are passing; **Windows 11 interactive QA is still required**. This is not yet a complete WeChat screenshot replacement.

## Implemented in v0.4.0

- Single-instance notification area app, `Alt+A` global shortcut (falls back to `Ctrl+Shift+A` if occupied).
- Take a frozen desktop snapshot **before** displaying the darkened overlay; v0.4 uses a single full-desktop BGRA buffer plus an `AlphaBlend` 1×1 black source instead of keeping a second full-sized darkened BGRA image.
- Hover-detect top-level visible windows, click to select one, or drag to create an arbitrary selection.
- Drag the selected region to move; **8 handles** let you resize every edge and corner.
- Floating Chinese-language toolbar: **Selection / Rectangle / Ellipse / Arrow / Freehand pen / Text / Mosaic / Undo / Redo / Finish / Cancel**.
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

1. Extract the **single** `RustShot.exe` from the workflow's `RustShot-v0.4.0-win-x64-portable` ZIP (or build it yourself) and start it.
2. Press `Alt+A` (or fallback `Ctrl+Shift+A`).
3. Drag a screenshot rectangle or click a highlighted window.
4. Drag the green handles to resize; drag inside the selected area to move.
5. Use the floating toolbar to select a drawing tool, then drag inside the selection to annotate. For text, click once, type in the native text input, press Enter to add the label. `Ctrl+Z`/`Ctrl+Y` for history.
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
