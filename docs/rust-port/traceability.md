# Catalog-to-function traceability

Every baseline catalog requirement is assigned to a Rust parent task and one or more function tasks. `SOURCE_REVIEWED` means the behavior was checked against the pinned C# source statically. `RUN_PASS` requires Rust execution evidence on Windows. No row implies comparison by running the original program; that remains `RUNTIME_DIFF_NOT_RUN`.

| Catalog ID | Rust owner task | Function contract(s) | Required verification / evidence |
|---|---|---|---|
| APP-001 | P2-T05, P2-T08 | F2-08, F2-14 | Windows launch record: normal vs `--startup`; no .NET process |
| APP-002 | P2-T04, P2-T08 | F2-07, F2-08 | Two-launch process/activation record |
| APP-003 | P2-T05, P2-T08 | F2-05, F2-06 | Rust-specific HKCU Run add/remove evidence |
| APP-004 | P2-T01–P2-T03, P2-T08 | F2-01–F2-05 | Default, round-trip, corrupt-file and read-only-import tests |
| APP-005 | P2-T07, P2-T08 | F2-11–F2-13 | Confirm/cancel reset tests and settings evidence |
| OVR-001 | P1-T01–P1-T03, P5-T01–P5-T02 | F1-01–F1-04, F5-01–F5-07, F5-14 | Windows screenshot/bounds at recorded scale |
| OVR-002 | P1-T04, P5-T05–P5-T06 | F1-06, F5-11, F5-13 | Snap/free position and restart evidence |
| OVR-003 | P1-T03, P5-T05 | F1-05–F1-06, F5-12 | Drag/save/restart evidence |
| OVR-004 | P1-T03, P5-T05, P5-T07 | F1-08, F5-12 | Locked/unlocked drag and menu check state |
| OVR-005 | P1-T05, P5-T07 | F1-05, F1-07–F1-08 | Menu command and placement checklist |
| OVR-006 | P2-T07, P5-T02 | F2-05, F5-09 | Live visibility toggle evidence |
| OVR-007 | P5-T08 | F5-08–F5-09 | Fullscreen/task view transition log |
| OVR-008 | P5-T07–P5-T08 | F5-10 | Topmost on/off window test |
| OVR-009 | P1-T06, P5-T09 | F1-05, F5-13 | Multi-monitor DPI/display screenshots |
| TEL-001 | P3-T02, P3-T08 | F3-05, F3-13 | Fixture + Windows independent CPU comparison |
| TEL-002 | P3-T02, P3-T08 | F3-06, F3-13 | Fixture + Windows memory comparison |
| TEL-003 | P4-T01–P4-T05 | F3-02, F4-01–F4-05, F4-07 | Vendor-specific run evidence or `BLOCKED` |
| TEL-004 | P4-T02, P4-T04–P4-T05 | F4-03, F4-06 | Temperature and unsupported-device evidence |
| TEL-005 | P3-T03–P3-T04, P3-T08 | F3-03–F3-08, F3-13 | Adapter selection and traffic comparison |
| TEL-006 | P3-T04, P3-T08 | F3-08–F3-09 | Boundary/rounding unit tests |
| TEL-007 | P3-T05–P3-T06, P5-T01 | F3-01, F3-10, F3-12–F3-13, F5-04 | Disk activity fixture and Windows check |
| TEL-008 | P3-T06, P3-T08 | F3-11–F3-13 | Drive-space fixture and Explorer comparison |
| TEL-009 | P3-T05–P3-T07, P5-T09 | F2-10, F3-01, F3-10, F5-04 | 0/1/3/9/>9 selection evidence |
| TEL-010 | P3-T07–P3-T08 | F3-15 | 500/1000/2000/5000 ms cadence evidence |
| VIS-001 | P2-T07, P5-T01 | F2-05, F5-04 | Per-metric toggle tests/screenshots |
| VIS-002 | P1-T07, P5-T01–P5-T02 | F5-01–F5-04 | Text/Compact label fixture and screenshots |
| VIS-003 | P2-T06, P5-T04 | F5-05, F5-07 | Font/weight screenshot and fallback record |
| VIS-004 | P2-T06, P5-T03 | F2-11, F5-06–F5-07 | Ten-preset field comparison |
| VIS-005 | P2-T06, P5-T03 | F2-05, F5-06–F5-07 | Global color persistence and pixel evidence |
| VIS-006 | P2-T06, P5-T03 | F5-06–F5-07 | Section override/inheritance fixtures and screenshots |
| VIS-007 | P2-T06, P5-T04, P5-T09 | F5-05, F5-13–F5-14 | Range and multi-monitor clipping checks |
| VIS-008 | P2-T06, P5-T02 | F5-07, F5-14 | Independent capsule/plate visual toggle evidence |
| UI-001 | P1-T05, P2-T06, P2-T08 | F2-09 | Navigation route tests for cards/sidebar |
| UI-002 | P2-T06, P3-T03, P3-T05, P4-T01 | F3-01–F3-03, F2-10 | Dynamic lists and persisted selections |
| UI-003 | P2-T06–P2-T08 | F2-09, F2-14 | About, Quit, Save & Close evidence |

## Current execution status

| Gate | Status | Evidence |
|---|---|---|
| Baseline SHA/source-derived catalog | `SOURCE_REVIEWED` | `original-feature-catalog.md`, pinned SHA above |
| Catalog ID → task/function ownership | `SOURCE_REVIEWED` | This matrix + `function-task-breakdown.md` |
| Linux Rust workspace | `RUN_PASS` | Rust 1.75 and 1.91: `fmt`, `clippy -D warnings`, workspace tests, and release builds passed offline. The overlay crate has 3 passing bitmap/scale contract tests; the core crate has no tests yet. |
| Linux app guard execution | `RUN_PASS` | `cargo run -p system-monitor-windows --offline` prints the non-Windows guard message and exits normally. This does not exercise overlay behavior. |
| Win32 FFI type-check | `RUN_PASS` | Rust 1.98 stable `cargo clippy --workspace --all-targets --target x86_64-pc-windows-msvc --offline -- -D warnings` and `cargo check --workspace --target x86_64-pc-windows-msvc --offline --config 'build.rustflags=["-Dwarnings"]'` pass. This checks Rust types but does not perform the MSVC link. |
| Windows GNU link artifact | `RUN_PASS` | `cargo build --workspace --target x86_64-pc-windows-gnu --release --offline` linked successfully with MinGW. `file` identifies a PE32+ x86-64 Windows GUI executable; PE imports include USER32, GDI32, SHELL32, and KERNEL32. This does not prove GUI runtime behavior. |
| Wine GUI execution attempt | `BLOCKED` | Ran the GNU executable through Wine 9 under Xvfb. Wine exited before process startup: `wineserver: socket: Operation not permitted`. The container prevents the local socket Wine needs; no application window was created. |
| Windows compile/launch/UI checks | `BLOCKED` | Windows-target source type-check and GNU PE link pass; native Windows MSVC linking and overlay runtime/UI remain unverified. The Wine attempt was blocked by the container socket restriction. Follow [`evidence/p1-windows-run-checklist.md`](evidence/p1-windows-run-checklist.md) for exact Windows-native acceptance steps. |
| Original-vs-Rust runtime comparison | `RUNTIME_DIFF_NOT_RUN` | Intentionally excluded by no-.NET/no-original-executable constraint |
