# Original Feature Catalog for a Rust Port

- **Reference repository:** `kil0bit-kb/kil0bit-system-monitor`
- **Reference revision:** `c1173f9ecce858931f694661457ef2b905696da4` (`origin/main`, 2026-09-28 checkout)
- **Purpose:** define observable parity requirements before implementing a Rust version and provide a verification path that never builds or runs the .NET original.
- **Scope:** original v3.0.0 features only. The separate AI usage extension is excluded from this baseline and should be added as its own requirement set.
- **Evidence rule:** README/GUIDE descriptions identify intended behavior; code paths identify implemented behavior. During Rust development and verification, do not invoke `dotnet`, build the original, or launch the original .NET executable. Resolve documentation/source ambiguities through explicit source review and record the resulting interpretation; do not claim a runtime differential test was performed.

## Status vocabulary

| Status | Meaning |
|---|---|
| `NOT_RUN` | No direct original-versus-Rust comparison has been performed. |
| `RUN_PASS` | The Rust feature was exercised on Windows and met its expected result. |
| `SOURCE_REVIEWED` | The Rust implementation was checked against the original source and its documented contract. This does not mean original runtime behavior was compared. |
| `DIFFERENT` | The Rust behavior conflicts with the source-derived contract or approved interpretation. |
| `BLOCKED` | Required hardware or Windows setup is unavailable. |
| `RUNTIME_DIFF_NOT_RUN` | Direct comparison with the original executable was intentionally not performed under the no-.NET rule. |

Every requirement starts at `NOT_RUN`. Source inspection is evidence for the contract, not runtime acceptance. Record `RUN_PASS` and `SOURCE_REVIEWED` separately for each item; retain `RUNTIME_DIFF_NOT_RUN` as a project-wide limitation.

## Functional requirements

