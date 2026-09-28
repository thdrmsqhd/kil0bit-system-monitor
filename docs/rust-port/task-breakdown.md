# Rust Port Task Breakdown

- **Parent plan:** [`implementation-plan.md`](implementation-plan.md)
- **Acceptance baseline:** [`original-feature-catalog.md`](original-feature-catalog.md)
- **Function-level implementation tasks and I/O contracts:** [`function-task-breakdown.md`](function-task-breakdown.md)
- **Baseline revision:** `c1173f9ecce858931f694661457ef2b905696da4`
- **Extension scope:** after original parity work, add requested AI coding usage features under separate `AIU-*` IDs below.
- **Task status:** see the Phase 0 execution record below; later phase tasks remain `TODO`.
- **No-.NET rule:** no task builds or launches the original .NET application, installs .NET, or invokes `dotnet`. Rust verification runs on Windows with Rust/MSVC only.

## How to use the task IDs

- Keep task IDs stable when updating progress. Record completion evidence beside the task or in the requirement verification matrix.
- Treat the `P*-T*` rows below as phase work packages. Schedule and complete their `F*-*` function tasks from `function-task-breakdown.md` individually; each specifies input, output, original-source mapping, and acceptance check.
- A task is done when its listed output exists, its acceptance checks pass, and the affected catalog IDs have updated statuses.
- `RUN_PASS` means the Rust behavior ran successfully on Windows. `SOURCE_REVIEWED` means the Rust behavior was checked against the catalog's source-derived contract. Do not substitute one status for the other.
- `RUNTIME_DIFF_NOT_RUN` remains the project-wide statement that the original .NET executable was not run for direct comparison.

## Phase 0 execution record (2026-09-28)

| Task | Status | Evidence / blocker |
|---|---|---|
| P0-T01 Freeze baseline | `DONE` | Baseline SHA and scope recorded in the catalog and function contracts. |
| P0-T02 Resolve source/document discrepancies | `DONE` | Source-derived decisions recorded in the feature catalog. |
| P0-T03 Create traceability matrix | `DONE` | [`traceability.md`](traceability.md) maps all 35 baseline IDs to task/function contracts and verification evidence. |
| P0-T04 Define architecture decision record | `DONE` | [`decisions/ADR-001-rust-windows-architecture.md`](decisions/ADR-001-rust-windows-architecture.md); renderer/Settings toolkit selection intentionally gated on Phase 1 PoT. |
| P0-T05 Establish Rust-only workspace and build policy | `DONE` | GitHub Actions `windows-2022` run `36401316507` passed format, Clippy, tests, and x64 MSVC release build from a clean checkout. Separate run `36402994870` launched the overlay, exercised its context menu and Settings window, and confirmed clean shutdown. No .NET commands were executed. |

No .NET command, .NET runtime, or original executable has been invoked. P0-T05 passed on a clean Windows Rust/MSVC CI host. The implementation branch is `rust/phase-0`; later phase gates remain open as listed below. Linux and Windows verification evidence is recorded in [`traceability.md`](traceability.md).

## Phase 0 — Baseline and design freeze

| Task | Implementation work | Depends on | Done when | Catalog |
|---|---|---|---|---|
| P0-T01 Freeze baseline | Record upstream URL, exact baseline SHA, source files inspected, license, and excluded AI extension scope. | — | Metadata is in the project docs and all implementation tasks reference the same SHA. | All |
| P0-T02 Resolve source/document discrepancies | For each catalog discrepancy, record the source-derived interpretation and unresolved behavior. Use code paths as the baseline; do not run the original app. | P0-T01 | The five listed mismatches have a written implementation decision and no unresolved choice blocks the PoT. | TEL-009, OVR-002, VIS-002, catalog notes |
| P0-T03 Create traceability matrix | Map every catalog requirement to Rust module, task ID, automated test, Windows run check, and evidence path. | P0-T01 | All 35 IDs have one owner task and an evidence field; no ID is unassigned. | All |
| P0-T04 Define architecture decision record | Compare native Win32 controls and Rust Settings UI options; define overlay HWND ownership, renderer, telemetry boundary, config format, and UI-to-overlay state flow. | P0-T01 | ADR lists chosen options, alternatives considered, reasons, and PoT exit conditions. | OVR-001, OVR-009, UI-001 |
| P0-T05 Establish Rust-only workspace and build policy | Create Cargo workspace, Rust toolchain pin, MSVC target, dependency policy, test commands, and CI workflow that invokes Rust commands only. | P0-T04 | Clean Windows checkout runs format, check/test, and release build; workflow contains no .NET setup or invocation. | Build prerequisite |

