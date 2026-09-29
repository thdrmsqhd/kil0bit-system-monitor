# Rust port status and use

> **상태 (2026-09-29): 진행 중이며 완료되지 않았습니다.** 최신 Windows CI의 포맷·Clippy·테스트·릴리스 빌드·패키지 생성은 통과했지만, Win32 드래그 smoke는 실패했습니다. 상세 인수인계는 [`handoff-2026-09-29.md`](handoff-2026-09-29.md)를 참고하세요.

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
```

The output directory contains the Rust executable, the MIT license, and a short portable-use note. No original executable or managed runtime is copied into it.

## Current implementation checkpoint

Implemented: layered Win32 overlay, native context menu, startup mode and per-user startup registration, single-instance activation, taskbar snap/free placement, lock, position persistence, DPI scaling, settings controls for metric visibility/compact mode and OpenCode/Codex usage, DPAPI key storage, OpenCode Go polling, read-only Codex auth parsing/polling, DeepSeek balance client/parser, CPU/RAM/network sampling, disk capacity sampling, and a Windows hosted smoke harness.

Still incomplete: full original settings/dashboard parity, per-drive activity and selection, GPU vendor providers, fullscreen fade/debounce and multi-monitor behavior (current hide check only compares the foreground window to primary-screen bounds), full theme/color/font system, DeepSeek balance UI wiring, Zen credit display, package/install upgrades and broad Windows 11 multi-monitor/hardware validation. Catalog results must stay `NOT_RUN` or `BLOCKED` until their stated checks actually pass. The upstream executable is never built or run; runtime old-vs-new comparison therefore remains `RUNTIME_DIFF_NOT_RUN`.

See [`original-feature-catalog.md`](original-feature-catalog.md), [`implementation-plan.md`](implementation-plan.md), and [`function-task-breakdown.md`](function-task-breakdown.md) for requirements and verification scope.
