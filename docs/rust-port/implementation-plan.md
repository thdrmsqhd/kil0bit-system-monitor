# Rust Port Implementation Plan

- **Baseline:** [`original-feature-catalog.md`](original-feature-catalog.md), 35 requirements, source revision `c1173f9ecce858931f694661457ef2b905696da4`.
- **Detailed task plan:** [`task-breakdown.md`](task-breakdown.md), with stable task IDs, dependencies, deliverables, and completion criteria under each phase.
- **Function-level task/I/O contracts:** [`function-task-breakdown.md`](function-task-breakdown.md), mapping planned Rust function inputs/outputs to original C# functions and `SystemMetrics` semantics.
- The detailed plan includes Phase 8 for AI usage providers after original feature parity: OpenCode Go first, Codex and DeepSeek next, with Zen balance optional.
- **Goal:** build a Windows 11 Rust application that covers the original catalog while never invoking `dotnet`, building the C# project, or launching the original .NET application during development or verification.
- **Initial parity scope:** x64 Windows 11, original hardware metrics and overlay/settings behavior. Phase 8 in the detailed task plan adds AI usage after original parity: OpenCode Go first, Codex and DeepSeek next, Zen credit balance optional.
- **Delivery rule:** each phase ends with a runnable or inspectable artifact, a requirement status update, and a recorded verification result.

## Non-negotiable constraints

1. Rust source and Rust-native dependencies only. Do not add a C# project, .NET SDK/runtime setup, `dotnet` commands, .NET build scripts, or the original executable to the build/test pipeline.
2. Use the Rust MSVC target on Windows. Install Rust and the Microsoft C++ build tools needed by that target; omit the .NET workload.
3. Verify Rust execution on Windows without launching the original app. Use the source-derived catalog as the behavior contract and mark old-versus-new runtime comparison as `RUNTIME_DIFF_NOT_RUN`.
4. Keep the original install and config untouched. Store Rust settings in a separate Rust-specific AppData directory. If importing old settings, read the original JSON once, validate it, and write the converted values only to Rust's config.
5. Do not mark hardware-dependent GPU behavior `RUN_PASS` unless it has run on matching physical hardware. Record unavailable vendors/devices as `BLOCKED`.
6. Preserve original license and attribution when distributing a derivative.

## Recommended architecture

| Component | Direction | Reason |
|---|---|---|
| Windows integration | Rust `windows` crate / Win32 APIs | Direct control over layered HWNDs, taskbar ownership, z-order, messages, DPI, AppBar notifications, registry, and DPAPI without .NET. |
| Overlay window | Dedicated native Win32 top-level layered window | Mirrors the original's taskbar overlay interaction model and keeps rendering independent from Settings. |
| Overlay drawing | Small software renderer to a 32-bit premultiplied-alpha bitmap, presented with `UpdateLayeredWindow` | Keeps transparent per-pixel rendering and does not require a managed UI runtime. Lock the renderer after the proof of technology phase. |
| Settings UI | `egui`/`eframe` desktop window, or native Win32 controls if the proof-of-technology shows the framework conflicts with overlay requirements | Rust-native settings UI is much less costly than hand-building all dashboard controls. Functional behavior is the parity target; pixel-perfect WPF styling is not. Keep Settings and the overlay in separate top-level windows. |
| Configuration | `serde` + versioned JSON under a Rust-specific `%APPDATA%` folder | Simple migration, inspectable non-secret options, and explicit schema versioning. |
| Secret storage | Windows DPAPI (`CryptProtectData`/`CryptUnprotectData`) for the future OpenCode key | Keeps credentials out of JSON and tied to the Windows user. |
| Telemetry | Provider modules behind a common snapshot model | Allows independent polling, error handling, fixtures, and hardware-specific implementations. |
| Async/background work | One telemetry coordinator with bounded polling; UI receives immutable snapshots | Prevents window-message/render code from blocking on hardware or network calls. |

**Renderer/UI decision gate:** before implementing all settings, create a short proof of technology (PoT): render a transparent/resizable bitmap in a taskbar-attached HWND, move/lock it, open a separate Rust settings window, and verify DPI behavior. Select and record the final renderer/settings toolkit from this evidence. Do not let the GUI framework own the overlay HWND if that prevents reliable AppBar and layered-window behavior.