## Phase 1 execution record (2026-09-28)

| Task | Implementation | Runtime verification |
|---|---|---|
| P1-T01 Create native overlay HWND | Implemented Win32 class registration, layered popup creation, close handling, message loop, cleanup, and AppBar position notification in `crates/system-monitor-windows/src/win32.rs`. | `RUN_PASS`: Windows CI run `36402994870` found a visible `WS_EX_LAYERED` HWND with valid bounds and observed successful process exit after `WM_CLOSE`. |
| P1-T02 Present a transparent bitmap | Implemented deterministic premultiplied BGRA capsule/text bitmap and layered presentation; three pixel/scale tests pass. | `RUN_PASS`: Windows CI run `36403912188` read the rendered white glyph from the desktop DC as `#f2f2f2`; layered surface dimensions were 192×52. |
| P1-T03 Prototype drag and lock | Implemented `WM_NCHITTEST` caption hit-testing plus a right-click Lock/Unlock menu. | `RUN_PASS`: Windows CI run `36405034921` dragged while unlocked, confirmed unchanged bounds while locked, then confirmed dragging after unlock; run completed successfully on Windows Server 2022. |
| P1-T04 Prototype taskbar snap and position | Implemented primary taskbar owner/AppBar registration, vertical centering, Snap/Free menu toggle, temporary-file X/Y restore, and `ABM_WINDOWPOSCHANGED` notification. | `RUN_PASS` (toggle behavior): Windows CI run `36405034921` verified startup taskbar centering and Free/Snap transitions on a hosted shell. Restart persistence remains unverified. |
| P1-T05 Prototype context menu and Settings window | Implemented Lock, Snap, Settings, and Exit PoT menu entries plus a native Settings window whose button changes overlay accent live. | `RUN_PASS`: Windows CI run `36405034921` opened the native context menu, clicked Settings, confirmed the visible Settings HWND, and closed the app cleanly. |
| P1-T06 Exercise DPI/display transitions | Implemented per-monitor-v2 awareness request, `WM_DPICHANGED` bitmap scaling, suggested-bounds handling, and taskbar recentering on display/settings changes. | Scale math tests `RUN_PASS`; multiple physical display scales are not available on the current Windows Server CI runner and remain untested. |
| P1-T07 Select renderer/UI toolkit and measure PoT | PoT uses a software BGRA surface + Win32 `UpdateLayeredWindow` and native Win32 Settings control. | `IN_PROGRESS`: Windows Server 2022 confirms HWND, compositing, menu, snap/free, drag, and lock behavior. Startup/resource measurements, settings live-property verification, Windows 11/DPI coverage, and final ADR selection remain open; Wine is unavailable in the local container. |

## Phase 1 — Win32 and renderer proof of technology

