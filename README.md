# RustShot — native Rust screenshot prototype for Windows 11

RustShot is a **new independent screenshot utility**, inspired by the familiar selection workflow of desktop WeChat. It does not inject into, inspect private data from, or use code/assets from WeChat.

**State: v0.1.0 PoC; not production ready and NOT feature-parity with WeChat.** This is the first executable code baseline, not the complete 5–8-week roadmap in [`docs/development-plan.md`](docs/development-plan.md).

## Build verification (2026-10-13)

- GitHub repository: https://github.com/onecat/rustshot-win
- Windows CI baseline: https://github.com/onecat/rustshot-win/actions/runs/38368803609
- Rust 1.99.0 on Windows Server 2022: Clippy, unit tests and x64 Release build **passed**.
- Actions artifact: `RustShot-v0.1.0-win-x64-portable` (outer Actions ZIP contains the portable ZIP).
- Executable is unsigned. Windows 11 interactive behavior, resource usage and screenshot correctness are **not yet verified**.
- The workflow temporarily runs `cargo fmt` in the CI checkout; checked-in rustfmt enforcement is tracked as remaining cleanup.

## Current functionality (source implementation; Windows runtime not yet validated)

- Single-instance system tray utility, with a hotkey `Alt+A` or fallback `Ctrl+Shift+A` if occupied.
- Capture the entire virtual desktop **before** drawing the overlay; freeze and dim the screen.
- Hover auto-highlights top-level Windows; click selects the hovered window; drag selects arbitrary physical-pixel region.
- Move an existing selection by dragging inside it. Selection constrained within the virtual desktop.
- Enter or toolbar accept: copy CF_DIB image to the Windows clipboard and auto-save to `Pictures\\RustShot` PNG.
- Esc or right click to cancel without export; tray menu to capture, open the output folder or exit.
- Per-Monitor-V2 process DPI awareness, signed coordinates and Win32 GDI capture baseline.
- No external Rust crate dependencies or telemetry; no administrator privilege requirement.

### What is **NOT** implemented yet

- WGC / DXGI Desktop Duplication engines, backend A/B benchmark and error recovery.
- 8 drag handles / resize selection, polygon/arrow/paint/text/mosaic annotations, undo/redo.
- Independent automatic-save & clipboard toggles, JSON settings, self-start toggle, hotkey editor.
- Golden-Master WeChat UI reference measurements and visual matching.
- HDR color management, robust hot-plug recovery, clipboard DIBV5, high-volume performance tuning.
- Installer, code signing, auto-update or OCR.

This v0.1.0 PNG encoder uses **uncompressed, valid PNG** to keep the PoC dependency-free. PNG files can be larger than expected; compression will be addressed in P3.

## Build / test

Requires **Windows 11 x64** and the stable Rust MSVC toolchain with the Windows SDK / Visual Studio C++ build tools.

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release --target x86_64-pc-windows-msvc
.\\target\\x86_64-pc-windows-msvc\\release\\rustshot.exe
```

`cargo test` for platform-independent geometry and PNG routines also runs on Linux once Rust is installed. Windows UI / GDI cannot be fully verified in this Linux authoring environment. A [Windows GitHub Actions workflow](.github/workflows/windows-ci.yml) is included and uploads the portable prototype zip **after** the repository is pushed and a Windows CI run succeeds.

## How to use the PoC

1. Launch `rustshot.exe`; it remains in the notification tray (no settings window yet).
2. Press `Alt+A`; if another program owns that hotkey, use `Ctrl+Shift+A`.
3. Hover over windows to preview their bounds. Click a window or drag a custom rectangle.
4. Drag within the selection to move it; press Enter or click *完成* to copy + save.
5. Esc / right click cancels. Right click the tray icon for actions or Exit.

### Limits and safety

Capture respects Windows protected content/security restrictions; some games, HDR desktops, secure surfaces and applications may not capture correctly under GDI. The prototype is not yet tested across all multi-DPI, multi-monitor cases. Images are kept in process memory and saved locally only. Users should avoid using a PoC for sensitive screenshots until Windows QA passes.

## Development and next gates

- **P0**: measured UI golden-master scenes are awaiting reference from the target Windows WeChat version.
- **P1**: this first pass is a **GDI baseline**, not a WGC/DXGI validation or accepted ADR. Implement and measure both native APIs before architecture is finalized.
- **P2/P3**: basic interactions and output are partially prototyped; no acceptance sign-off.
- **P4–P6**: not started.

See [`docs/ADR-001-capture-backend.md`](docs/ADR-001-capture-backend.md) and [`docs/development-plan.md`](docs/development-plan.md).

## License

MIT; see LICENSE. RustShot is not affiliated with WeChat or Tencent.
