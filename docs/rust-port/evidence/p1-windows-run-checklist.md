# Phase 1 PoT: Windows 11 local run checklist

This PoT exercises only the Rust binary. It does not build or launch the original application and does not call .NET.

**Policy (2026-10-13):** GitHub Actions no longer runs the Win32 layered-window smoke step. Format, Clippy, tests, MSVC release build, and portable packaging remain in CI. All GUI acceptance (drag, lock, snap, DPI, menu/Settings) is verified only on a real Windows 11 interactive session using this checklist. Do not treat Server 2022 CI results as Windows 11 evidence.

## Host requirements

- Windows 11 x64
- Rust stable toolchain (see `rust-toolchain.toml` if present)
- Visual Studio 2022 C++ Build Tools (MSVC linker)

## Commands

Run from the Rust workspace root:

```powershell
rustc --version
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release --target x86_64-pc-windows-msvc
cargo run --release -p system-monitor-windows
```

Optional local smoke harness (not a CI gate; may fail on drag under some environments):

```powershell
.\target\x86_64-pc-windows-msvc\release\overlay-smoke.exe .\target\x86_64-pc-windows-msvc\release\system-monitor-windows.exe
```

Prefer the manual steps below for acceptance. Record pass/fail per item; only mark `RUN_PASS` when the step succeeds on your machine.

## Expected result (acceptance steps)

1. **Launch / visible capsule** — A borderless dark blue-gray capsule appears near horizontal coordinate 100 and is vertically centered on the primary taskbar by default, with white `RUST` bitmap text. Choosing **Free Position** starts from the saved free X/Y (initially 100, 100).
2. **Transparency** — Pixels outside the capsule remain transparent; desktop content is visible around it.
3. **Z-order / taskbar** — The capsule is above ordinary windows and has no taskbar button.
4. **Drag (unlocked)** — Dragging the capsule moves it while unlocked. **This is the primary gate that CI could not prove.**
5. **Lock / unlock** — Right-click → **Lock Position**; dragging no longer moves it. **Unlock Position** and verify dragging works again.
6. **Snap / free + persistence** — Right-click → toggle **Snap to Taskbar**; the capsule centers vertically against the primary taskbar. Toggle **Free Position** and confirm prior free X/Y returns. Drag to a new free position, exit, relaunch, and confirm the position is restored.
7. **Settings** — Right-click → **Settings**; click **Toggle overlay accent** and confirm the capsule changes color live. Close the Settings window.
8. **Multi-monitor / DPI** — Move between monitors with different Windows scale factors. Confirm the capsule scales and stays visible; repeat with snap enabled and disabled.
9. **Exit** — Right-click → **Exit PoT**. The process exits and no overlay window remains.
10. **Relaunch** — Launch again works, showing class/window resources are released.

## Evidence to capture

- Screenshot of the capsule with desktop visible through transparent corners
- Windows version, display scale(s), compiler/`rustc` version
- Startup time (launch → first visible capsule) and process working set after launch (for ADR-001)
- Pass/fail note per step 1–10; failures with short description

Store notes under `docs/rust-port/evidence/` when updating Phase 1 status in `traceability.md` / `task-breakdown.md`.

## Historical notes (not acceptance criteria)

### Linux workspace attempt (2026-09-28)

- `cargo test --workspace --offline` and `cargo fmt` passed on Linux; no GUI result (Wine/Xvfb could not run the overlay).

### Former Windows CI smoke (removed 2026-10-13)

- `windows-2022` previously ran `overlay-smoke.exe`. Some runs confirmed layered HWND, glyph pixel, menu/Settings, and clean shutdown. Injected drag via `SendInput` repeatedly failed and burned runner minutes; the step was removed from `.github/workflows/rust.yml`. Keep those logs as historical context only—not as current Windows 11 acceptance.