| Task | Implementation work | Depends on | Done when | Catalog |
|---|---|---|---|---|
| P1-T01 Create native overlay HWND | Register window class, create/destroy HWND, retain callbacks safely, handle close and shutdown. | P0-T05 | Rust GUI process creates one overlay window and exits without orphan windows or threads. | OVR-001 |
| P1-T02 Present a transparent bitmap | Create a 32-bit premultiplied-alpha surface and present it through a layered-window API. Confirm transparent and opaque pixels render correctly. | P1-T01 | A sample shape/text overlay has clean per-pixel transparency and no black/white fringe. | OVR-001 |
| P1-T03 Prototype drag and lock | Implement hit testing, mouse capture, move messages, and a lock flag in the PoT. | P1-T01 | Drag moves the window when unlocked and is ignored when locked. | OVR-003, OVR-004 |
| P1-T04 Prototype taskbar snap and position | Attach to the primary taskbar, calculate placement, switch to free placement, and persist a temporary X/Y value. | P1-T01 | Toggle between snapped/free states; saved X/Y is restored after app restart. | OVR-002, OVR-003 |
| P1-T05 Prototype context menu and Settings window | Open a native popup menu and separate Settings window; pass a test setting back to overlay. | P1-T01 | Menu opens in a usable direction; Settings opens, closes, and changes one live overlay property. | OVR-005, UI-001 |
| P1-T06 Exercise DPI/display transitions | Process DPI and display change notifications; move between monitors with different scale factors. | P1-T02, P1-T04 | No stale coordinates, clipped overlay, or crash after monitor/scale changes. | OVR-009 |
| P1-T07 Select renderer/UI toolkit and measure PoT | Compare candidate renderer and Settings UI from P0-T04 for stability, memory, startup time, and native HWND interoperability. Finalize ADR. | P1-T01–P1-T06 | One renderer and one Settings toolkit are selected; known visual differences and performance figures are recorded. | OVR-001, UI-001 |

## Phase 2 — App shell, settings, persistence, and lifecycle

| Task | Implementation work | Depends on | Done when | Catalog |
|---|---|---|---|---|
| P2-T01 Define config schema | Add versioned Rust config types with defaults for all original settings, validation/clamping, and unknown-field tolerance. | P0-T03, P1-T07 | Defaults match catalog; round-trip serialization and version tests pass. | APP-004, APP-005 |
| P2-T02 Import legacy config read-only | Read the original AppData JSON only when user chooses/import is available; map supported fields into Rust schema and preserve unknown/unsupported values in a report. | P2-T01 | Import test proves the old file bytes are unchanged and mapped values match source schema. | APP-004 |
| P2-T03 Persist Rust config safely | Write to a Rust-specific AppData directory using temp-file + atomic replacement; handle missing/corrupt files and backup/restore policy. | P2-T01 | Restart restores settings; malformed input returns safe defaults without losing the source file. | APP-004 |
| P2-T04 Implement single-instance behavior | Use a named Windows mutex and activation message/IPC to raise the existing Settings window on second launch. | P1-T05 | Repeated launch leaves one Rust process and activates the existing instance. | APP-002 |
| P2-T05 Implement startup registration | Add/remove a per-user Run entry using the Rust executable path and `--startup`; do not overwrite the original app's entry. | P2-T03 | Toggle writes/removes only the Rust-specific value; startup launch skips Settings. | APP-001, APP-003 |
| P2-T06 Build Settings navigation shell | Implement Home, General, Monitoring, Appearance, About routes and dashboard links with the selected toolkit. | P1-T07 | Every route opens from navigation and returns without losing config edits. | UI-001 |
| P2-T07 Bind General settings and reset flows | Wire overlay enable, snap, startup, lock, fullscreen hide, topmost, refresh options, Save & Close, Quit, reset appearance, and reset all. | P2-T01, P2-T03, P2-T06 | Every control changes config and live state where applicable; canceling reset leaves values unchanged. | APP-003–APP-005, UI-003 |
| P2-T08 Verify app lifecycle and config | Exercise normal launch, `--startup`, second launch, reset, save/close, quit, corrupted config, and legacy import. | P2-T02–P2-T07 | Evidence exists for each lifecycle path; affected IDs have `RUN_PASS` and `SOURCE_REVIEWED`. | APP-001–APP-005, UI-001, UI-003 |

## Phase 3 — Telemetry core: CPU, RAM, network, and disks

