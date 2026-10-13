# Rust port status and use

> **상태 (2026-10-13): 진행 중이며 완료되지 않았습니다.** CI는 포맷·Clippy·테스트·MSVC 릴리스 빌드·포터블 패키지만 돌립니다. Win32 오버레이 드래그 등 GUI 수용은 Windows 11 로컬 체크리스트로 옮겼습니다. 상세 인수인계는 [`handoff-2026-09-29.md`](handoff-2026-09-29.md)를 참고하세요.

This fork keeps the upstream C# application as its source reference and develops a separate native Rust implementation. Work happens on `rust/phase-0`; `main` remains the upstream synchronization branch. The original project files and installation are not modified by the Rust application.

## Rust-only environment

- Rust stable toolchain and Cargo.
- Windows 11 and the Visual Studio C++ build tools for the `x86_64-pc-windows-msvc` target.
- .NET is not needed to build or run the Rust implementation.

Run from the repository root:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release --target x86_64-pc-windows-msvc
cargo run -p system-monitor-core --bin package_port -- target/x86_64-pc-windows-msvc/release/system-monitor-windows.exe rust-port-package
cargo run --release -p system-monitor-windows
```

The output directory contains the Rust executable, the MIT license, and a short portable-use note. No original executable or managed runtime is copied into it.

## CI policy (2026-10-13)

GitHub Actions `windows-2022` runs format, Clippy, unit tests, MSVC release build, and portable package upload only. The previous Win32 layered-window smoke step (including injected drag via `SendInput`) was removed: headless Server 2022 could not reliably deliver mouse input to a layered overlay, and repeated failures burned free runner minutes. GUI acceptance evidence must come from a real Windows 11 interactive session using [`evidence/p1-windows-run-checklist.md`](evidence/p1-windows-run-checklist.md).

Optional local smoke (not a CI gate):

```powershell
cargo build --workspace --release --target x86_64-pc-windows-msvc
.\target\x86_64-pc-windows-msvc\release\overlay-smoke.exe .\target\x86_64-pc-windows-msvc\release\system-monitor-windows.exe
```

## Current implementation checkpoint

Implemented: layered Win32 overlay, native context menu, startup mode and per-user startup registration, single-instance activation, taskbar snap/free placement, lock, position persistence, DPI scaling, settings controls for metric visibility/compact mode and OpenCode/Codex usage, DPAPI key storage, OpenCode Go polling, read-only Codex auth parsing/polling, DeepSeek balance client/parser, CPU/RAM/network sampling, disk capacity and activity sampling, NVIDIA/AMD/D3DKMT GPU sample paths, and packaging.

Still incomplete: full original settings/dashboard parity, GPU device name/LUID selection mapping beyond vendor+index, fullscreen fade/debounce and multi-monitor behavior, full theme/color/font system, DeepSeek balance UI wiring, Zen credit display, package/install upgrades, and broad Windows 11 multi-monitor/hardware validation. Catalog results must stay `NOT_RUN` or `BLOCKED` until their stated checks actually pass. The upstream executable is never built or run; runtime old-vs-new comparison therefore remains `RUNTIME_DIFF_NOT_RUN`.

See [`original-feature-catalog.md`](original-feature-catalog.md), [`implementation-plan.md`](implementation-plan.md), and [`function-task-breakdown.md`](function-task-breakdown.md) for requirements and verification scope.
