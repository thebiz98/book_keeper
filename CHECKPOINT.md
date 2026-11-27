# Book Keeper — Checkpoint & Resumption Guide

*Last updated: 2026-11-28.* Design decisions live in `docs/DESIGN_DECISIONS.md` (authoritative) and the HLD / LLD / UI LLD. Feature history is in `git log` (merge commits describe each branch). Test layout is in `TEST_INFRA.md`.

## State
- `main` holds everything delivered through v1.9: sales history totals, the meaningful Adjustments list, the service stock status, backup hardening and Restore from backup (FR-60, DD-99), and the dev launcher closing its console.
- Schema **v12**.
- Gate: `cargo test` (675 tests), `npx vitest run` (555 tests), `npx tsc --noEmit`, `cargo clippy --all-targets`.

| Layer | Location |
|---|---|
| Domain, DTOs, contracts | `src-tauri/src/core`, `src-tauri/src/{data,services}/contracts` |
| Services | `src-tauri/src/services/impl` |
| SQLite (pool, migrations, unit of work, FTS5, repos) | `src-tauri/src/data/sqlite` |
| Bootstrap | `app_state::build_sqlite_app_state*`; `main.rs` applies a pending restore first |
| IPC | `src-tauri/src/api`, `src-tauri/src/main.rs` |
| Frontend | `src/`, with one IPC boundary in `src/api/ipc.ts` (`invokeCmd`, which converts snake_case ↔ camelCase) |

## Conventions that matter
- Repositories follow `src-tauri/src/data/sqlite/mod.rs`: table/column naming, enums stored as TEXT, explicit id inserts.
- Never call `uow.begin_transaction()` while a transaction is open (`BEGIN IMMEDIATE` deadlocks until busy_timeout). Use the open `tx`.
- The search indexer writes through its own connection, so index only **after** commit.
- Frontend types are camelCase only. Rust DTOs stay snake_case; conversion happens only in `invokeCmd`.
- Business rules and money/stock values come from the backend; previews are backend dry runs (DD-56).
- Rust tests create temp files through `tests/common` (`TempDir`, `temp_file`). A sweep removes `%TEMP%/bk_*` leftovers older than 2 hours.

## Environment
- **Windows 11 ARM64.**
  - Requires VS Build Tools 2022 with the **ARM64** component (`Microsoft.VisualStudio.Component.VC.Tools.ARM64`).
  - Run cargo from PowerShell/cmd, not Git Bash, because Git's `/usr/bin/link` shadows MSVC `link.exe`.
  - All cargo builds use `C:\custom_apps\book_keeper\target`.
- **Running the app.**
  - The desktop shortcut runs `scripts\dev-launch.cmd`, which runs `dev-launch.ps1` → `npx tauri dev`.
  - The console closes when the app closes. A build error keeps it open.
  - Only what is checked out in this folder runs.
- **Data.**
  - The database is `%APPDATA%\com.bookkeeper.app\shop.db`.
  - Backups are in `%APPDATA%\com.bookkeeper.app\backups` (auto every 24 h and on close, kept 7 days) or in the folder chosen in Settings.
  - Restore from backup closes the app; the swap happens on the next start.
- **Termux (aarch64).**
  - cargo and node need the sandbox disabled.
  - A desktop `tauri build` can't run there.
