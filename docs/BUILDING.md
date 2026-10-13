# Building RustShot on Windows 11

**GitHub repository:** https://github.com/onecat/rustshot-win

## Obtain a CI-built test ZIP

1. Open https://github.com/onecat/rustshot-win/actions/workflows/windows-ci.yml
2. Select the latest **green** Windows workflow run.
3. Under **Artifacts**, download `RustShot-v0.1.0-win-x64-portable`.
4. Extract the Actions ZIP, then extract the contained `RustShot-v0.1.0-win-x64-portable.zip`.
5. Run `RustShot.exe` on a Windows 11 x64 desktop. This binary is unsigned; review its provenance before allowing it to run.

CI baseline: https://github.com/onecat/rustshot-win/actions/runs/38368803609
Rust 1.99.0 / Windows Server 2022: Clippy, unit tests, Windows MSVC release build and ZIP artifact passed. **No Windows 11 user-interface testing has yet been performed.**

## Build from the repository

```powershell
git clone https://github.com/onecat/rustshot-win.git
cd rustshot-win
cargo build --release --target x86_64-pc-windows-msvc
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

Requires a stable Rust MSVC toolchain, Visual Studio C++ Build Tools, and Windows SDK. The current PoC uses only the Windows system libraries and no Cargo dependencies.

For screenshot testing, use Alt+A (or Ctrl+Shift+A when occupied), hover to select a window, drag to select an area, Enter to confirm, Esc to cancel. Files are saved under the Pictures known folder, subdirectory RustShot.

## Release gate

An Actions green check does **not** imply feature parity, security certification, measured resource usage or Windows 11 compatibility. The initial GDI screenshot backend remains provisional. Test multiple monitors, DPI scaling, protected windows, RDP, clipboard contention, screenshot rendering and repeated hotkey cycles on real Windows machines before declaring a stable release.