## Phases

### Phase 0 — Baseline and design freeze

**Work**

- Treat the 35 catalog IDs as the baseline; freeze the source revision.
- Record source-derived interpretations for the five known documentation/source mismatches.
- Create a Rust workspace and a decision record for overlay renderer and Settings toolkit.
- Define the telemetry snapshot, configuration schema version, and requirement-to-module mapping.
- Establish a no-.NET build/test checklist for developer machine and CI.

**Deliverables**

- Rust architecture decision record.
- Requirement mapping: each catalog ID → module → test/verification evidence.
- Empty Rust workspace builds and runs a console-free Windows GUI process.

**Exit gate**

- `cargo build --release --target x86_64-pc-windows-msvc` succeeds on Windows.
- `cargo test --all-targets` succeeds.
- No .NET project or command exists in the build path.

### Phase 1 — Win32 and renderer proof of technology

**Work**

- Register/create an HWND and implement transparent bitmap presentation.
- Prove taskbar attachment, free placement, drag/lock, and a minimal right-click menu.
- Open/close a separate Settings window and pass data to/from the overlay.
- Verify shutdown cleans up HWNDs, timers, and worker threads.
- Evaluate DPI changes and taskbar placement behavior on Windows 11.

**Covers first:** `OVR-001` surface; feasibility spikes for `OVR-002`–`OVR-005`, `OVR-009`, and `UI-001`.

**Exit gate**

- Rust executable visibly runs on Windows without .NET processes or .NET host dependencies.
- Transparent pixels, resizing, monitor DPI changes, drag, lock, and menu all work in the PoT.
- Final renderer and settings toolkit are recorded with measured memory use and known visual differences.

### Phase 2 — App shell, settings, persistence, and lifecycle

**Work**

- Implement single-instance activation, normal launch versus `--startup`, Settings navigation, quit, and clean shutdown.
- Implement versioned Rust config load/save, defaults, reset behavior, and corruption recovery.
- Add optional one-time read-only import from `%APPDATA%\kil0bit-system-monitor\config.json`; do not modify or delete it.
- Implement per-user startup registration and verify registry cleanup on disable.
- Build the basic Home, General, Monitoring, Appearance, and About routes; wire each control to the versioned config.

**Covers:** `APP-001`–`APP-005`, `UI-001`, `UI-003`.

**Exit gate**

- Two launches result in one Rust process and activate Settings in the first.
- Every persisted option survives restart; reset and cancel behavior match the catalog.
- Startup registration adds/removes only the Rust app entry.
- Import tests prove the original config stays byte-for-byte unchanged.

### Phase 3 — Telemetry core: CPU, RAM, network, and disks

**Work**

- Implement an immutable system snapshot and isolated providers.
- Add CPU total load and used physical memory.
- Add upload/download counters, eligible adapter filtering, Default aggregation, named-adapter selection, formatting, and configurable polling interval.
- Add per-disk activity, drive used-space percentage, multiple drive selection, and “none selected”.
- Add deterministic unit tests with fixture counters for formatting, aggregation, selection, and reset cases.
- Report missing counters/permissions/device states as unavailable values; keep the app running.

**Covers:** `TEL-001`, `TEL-002`, `TEL-005`–`TEL-010`, `UI-002` (network/disks).

**Exit gate**

- All unit tests pass; rate formatting and aggregation boundaries have fixture coverage.
- Live CPU/RAM/network/disk values update on Windows at each configured interval.
- No telemetry polling blocks the UI or overlay window procedure.

### Phase 4 — GPU load and temperature

**Work**

- Implement device discovery and selection.
- Add NVIDIA reading and the source-derived AMD/Windows fallback paths that can be supported without .NET.
- Implement temperature retrieval and `N/A` handling with bounded caching.
- Keep GPU providers isolated so unavailable vendor APIs do not affect other metrics.

**Covers:** `TEL-003`, `TEL-004`, GPU part of `UI-002`.

**Exit gate**

- CPU-only or unsupported-GPU systems remain stable and display fallback/unavailable state.
- Each supported vendor path has a recorded physical-machine run; untested vendors remain `BLOCKED`.
- GPU adapter changes take effect without restarting the app.

### Phase 5 — Full overlay behavior and rendering

**Work**

