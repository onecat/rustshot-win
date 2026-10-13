# UI baseline — provisional until user's WeChat reference is captured

1. **Idle**: one tray icon, no windows, hotkey Alt+A. Fall back to Ctrl+Shift+A when Alt+A is owned by another process.
2. **Capture**: capture pixels before any overlay; draw a single full-virtual-desktop window showing frozen original pixels dimmed to ~57% brightness (temporary approximation).
3. **Hover**: when not selecting, highlight the first visible top-level Z-order window beneath cursor using DWM extended frame bounds. This is an approximation and requires mixed-DPI tests.
4. **Selection**: click a highlighted window, or drag any rectangle in any direction. Border is green. Width × Height label follows selection; toolbar sits below or above as space permits.
5. **Move**: drag inside a selection to move. Resizing by 8 handles is **not implemented** in the v0.1 baseline.
6. **Confirm**: Enter or left-side toolbar action; exports cropped frame to CF_DIB and PNG. No image annotation yet.
7. **Cancel**: Esc/right-click/right toolbar action; no export.

## Reference capture plan

When a real Windows 11 WeChat version is available, capture 15 baseline interactions (cold open, hover, click, all 4 diagonal drags, resize, move, toolbar top/bottom, text IME, mosaic, finish/cancel, dual-monitor negative coordinates, shortcut conflict) and update this document. Do not upload real user content or copyrighted UI assets to public GitHub.
