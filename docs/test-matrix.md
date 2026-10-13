# Prototype acceptance ledger

| Check | Status | Notes |
| --- | --- | --- |
| Windows CI cargo fmt/clippy/test/build | NOT RUN | No Windows-hosted CI until repository exists on GitHub |
| Local Linux cargo test | BLOCKED | Rust compiler/toolchain unavailable in this environment |
| 1080p/4K/dual monitor live screenshot | NOT RUN | Requires real Windows 11 |
| Screenshot before overlay (no self-capture) | CODE REVIEW PENDING | Call order implemented, needs visual assertion |
| Alt+A conflict fallback | CODE REVIEW PENDING | RegisterHotKey fallback Ctrl+Shift+A |
| Clipboard CF_DIB pastes into chat/Paint | NOT RUN | Requires Windows UI |
| PNG opens without warnings | NOT RUN | Check saved file with Windows Photos and another decoder |
| Esc / right click does not save | CODE REVIEW PENDING | No IO from cancel branch |
| Protected-content/security behavior | NOT RUN | Never attempt bypass |
| 1000 screenshots/10-hour idle | NOT RUN | Test after capture backend selected |

## Manual smoke test (Windows 11)

- Verify tray icon appears and no top-level window shows during idle.
- With WeChat open and Alt+A occupied, verify fallback Ctrl+Shift+A.
- Capture part of desktop and paste directly into Paint; compare pixel alignment.
- Verify one PNG is created in *Pictures\\RustShot*, even when clipboard unavailable.
- Cancel a second capture and verify there is no new image.
- Drag from right/bottom to top/left; test a secondary monitor placed left of primary.
- Check output doesn't contain overlay toolbar/selection border.
- Kill / restart app and verify no stale topmost window remains.