| Task | Implementation work | Depends on | Done when | Catalog |
|---|---|---|---|---|
| P3-T01 Define snapshot and polling coordinator | Define immutable snapshot and unavailable/error states; run polling off the UI thread with cancellation, bounded cadence, and one in-flight poll. | P2-T01 | UI can consume snapshots without sharing mutable provider state; stop/restart does not leak a worker. | TEL-001–TEL-010 |
| P3-T02 Implement CPU and RAM providers | Read total CPU load and used physical memory through Windows APIs/performance counters. | P3-T01 | Values update at configured cadence; provider failures do not terminate app. | TEL-001, TEL-002 |
| P3-T03 Enumerate/select network adapters | Match eligible active adapter filtering to source-derived rules; populate dynamic adapter list and persist selection. | P2-T06, P3-T01 | UI list reflects current adapters and retains selection across restart. | TEL-005, UI-002 |
| P3-T04 Implement network rates and formatter | Calculate byte deltas per interval, handle counter reset/reconnect, aggregate Default adapters, calculate selected adapter, and format KB/s/MB/s/GB/s. | P3-T03 | Fixture tests cover zero/negative delta, thresholds, rounding, aggregate and single-adapter cases. | TEL-005, TEL-006 |
| P3-T05 Enumerate physical-disk instances | Discover physical disk counters and map counter instance names to displayable drives. | P3-T01 | Drives refresh safely when attached/removed; `_Total` is not presented as an individual drive. | TEL-007, TEL-009, UI-002 |
| P3-T06 Implement disk activity and storage usage | Read per-disk activity and drive-letter used-space percentage; handle unready/removable drives. | P3-T05 | Fixture and Windows checks cover zero, full, unavailable, and removable drives. | TEL-007, TEL-008 |
| P3-T07 Implement disk selection and refresh settings | Add All/None/multiple selection, persist order, wire polling choices, and apply interval changes without restarting app. | P2-T06, P3-T05 | Selected drive set and interval take effect immediately and survive restart. | TEL-009, TEL-010, UI-002 |
| P3-T08 Test core telemetry | Add deterministic provider/formatter tests and live Windows checks against independent Windows measurement tools. | P3-T02, P3-T04, P3-T06, P3-T07 | Tests pass and live CPU/RAM/network/disk evidence is attached to each ID. | TEL-001, TEL-002, TEL-005–TEL-010 |

## Phase 4 — GPU load and temperature

| Task | Implementation work | Depends on | Done when | Catalog |
|---|---|---|---|---|
| P4-T01 Discover GPUs and map selection | Enumerate adapters, stable display names and device identifiers; bind Settings selection to provider. | P2-T06, P3-T01 | Adapter list is dynamic; switching device reinitializes provider without restarting. | TEL-003, UI-002 |
| P4-T02 Implement NVIDIA provider | Use bounded `nvidia-smi` queries or equivalent supported native path; parse usage/temp and terminate hung subprocesses. | P4-T01 | Valid data updates; missing utility, timeout, or malformed output produces unavailable state. | TEL-003, TEL-004 |
| P4-T03 Implement AMD provider | Reproduce the source-supported AMD load path using a Rust-compatible binding/FFI strategy; isolate DLL/API failures. | P4-T01 | AMD load is verified on an AMD device; absent API cleanly falls back. | TEL-003 |
| P4-T04 Implement Windows fallback providers | Add Windows GPU performance-counter fallback and D3DKMT temperature path where available. | P4-T01 | Providers select/fallback deterministically and return values in documented units. | TEL-003, TEL-004 |
| P4-T05 Add GPU absence and hardware acceptance cases | Cover no GPU, multiple GPUs, zero reading, unsupported temperature, device changes, and provider failure. | P4-T02–P4-T04 | Non-supported cases keep app alive; each vendor status is evidence-backed or `BLOCKED`. | TEL-003, TEL-004 |

## Phase 5 — Full overlay behavior and rendering

