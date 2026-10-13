# ADR-001: Capture backend (OPEN — PoC in progress)

- **First working baseline**: GDI `BitBlt` with `CAPTUREBLT` captures a virtual-desktop BGRA frame **before** the overlay appears. This allows testing the interaction loop with no external libraries, but does NOT meet the performance/compatibility evaluation gate.
- **Planned evaluation**: Implement and benchmark WGC and DXGI Desktop Duplication against GDI on real Windows 11 1080p/4K/mixed-DPI devices. Prefer DXGI when WGC's permission/capture-border UX disrupts one-key activation.
- **Not yet verified**: 4K timings; WGC/DXGI frame delivery; HDR tone mapping; full-screen games; protected windows; RDP; high-DPI hot-plug.
- **Decision status**: NOT ACCEPTED; cannot claim P1 gate has passed.

This intentionally limits v0.1.0 to an honest, testable prototype. No claim of WeChat feature parity.