- Render all metric groups in Text and Compact modes with source-compatible labels and dynamic widths.
- Implement global/per-section colors, fonts, bold, capsule/plate options, scale, and spacing.
- Implement taskbar snap, free placement, saved position, lock, context menu, Task Manager launch, About shortcut, exit, Keep on Top, and Hide in Fullscreen.
- Add fade/debounce transitions, shell/task-view exceptions, monitor/DPI updates, and multi-disk columns.
- Ensure visibility toggles do not stop the app's telemetry/configuration services.

**Covers:** `OVR-001`–`OVR-009`, `VIS-001`–`VIS-008`.

**Exit gate**

- Every overlay and visual option has an ID-linked Windows verification record.
- No clipping at scale limits or after monitor changes for tested resolutions.
- Full-screen hide/reappear behavior is stable across repeated transitions.

### Phase 6 — Integration, reliability, and no-.NET acceptance

**Work**

- Execute all catalog checks against Rust using a clean Windows 11 environment.
- Run restart/persistence, single-instance, device absence, network interruption, adapter reconnection, and long-running stability checks.
- Capture screenshots and measurement notes keyed by catalog ID.
- Inspect the Rust app's process tree and native dependencies; confirm no .NET app/runtime is launched.
- Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and release build.
- Update each requirement with `RUN_PASS`, `SOURCE_REVIEWED`, `BLOCKED`, `DIFFERENT`, or `NOT_RUN`. Keep project-wide `RUNTIME_DIFF_NOT_RUN` visible.

**Exit gate**

- All feasible catalog requirements have `RUN_PASS` and `SOURCE_REVIEWED`.
- Every blocked/unrun item has a reason and next verification condition.
- Build logs, test results, screenshots, and requirement matrix are retained.
- No `.NET` command, original app, or .NET process was used in this phase.

### Phase 7 — Packaging and personal release

**Work**

- Publish a Windows x64 portable release first; test it in a clean Windows profile.
- Include original MIT license and attribution; document settings location, hardware limitations, and no-.NET runtime requirement.
- Add installer only if needed, choosing tools that do not add a .NET build/runtime dependency to the workflow.
- Keep the original C# project untouched; Rust files live in the Rust port branch/repository.

**Exit gate**

- Portable package starts on a clean Windows 11 profile, retains user settings, and uninstalls/removes cleanly.
- No .NET runtime is required by the delivered Rust binary.
- The release notes distinguish `SOURCE_REVIEWED` behavior from unperformed runtime differential checks.

## Requirement-to-phase map

| Catalog range | Main phase | Dependency |
|---|---:|---|
| `APP-001`–`APP-005` | 2 | Win32/settings PoT |
| `OVR-001`–`OVR-009` | 1, 5 | Renderer and HWND PoT |
| `TEL-001`–`TEL-002`, `TEL-005`–`TEL-010` | 3 | Snapshot/config/provider shell |
| `TEL-003`–`TEL-004` | 4 | Telemetry core and actual GPU hardware |
| `VIS-001`–`VIS-008` | 5 | Overlay renderer and Settings controls |
| `UI-001`–`UI-003` | 2, 3, 6 | Selected Settings toolkit and providers |

## Test and evidence policy

- **Unit tests:** pure logic only (unit conversion, percent formatting, label selection, option defaults, config migration, retry/backoff rules).
- **Windows component checks:** HWND styles, per-pixel transparency, context menu commands, taskbar positioning, registry entry, DPAPI, display/DPI events.
- **Manual hardware checks:** CPU/RAM/network/disk and every supported GPU vendor. Record exact PC/GPU/driver/Windows setup.
- **No-.NET proof:** verify the build log contains only Rust/MSVC commands, the process tree contains no .NET host/original app, and the package does not depend on `hostfxr` or `coreclr`.
- **Parity claims:** source review plus Rust-side execution can establish source-derived coverage. It cannot establish direct old-vs-new runtime equivalence without running the original; report that limitation accurately.

## Not included in this baseline plan

- OpenCode Go usage, ChatGPT/Codex usage, DeepSeek balance, Zen credit balance.
- Windows toast notifications, usage history/CSV, automatic update service.
- Linux/macOS support, Microsoft Store, public distribution, pixel-perfect recreation of the WPF dashboard.

These can be planned after Phase 6 without changing original baseline requirement IDs.