| Task | Implementation work | Depends on | Done when | Catalog |
|---|---|---|---|---|
| P5-T01 Implement metric layout model | Map snapshot to stable metric pairs, labels, compact labels, reserves, disk columns, and dynamically measured width. | P3-T01, P4-T01 | Every enabled metric appears in deterministic order; values changing do not cause avoidable width jitter. | OVR-001, TEL-007–TEL-008, VIS-001–VIS-002 |
| P5-T02 Render Text and Compact modes | Render values, labels, typography, capsule shapes, background plate, hover effect, and alpha correctly using selected renderer. | P1-T07, P5-T01 | Text/Compact screenshots have no clipping and match chosen source-derived appearance behavior. | OVR-001, VIS-002, VIS-003, VIS-008 |
| P5-T03 Implement global and section palettes | Add preset themes, global colors, per-section label/value overrides, inheritance, and clear/reset behavior. | P2-T06, P5-T02 | Each preset and palette control changes expected renderer colors live and persists. | VIS-004–VIS-006 |
| P5-T04 Implement scale, spacing, and fonts | Add font choices, bold, scale range, column gap range, font fallback, and cache invalidation. | P2-T06, P5-T02 | Min/default/max settings render without overlap; font changes appear immediately. | VIS-003, VIS-007 |
| P5-T05 Implement overlay position persistence and drag rules | Persist x/y at move end; support snap/free/lock transitions and bounds validation. | P1-T03, P1-T04, P2-T03 | Restart restores free position; lock/snap behavior follows config. | OVR-002–OVR-004 |
| P5-T06 Implement AppBar/taskbar integration | Register/unregister appbar, place relative to taskbar, restore owner behavior, and handle taskbar movement/visibility changes. | P1-T04, P5-T05 | Snap transitions do not strand overlay or alter free coordinates unexpectedly. | OVR-002 |
| P5-T07 Implement context menu commands | Add Settings, Task Manager, topmost, full-screen hide, lock, snap, About, and Exit; show correct checkmarks and positioning. | P1-T05, P2-T07, P5-T05 | Every command produces expected action and persisted check state. | OVR-005, OVR-008, UI-003 |
| P5-T08 Implement full-screen visibility and transitions | Detect shell/full-screen states, debounce/fade, hide/reveal, and avoid repeated z-order flicker. | P1-T01, P5-T06 | Repeated full-screen enter/exit and Task View transitions stay stable. | OVR-006–OVR-008 |
| P5-T09 Integrate DPI/display and multi-disk layout | Reflow metrics, update bitmap size/scale, preserve valid location, and render all selected drives across monitor changes. | P1-T06, P5-T01–P5-T08 | No clipping or lost position through display/DPI changes and large disk selection. | OVR-009, TEL-009, VIS-007 |

## Phase 6 — Integration, reliability, and no-.NET acceptance

| Task | Implementation work | Depends on | Done when | Catalog |
|---|---|---|---|---|
| P6-T01 Prepare clean Windows test host | Set up clean Windows 11 Sandbox/VM or Rust-only host; record OS, scales, GPU, adapters, disks, and taskbar configuration. | P0-T05 | Host setup is reproducible and no .NET setup is needed. | All |
| P6-T02 Verify Rust build and native dependencies | Run format, clippy, test, release build; inspect PE imports/package; inspect process tree during run. | P2-T08, P3-T08, P4-T05, P5-T09 | Commands are Rust/MSVC-only; no .NET host/original process is observed; evidence saved. | All, no-.NET gate |
| P6-T03 Verify APP/UI workflows | Run launch modes, second launch, startup registry, settings navigation, reset, quit, save, persistence and migration cases. | P6-T01, P6-T02 | APP/UI IDs have `RUN_PASS` + `SOURCE_REVIEWED`, or a clear blocker. | APP-001–APP-005, UI-001–UI-003 |
| P6-T04 Verify OVR/VIS interactions | Run transparency, position, lock, menu, taskbar snap, full-screen, topmost, visual controls, scale/font/color and multi-monitor scenarios. | P6-T01, P6-T02 | OVR/VIS IDs have screenshots or action logs and a status. | OVR-001–OVR-009, VIS-001–VIS-008 |
| P6-T05 Verify telemetry with independent tools | Compare available sensor values against Task Manager, Performance Monitor, GPU vendor utility, network transfer, and Explorer. | P6-T01, P6-T02 | Telemetry evidence has hardware metadata, comparison method, tolerance, and status. | TEL-001–TEL-010 |
| P6-T06 Run failure/recovery and soak checks | Test unavailable counters, adapter reconnect, missing GPU tool, disk removal, corrupt settings, network loss, long duration, and clean exit. | P6-T03–P6-T05 | Failures remain local, recover when source returns, and do not leak processes/resources. | APP-004, OVR-006, TEL-001–TEL-010 |
| P6-T07 Close catalog statuses and exclusions | Update the traceability matrix and catalog statuses; list blocked/unrun items and approved exclusions. | P6-T03–P6-T06 | No requirement is silently omitted; project states `RUNTIME_DIFF_NOT_RUN`. | All |

