# Book Keeper — Checkpoint & Resumption Guide

*Last updated: 2026-11-21.* Status per milestone lives in `PROJECT.md` (the single tracker). Test layout and gate are in `TEST_INFRA.md`.

## State
All milestones M1–M6 are done. Gate (`bash run_e2e_tests.sh`): 54 backend tests on real SQLite, clippy clean, `tsc` clean, 101 Vitest tests, `vite build` succeeds.

| Layer | Location |
|---|---|
| Domain, DTOs, contracts | `src-tauri/src/core`, `src-tauri/src/{data,services}/contracts` |
| Services | `src-tauri/src/services/impl` |
| SQLite (pool, schema v1, unit of work, FTS5, 11 repos) | `src-tauri/src/data/sqlite` |
| Bootstrap | `app_state::build_sqlite_app_state(db_path)` |
| IPC (46 commands) | `src-tauri/src/api`, `src-tauri/src/main.rs` |
| Frontend | `src/` — one IPC boundary in `src/api/ipc.ts` (`invokeCmd`, converts snake_case ↔ camelCase) |

## Conventions that matter
- Repositories follow `src-tauri/src/data/sqlite/mod.rs`: table/column naming, enums stored as TEXT, explicit id inserts.
- Never call `uow.begin_transaction()` while a transaction is open (`BEGIN IMMEDIATE` deadlocks until busy_timeout). Use the open `tx`.
- The search indexer writes through its own connection, so index only **after** commit.
- Frontend types are camelCase only. Rust DTOs stay snake_case; conversion happens only in `invokeCmd`.

## Environment
Termux (aarch64): cargo and node need the sandbox disabled. In the sandbox `find`/`grep` fail; use `rg`. A full `tauri build` (window bundle) can't run on Termux because the webview libs are missing; build the desktop bundle on a desktop OS.

Windows 11 ARM64 (verified 2026-11-22): requires VS Build Tools 2022 with the **ARM64** component (`Microsoft.VisualStudio.Component.VC.Tools.ARM64`); x64-only tools fail with `linker link.exe not found`. Run cargo from PowerShell/cmd, not Git Bash, because Git's `/usr/bin/link` shadows MSVC `link.exe`. Dev run without installing: `scripts\dev-launch.cmd` (`npx tauri dev`); DB lives at `%APPDATA%\com.bookkeeper.app\shop.db`.

## Next (not started)
- Package and run with `tauri build` on a desktop machine, then do a manual UI pass in Tauri mode (so far only the mock client and IPC contracts have been checked).
- Optional: tighten the remaining `as any` tab-ID casts in ReportsPage and ReturnsAdjustmentsPage.
