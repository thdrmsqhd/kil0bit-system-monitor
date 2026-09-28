# Phase 1 PoT: Windows run checklist

This PoT exercises only the Rust binary. It does not build or launch the original application and does not call .NET.

## Host requirements

- Windows 11 x64
- Rust stable toolchain from `rust-toolchain.toml`
- Visual Studio 2022 C++ Build Tools (MSVC linker)

## Commands

Run from the Rust workspace root:

```powershell
rustc --version
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --release -p system-monitor-windows
```

## Expected result

1. A borderless dark blue-gray capsule appears near horizontal coordinate 100 and is vertically centered on the primary taskbar by default, with white `RUST` bitmap text. Choosing **Free Position** starts from the saved free X/Y (initially 100, 100).
2. Pixels outside the capsule remain transparent; desktop content is visible around it.
3. The capsule is above ordinary windows and has no taskbar button.
4. Dragging the capsule moves it while unlocked.
5. Right-click and choose **Lock Position**; dragging no longer moves it. Choose **Unlock Position** and verify dragging works again.
6. Right-click and toggle **Snap to Taskbar**; the capsule should center vertically against the primary taskbar. Toggle **Free Position** and confirm its prior free X/Y returns. Drag to a new free position, exit, relaunch, and confirm the position is restored.
7. Right-click → **Settings**; click **Toggle overlay accent** and confirm the capsule changes color live. Close the Settings window.
8. Move between monitors with different Windows scale factors. Confirm the capsule scales and stays visible; repeat with snap enabled and disabled.
9. Right-click and choose **Exit PoT**. The process exits and no overlay window remains.
10. Repeating the launch works again, showing class/window resources are released.

Capture evidence showing the capsule and desktop visible around its transparent corners. Record Windows version, display scale, compiler version, and any build/runtime error. The Linux checks validate the software-generated premultiplied BGRA surface and Win32 FFI type-checking only. Windows CI on Server 2022 already proves visible layered HWND creation, menu/Settings opening, and clean shutdown; Windows 11-specific taskbar, DPI, drag/lock, screenshot, and resource checks remain open. Record startup time from launch to first visible capsule and process working set after launch for the renderer/UI-toolkit comparison in ADR-001.
# Automated verification attempt (2026-09-28)

This section records the verification run performed on the Linux workspace host. No .NET command, runtime, or original executable was used.

- Rust stable 1.98.1: `cargo test --workspace --offline` passed (3 bitmap/scale unit tests); `cargo fmt --all -- --check` passed.
- Windows MSVC target: workspace `cargo check` with warnings denied passed. This is target-aware Rust checking, not a native MSVC link or Windows run.
- Windows GNU target: release build linked with MinGW. The output is a PE32+ x86-64 Windows GUI executable with the expected USER32/GDI32/SHELL32/KERNEL32 imports.
- GUI execution: attempted with Wine 9 under Xvfb. Wine stopped before launching the app because `wineserver` could not create/use its socket (`Operation not permitted`) in the managed container. No overlay screenshot or interaction result is claimed.
- The local Linux session therefore produced no GUI runtime result. A Windows-hosted CI smoke harness later passed layered HWND creation, native context menu and Settings opening, and clean shutdown. The current harness adds a desktop-composited glyph-pixel assertion; its first successful run is still pending. The steps below record the remaining interaction and visual acceptance checks that the smoke harness does not cover.

## Automated Windows runner smoke test

The feature branch now has a Rust-only GitHub Actions job on `windows-2022`. It runs format, Clippy, tests, MSVC release build, then launches the overlay PoT with `overlay-smoke.exe`. Run `36402994870` confirmed a visible `WS_EX_LAYERED` HWND with non-zero bounds, clicked Settings in the native context menu, confirmed the Settings HWND was visible, and observed clean process shutdown. A screen glyph-pixel check is being added. Taskbar positioning, the Settings button's visual effect, multi-monitor/DPI behavior, and the full visual appearance remain separate acceptance checks.