## Phase 7 — Packaging and personal release

| Task | Implementation work | Depends on | Done when | Catalog |
|---|---|---|---|---|
| P7-T01 Set release metadata and notices | Set app name/version, original MIT license/attribution, settings location, supported OS, hardware limits, and no-.NET runtime statement. | P6-T07 | Release metadata and notices match package contents. | UI-003 |
| P7-T02 Create portable x64 package | Package release executable and required native resources only; create checksums and a clean install/run note. | P7-T01 | Extracted package starts from a clean profile and includes no .NET runtime. | APP-001, no-.NET gate |
| P7-T03 Validate profile cleanup and upgrade | Test first-run config creation, upgrade preserving Rust settings, backup/restore, and clean removal; do not touch original app config. | P7-T02 | Rust files can be removed without altering original application data. | APP-004 |
| P7-T04 Publish personal release evidence | Attach build/test summary, catalog status, known limitations, and SHA/checksum. | P7-T03 | Release notes make unverified hardware and `RUNTIME_DIFF_NOT_RUN` explicit. | All |

## Phase 8 — AI coding tool usage extension

This phase follows baseline acceptance so provider failures and added UI cannot obscure original feature parity. Recheck provider endpoints and payloads against current documentation/behavior before implementation; the prior handoff's API notes are design inputs, not permanent contracts.

| Task | Implementation work | Depends on | Done when | Extension requirement |
|---|---|---|---|---|
| P8-T01 Define AI usage model and provider interface | Model provider identity, usage windows, used/remaining percentages, reset times, update time, stale/error state; define async provider API and fixture HTTP client. | P6-T07, P0-T05 | Providers can be tested with deterministic JSON fixtures and cannot block overlay rendering. | AIU-001, AIU-002 |
| P8-T02 Implement secret storage and key settings | Add OpenCode Go key entry, save/remove/status controls, minimum polling interval, and Windows-user DPAPI encryption outside plaintext config. Never log or redisplay the saved secret. | P2-T03, P2-T06, P8-T01 | Key survives restart for same user; ciphertext is absent from JSON; remove stops requests and clears state. | AIU-001 |
| P8-T03 Implement OpenCode Go provider | Call the currently verified usage endpoint; parse 5-hour/rolling, weekly, and monthly percentages and reset intervals; validate 0–100 units and omitted windows. | P8-T01, P8-T02 | Fixture tests cover valid, missing, malformed, 401/403, rate-limit, and server-error responses; one percent remains one percent. | AIU-002 |
| P8-T04 Add refresh, backoff, and stale-state handling | Default to five-minute polling, enforce a one-minute minimum, back off after repeated failures, retain last valid snapshot, and mark it stale. | P8-T03 | Timers never overlap; disabling/removing key stops requests; transient failure retains a marked stale value; recovery updates it. | AIU-002, AIU-003 |
| P8-T05 Display OpenCode Go on overlay | Add selectable 5-hour/weekly/monthly segments, remaining/used format, compact labels, and threshold colors without width jitter. | P5-T01, P5-T02, P8-T04 | Each window can be toggled; both percentage conventions work in Text and Compact modes. | AIU-003 |
| P8-T06 Extend Settings for AI usage | Add AI Usage route, key state, display selection, percentage convention, poll interval, refresh status, and remove-key action. | P2-T06, P8-T02, P8-T04 | Settings persist and status updates without exposing credential material. | AIU-001–AIU-003 |
| P8-T07 Add read-only Codex usage provider | Read Codex auth file without writing it; call the currently verified usage endpoint; parse 5-hour and weekly windows; never refresh tokens. | P8-T01, P8-T06 | Mock tests prove auth-file bytes are unchanged; expired token is provider-local error; no token/response secrets are logged. | AIU-004 |
| P8-T08 Add DeepSeek balance provider | Store API key with DPAPI; call current documented balance API; parse availability and currency-specific balances. | P8-T01, P8-T02, P8-T06 | Fixtures cover USD/CNY balances, unavailable result, invalid key, and API error. | AIU-005 |
| P8-T09 Add optional Zen credit balance | If included after P8-T06, determine supported console-cookie/workspace flow, minimize cookie exposure, use separate opt-in setting, and parse subscription/billing balance. | P8-T01, P8-T06 | Feature remains disabled by default; cookies are never logged; failure affects only Zen segment. | AIU-006 (optional) |
| P8-T10 Run AI integration and secret audit | Run mock-server end-to-end, offline/error recovery, settings/overlay visibility, and secret-file/diagnostic checks. | P8-T03–P8-T08; P8-T09 if selected | Provider statuses are independent, fixtures pass, and no credential leaks into logs/config/repository. | AIU-001–AIU-006 |