| ID | Area | Original observable behavior to preserve | Evidence in original | Parity check |
|---|---|---|---|---|
| APP-001 | Startup | A normal launch starts the monitor and opens Settings. A launch with `--startup` starts without opening Settings. | `App.xaml.cs` | Launch normally and with `--startup`; compare windows and overlay. |
| APP-002 | Single instance | A second launch forwards a request to open Settings to the existing instance, then exits. | `App.xaml.cs`, named mutex and `WM_SHOW_SETTINGS` | Launch twice; confirm one process/overlay and existing Settings is activated. |
| APP-003 | Startup registration | “Launch on Startup” adds/removes a per-user Run registry entry and starts with `--startup`. | `Services/StartupService.cs`, General settings | Toggle, inspect behavior after sign-in/relaunch, then disable and confirm removal. |
| APP-004 | Settings persistence | Configuration survives app restart in the user AppData config file; changing Launch on Startup synchronizes its registry entry. | `Services/ConfigService.cs`, `Models/SystemMetrics.cs` | Change several options, restart, compare every saved value. |
| APP-005 | Reset | “Reset All Settings” asks for confirmation, restores the app defaults, and disables startup. “Reset Appearance” restores appearance defaults. | `SettingsWindow.xaml.cs` | Cancel each confirmation and verify no change; confirm and compare to defaults. |
| OVR-001 | Overlay surface | A borderless, transparent, top-level overlay renders as a compact, dynamically sized row of metric capsules/text. | `OverlayWindow.cs`, Win32 layered window + GDI+ rendering | Compare screenshot and bounds at the same Windows scale and settings. |
| OVR-002 | Taskbar snap | With Snap to Taskbar on, overlay attaches to the primary taskbar and is vertically centered; its horizontal position is retained. Turning snap off restores free placement. | `OverlayWindow.cs`, `AttachToTaskbar`, `AlignToTaskbarCenter` | Toggle on/off; compare position before and after restart. Test taskbar at alternate edge if supported by original. |
| OVR-003 | Free placement and drag | With snap and lock off, left-drag moves the overlay; its position is saved after moving. | GUIDE.md, `OverlayWindow.cs` window messages | Drag, restart, and confirm the same location. |
| OVR-004 | Position lock | Lock Position prevents left-drag movement. It can be toggled from Settings and the context menu. | `OverlayWindow.cs`, General settings | Try dragging while locked/unlocked; confirm context menu check state follows setting. |
| OVR-005 | Context menu | Right-click offers Settings, Task Manager, Keep on Top, Hide in Fullscreen, Lock Position, Snap to Taskbar, About, and Exit. Menu placement flips vertically based on overlay position. | `OverlayWindow.cs`, `WndProc` | Exercise every item; test menu near top and bottom halves of the screen. |
| OVR-006 | Overlay visibility | General settings can turn the overlay on/off without exiting the monitor. | `AppConfig.ShowOverlay`, `OverlayWindow.UpdateVisibility` | Toggle both directions and confirm telemetry/app remains available. |
| OVR-007 | Full-screen behavior | When enabled, the overlay hides over detected full-screen apps; it fades/debounces visibility changes and treats Windows shell/task-view windows specially. | `OverlayWindow.cs`, `ShouldShowOverlay`, AppBar callback | Test a borderless full-screen app, browser F11/video, Task View, and return to desktop. |
| OVR-008 | Z-order | Keep on Top controls whether the overlay is kept topmost. | `OverlayWindow.cs`, `EnforceZOrder` | Toggle it and compare overlay visibility over ordinary windows and taskbar. |
| OVR-009 | DPI/display changes | Rendering and overlay geometry account for window DPI and display changes. | `OverlayWindow.cs`, `WM_DPICHANGED`, `WM_DISPLAYCHANGE` | Move between monitors with different scaling; compare size, location, and clipping. |
| TEL-001 | CPU | Displays total processor load as an integer percentage. | `Services/TelemetryService.cs`, `SystemMetrics.CpuUsage` | Compare displayed value with a trusted Windows CPU monitor under load. |
| TEL-002 | RAM | Displays used physical memory as a percentage. | `TelemetryService.cs`, `GlobalMemoryStatusEx` | Compare under idle/load conditions with Windows memory usage. |
| TEL-003 | GPU load | Displays selected GPU load. NVIDIA uses `nvidia-smi`; AMD uses ADL when available; performance counters provide a fallback. | `TelemetryService.cs`, `InitializeGpu`, `GetNvidiaUsage`, `UpdateGpuCounters` | Test available vendors and compare under GPU load; mark unsupported hardware `BLOCKED`, not `PASS`. |
| TEL-004 | GPU temperature | Displays GPU temperature when the selected device/API supplies it; otherwise shows `N/A`. Temperature reads are cached. | `TelemetryService.cs`, `GetGpuTemperature` | Compare with vendor utility and test an unsupported/no-temperature device. |
| TEL-005 | Network upload/download | Displays separate upload and download rates. Default aggregates eligible active Ethernet/Wi-Fi adapters; user can select an individual adapter. | `TelemetryService.cs`, `GetNetworkStats`, Monitoring settings | Generate upload and download traffic; compare units and selected-adapter behavior. |
| TEL-006 | Network formatting | Rates use KB/s, MB/s, or GB/s with magnitude-dependent decimal precision. | `TelemetryService.cs`, `FormatNet` | Feed/observe values across unit boundaries and compare displayed rounding. |
| TEL-007 | Disk activity | Displays activity percentage for each selected physical-disk instance. | `TelemetryService.cs`, disk counters, `OverlayWindow.PrepareMetricsData` | Run disk I/O; compare selected drives and activity values. |
| TEL-008 | Disk used space | Displays used-space percentage for selected ready drive letters. | `TelemetryService.cs`, `DriveInfo`; Monitoring settings | Compare with Explorer properties for each selected drive. |
| TEL-009 | Multi-disk selection | The user can select multiple drives or none; each selected drive gets its own column. Guide describes up to nine drives and a 3x3 balance. | `SettingsWindow.xaml.cs`, `SelectedDisks`; GUIDE.md | Test 0, 1, 3, 9, and more than 9 visible drives. Determine whether nine is enforced or only guidance. |
| TEL-010 | Sensor refresh | Sensor refresh options are 500, 1000, 2000, and 5000 ms; default is 1000 ms. | `SettingsWindow.xaml`, `AppConfig.UpdateInterval`, `TelemetryService.Config_PropertyChanged` | Change each interval and measure update cadence; verify saved default. |
| VIS-001 | Metric selection | CPU, RAM, GPU load, GPU temperature, network up/down, disk use, and disk activity can each be independently shown/hidden. | `AppConfig` flags; Monitoring settings | Toggle each metric independently and compare overlay contents. |
| VIS-002 | Display modes | Text mode shows full labels; Compact mode uses shortened labels. | `OverlayWindow.PrepareMetricsData`, Appearance settings | Compare all enabled metrics in both modes, including multi-disk labels. |
| VIS-003 | Typography | Font family and bold/regular weight are configurable. Listed fonts: Segoe UI, Segoe UI Variable Display, Inter, Roboto, Consolas. | `SettingsWindow.xaml`, GDI+ font rendering | Compare each selectable font and weight; missing installed font behavior must be recorded. |
| VIS-004 | Preset themes | Presets: Default, Cyberpunk, Matrix, Stealth, Synthwave, Midnight Gold, Frost, Inferno, Toxic, Nordic. Presets set palette; some also set font/weight. | `SettingsWindow.xaml`, `ThemeCombo_SelectionChanged` | Select each preset and compare label/value/background colors, font, and weight. |
| VIS-005 | Global colors | User can select metric accent, label tone, background plate color, and capsule color. | Appearance settings, `AppConfig` color values | Change each independently and compare rendered pixels. |
| VIS-006 | Per-section colors | Network, CPU/RAM, GPU/temp, and disk have independent label and metric colors; unset values inherit global colors. | `AppConfig.*LabelColorHex`, `*AccentColorHex`; Appearance settings | Set, clear, and inherit each section’s colors. |
| VIS-007 | Scale and spacing | Overall scale ranges from 0.5 to 2.0; column spacing ranges 0–20. | Appearance settings, `OverlayWindow.UpdateLayer` | Compare minimum/default/maximum and ensure no text clipping. |
| VIS-008 | Capsules and plate | Capsules can be enabled/disabled; a background plate can be enabled and recolored. | `AppConfig.ShowPods`, `ShowBackground`; `OverlayWindow` rendering | Toggle both independently and compare shape, spacing, and background. |
| UI-001 | Settings navigation | Dashboard links navigate to Home, General, Monitoring, Appearance, About; sidebar navigation selects the same sections. | `SettingsWindow.xaml`, `SelectSection` | Open every section through card and sidebar where available. |
| UI-002 | Monitoring choices | Settings list available network adapters, GPUs, and disk instances dynamically; adapter selections persist. | `TelemetryService.GetAvailable*`, Monitoring section | Compare choices with current machine and verify selections after restart. |
| UI-003 | About and quit | About shows app version and developer links. Settings offers Quit Application and Save & Close. | About section and footer in `SettingsWindow.xaml` | Verify version/links, quit behavior, and save/close behavior. |

