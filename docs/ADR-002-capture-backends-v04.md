# ADR-002 — v0.4 desktop capture backend decision

Status: **Research complete; GDI retained for v0.4; native WGC/DXGI integration not implemented.**

## Goal

Use the Windows 11 capture backend that provides reliable instant one-shot screenshot initiation, consistent virtual-desktop pixels, low idle cost, no unexplained OS capture border, and correct behavior across mixed-DPI monitors.

## Existing GDI implementation

- Single Win32 process, hotkey, full virtual-desktop screen capture via \`BitBlt\` into a top-down 32-bit DIB, then a copied owned BGRA pixel buffer.
- Desktop snapshot happens **before** the topmost selection overlay appears, so UI controls are never captured into the image.
- The v0.4 overlay darkens the captured frame using a 1×1 AlphaBlend source instead of storing a second full-screen BGRA image.
- Known tradeoffs: GDI may not reflect protected GPU content, some exclusive/fullscreen apps, HDR tone mapping, or unusual driver pipelines.

## Windows Graphics Capture (WGC)

Microsoft references:
- https://learn.microsoft.com/en-us/windows/uwp/audio-video-camera/screen-capture
- https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscapturesession.isborderrequired

Advantages:
- Modern capture stack; Windows 11 WinRT support and D3D11 textures.
- Per-monitor / per-window capture with system-managed resource scheduling.

Constraints:
- Screen/window snapshots require WinRT/D3D11 interop and GPU readback to CPU BGRA for the existing editor.
- Colored OS border control is subject to OS permissions. The docs require user consent, including a packaged-app capability for borderless capture. A plain un-packaged portable EXE cannot assume it may silently disable the system capture border.
- Capturing a whole desktop with multiple monitors requires coordinated per-monitor data flow and compositing into one signed virtual coordinate space.
- Protected content remains protected.

## DXGI Desktop Duplication

Microsoft references:
- https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/desktop-dup-api
- https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgioutput1-duplicateoutput

Advantages:
- GPU-based desktop frames without requiring WGC's capture border UI.
- Suitable for full-desktop monitoring and zero-copy downstream GPU paths.

Constraints:
- One duplication per active output is necessary for an entire virtual desktop; output frames are not time-synchronized automatically.
- Must create the D3D11 device on the appropriate adapter; multi-GPU setups need per-adapter handling.
- Duplication resources may be unavailable/unsupported due to desktop state, modes, concurrent duplication limits or session switches.
- Must copy/map staging textures and restore alpha/format/rotation before writing CPU BGRA.
- Additional thread/device/COM lifetime risks without a measured benefit for a hotkey-only utility.

## v0.4 decision

Keep GDI as the **only** runtime capture backend until Windows 11 manual baseline testing is available. Avoid silently selecting an untested GPU backend for users. Optimize data copies, encoding memory and pointer coordinates first.

The next GPU prototype should be isolated behind a \`CaptureBackend\` interface, with explicit fallback to GDI after failure and a display-by-display evaluation. No WGC/DXGI success is claimed by the v0.4 CI or portable build.

## Prototype acceptance gates (future release)

1. Detect monitors and GPU adapters; record virtual desktop bounds, scaling and rotation.
2. Capture simultaneously at 100%/150%/200% DPI, monitor left/right of primary, HDR/non-HDR and laptop external display.
3. Compare output checksums/coordinate mapping to GDI and visual reference, excluding protected surfaces.
4. Measure P50/P95 hotkey-to-overlay latency, peak private bytes, GDI handle count and CPU with 100 back-to-back captures.
5. Simulate session lock, display change, disconnection, RDP and device lost, with resources released and GDI fallback.
6. Never attempt to evade Windows permission prompts, capture borders or protected content.

## Revisit rule

Switch the default backend only after a tested implementation measurably beats the existing GDI path with no new UX/privacy regressions. Otherwise GDI remains the stable baseline.