## Critical path and parallelizable work

```text
P0 baseline + Rust workspace
  → P1 native-window PoT and renderer decision
  → P2 app/config/settings shell
  → P3 core telemetry ─→ P4 GPU providers
  → P5 full overlay and appearance integration
  → P6 Windows acceptance and catalog closure
  ├→ P7 baseline portable package
  └→ P8 AI usage extension (follow-up package)
```

- P3 network and disk provider tasks can proceed independently after P3-T01.
- P4 NVIDIA, AMD, and Windows fallback providers can proceed independently after P4-T01, but share the snapshot contract.
- UI settings construction can proceed in parallel with provider implementation after P2-T06, using mock snapshots.
- P6 hardware checks can be split by available hardware; each untested vendor stays `BLOCKED`.
- P8 starts with OpenCode Go. Codex and DeepSeek follow after the shared model/settings path; Zen balance is opt-in.

## Definition of done for a task

1. Implementation is in the Rust port workspace and follows the no-.NET rule.
2. Unit/component/manual verification appropriate to the task is recorded.
3. Related catalog IDs are updated with `RUN_PASS`, `SOURCE_REVIEWED`, `BLOCKED`, `DIFFERENT`, or `NOT_RUN`.
4. Evidence points to a test result, Windows run record, screenshot, or source review note.
5. No task completion implies direct comparison with the original executable; that remains `RUNTIME_DIFF_NOT_RUN`.
6. AI-provider tests use mocked HTTP responses by default; live account credentials are never required for CI.

## Separate extension requirements

| ID | Behavior |
|---|---|
| AIU-001 | Secure OpenCode Go workspace API key settings, encrypted for the current Windows user and absent from plaintext config/logs. |
| AIU-002 | OpenCode Go 5-hour/weekly/monthly windows, reset times, resilient polling, and stale/error state. |
| AIU-003 | Selectable AI usage segments with remaining/used percentage modes and threshold colors. |
| AIU-004 | Optional Codex 5-hour/weekly usage from a read-only auth file, with no token refresh or auth-file writes. |
| AIU-005 | Optional DeepSeek balance and currency display using a separately protected API key. |
| AIU-006 | Optional Zen subscription/credit balance using a separately enabled console-auth flow. |

These extension IDs do not change the 35-item original-feature baseline. Windows toast alerts, usage history/CSV, Linux/macOS, Store/public distribution, and pixel-perfect WPF recreation remain out of scope unless separately planned.
