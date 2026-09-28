# ADR-001: Rust Windows architecture and no-.NET workflow

- **Status:** Accepted as the Phase 0 architecture contract; renderer and Settings toolkit remain subject to Phase 1 PoT.
- **Date:** 2026-09-28
- **Baseline:** `c1173f9ecce858931f694661457ef2b905696da4`

## Context

The port must preserve the cataloged Windows behavior while the development and verification workflow never invokes .NET or the original executable. The existing product is a Windows taskbar monitor with a transparent overlay, telemetry polling, persistent settings, and a separate Settings window.

## Decisions

1. Use a Cargo workspace with `system-monitor-core` for platform-independent models/formatters and `system-monitor-windows` for Win32 integration and executable startup.
2. Use Rust stable targeting `x86_64-pc-windows-msvc`; the Windows build requires the Microsoft C++ Build Tools linker/runtime toolchain. CI commands are Cargo/Rust commands only.
3. Own the overlay through a native Win32 top-level layered HWND. Preserve the original window behaviors from the catalog: no ordinary taskbar button, transparent pixels, topmost option, drag/lock, context menu, taskbar placement, and DPI/display transitions.
4. Keep telemetry as immutable `SystemMetrics` snapshots consumed by the UI. Provider errors stay typed internally and are translated to source-compatible display fallbacks at the presentation boundary.
5. Store Rust settings in a Rust-specific user data directory. Legacy config import is explicitly read-only; startup registration uses a Rust-specific registry value and does not modify the original app's entry.
6. Do not select the final drawing backend or Settings toolkit before Phase 1 PoT. Candidate renderers and native-control/UI-toolkit options will be compared for behavior, startup, resource use, and HWND interoperability. The ADR must be amended with measured results before Phase 2.
7. Direct runtime comparison with the original remains `RUNTIME_DIFF_NOT_RUN`; source review plus independent Windows measurements are the allowed evidence.

## Alternatives considered

- **Reimplement in .NET:** rejected because the user's development environment must not require or execute .NET.
- **Cross-platform UI framework as the whole app:** deferred; the original contract is Windows/Win32-specific and cross-platform abstraction does not remove native Windows verification needs.
- **Choose a renderer/toolkit by preference before a PoT:** rejected because transparent layered-window compatibility, font measurement, and native menu/settings interoperability need measured evidence.

## Phase 1 PoT measurement record

| Candidate currently implemented | Static result | Windows measurement |
|---|---|---|
| Software-generated BGRA + `UpdateLayeredWindow` | 192×52×4 = 39,936-byte base surface; 150% surface = 89,856 bytes. Pixel alpha is premultiplied and covered by deterministic tests. | Startup-to-visible time, working set, transparent-edge appearance, and DPI/window interoperability remain unmeasured. |
| Native Win32 Settings controls | One built-in button toggles the PoT overlay accent through a posted window message; no UI framework dependency. | Window behavior and interoperability remain unmeasured. |

These are the current PoT candidates, not a final toolkit/renderer selection. Complete the Windows checklist and add observed startup/resource values before changing this ADR status to a final selection.

## Phase 1 exit conditions

A clean Windows Rust/MSVC host must build and launch the Rust executable, demonstrate transparent overlay presentation, drag/lock and taskbar placement, open/close the context menu and Settings window, and record renderer/toolkit timing/resource observations. No .NET command, runtime, or original executable is part of the procedure.