## Known documentation/source mismatches to resolve on the original

1. GUIDE calls the full-label display mode “Standard”; the setting/model use `Text` and `Compact`.
2. GUIDE says disk selection is up to nine drives. The source selection handler stores selected names without an obvious nine-drive check. **Port interpretation:** preserve the source behavior and allow every discovered drive unless another source path proves there is a limit.
3. GUIDE describes a “3x3 layout,” while `PrepareMetricsData` appends each selected drive as another horizontal column. **Port interpretation:** reproduce the horizontal sequence; the 3x3 text is not supported by the inspected renderer.
4. The attachment code looks up `Shell_TrayWnd` and uses its rectangle, which points to the primary taskbar. **Port interpretation:** primary taskbar snapping is required; secondary-taskbar behavior is unspecified and should not be claimed as parity.
5. The project description mentions icon styling, but `AppConfig.DisplayStyle` and the settings list expose `Text` and `Compact`, and the renderer only branches on `Compact`. **Port interpretation:** implement Text and Compact only; do not add an icon mode to the baseline.

## No-.NET build and execution verification

The development machine needs the Rust toolchain and Windows C++ build tools for the MSVC target; it does not need .NET. Microsoft's Windows Rust setup recommends the MSVC toolchain and lists C++ build tools as a prerequisite. This is independent from installing the .NET SDK/runtime. See [Microsoft's Rust on Windows setup](https://learn.microsoft.com/en-us/windows/dev-environment/rust/setup).

1. Build and test only Rust targets, for example:
   ```powershell
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all-targets
   cargo build --release --target x86_64-pc-windows-msvc
   ```
   Cargo's test runner runs the Rust test targets. Do not add .NET setup steps or .NET commands to editor tasks, scripts, CI workflows, or release workflows. See [Cargo test](https://doc.rust-lang.org/cargo/commands/cargo-test.html).
2. Run the produced Rust executable on Windows 11. Prefer a clean Windows Sandbox/VM or a machine without the .NET SDK/runtime installed. Record Windows build, monitor count/resolution/scaling, taskbar edge/auto-hide, GPU vendor, and connected disks.
3. Confirm the process tree contains the Rust app and its native helpers only. Confirm there is no `dotnet.exe`, `Kil0bitSystemMonitor.exe`, or .NET host/runtime process. A non-.NET Rust build should not need `hostfxr.dll` or `coreclr.dll`; inspect imports/dependencies if packaging introduces native helpers.
4. For each requirement, run its parity check on the Rust app and capture screenshots, values, and process observations under its ID. Compare system measurements with independent Windows sources such as Task Manager, Performance Monitor, vendor GPU tools, and Explorer; none require running the original .NET application.
5. Record `RUN_PASS` when the Rust program actually ran and satisfied the expected result. Record `SOURCE_REVIEWED` after checking its implementation against the source-derived behavior. Keep `RUNTIME_DIFF_NOT_RUN` visible: without executing the original app, direct old-vs-new runtime equivalence cannot be established. Unsupported hardware is `BLOCKED`; untested behavior remains `NOT_RUN`.

GitHub-hosted Windows runners are an optional remote build route; their images can have other toolchains installed, so ensure the workflow itself invokes only Rust commands. If the rule means .NET must not even be present on any build host, use a clean self-hosted Windows runner or the user's Rust/MSVC machine instead. GitHub documents `windows-latest` and other hosted Windows runner images [here](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job).

## Resolving original documentation/source mismatches without running .NET

1. Record the exact `origin/main` revision and the source file/property/method supporting the interpretation.
2. Prefer executable code paths over prose where the two disagree, but label the interpretation as source-derived and list unresolved edge cases.
3. Use repository screenshots and docs as visual/interaction references where useful. They cannot prove dynamic behavior.
4. Add an acceptance test for the selected interpretation to the Rust project. If the source does not determine an edge case, choose a behavior explicitly and document it as a port decision rather than claiming exact original parity.

## Initial parity gate

Do not claim “1:1 runtime parity” under the strict no-.NET policy: the original executable is never run, so old-vs-new differential verification is deliberately unavailable. The strongest valid claim is: all feasible requirements have `RUN_PASS`, their implementations have `SOURCE_REVIEWED`, and unresolved differences have documented port decisions. If direct differential proof later becomes necessary, it requires a separately authorized comparison phase that runs the original; that phase is outside this no-.NET workflow.
