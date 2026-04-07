# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development commands

- Install dependencies: `pnpm install`
- Frontend-only dev server: `pnpm dev`
- Full desktop app dev (recommended): `pnpm tauri dev`
- Frontend build (type-check + Vite build): `pnpm build`
- Desktop debug build without bundling: `pnpm tauri build --debug --no-bundle`
- Run all frontend tests: `pnpm test`
- Run a single Vitest file: `pnpm test tests/useToolActions.test.ts`
- Run Rust unit tests in Tauri backend: `cargo test --manifest-path src-tauri/Cargo.toml`

## Current platform status

- Implemented and validated: macOS
- In progress: Windows, Linux

## High-level architecture

DevHub is a Tauri 2 desktop app with a Vue 3 frontend. The frontend renders UI state and dispatches actions; the Rust backend is the source of truth for tool detection/execution and emits progress/log events.

### Frontend flow (Vue + composables)

- App shell and module routing live in `src/App.vue`.
- Core state orchestration lives in `src/composables/useCliHubState.ts`:
  - Combines settings state (`useSettingsState`) and tool action state (`useToolActions`).
  - Subscribes to backend events on mount and cleans listeners on unmount.
- Tool lifecycle UX is implemented in `src/composables/useToolActions.ts`:
  - Calls Tauri commands via `invoke(...)` (refresh, install/update/uninstall, batch update, source checks, PATH fix/cleanup).
  - Receives backend events (`tool-progress`, `tool-updated`, `tool-log`, `tool-action-result`) and updates reactive tool state.
  - Handles Claude-specific install/uninstall options for PATH setup/cleanup.
- Task summary chips/popover are derived from tool statuses in `src/composables/useTaskProgress.ts`.
- Type contracts shared across UI modules are in `src/types/models.ts`; frontend bootstrap tool list is in `src/data/initial-data.ts`.

### Backend flow (Rust + Tauri commands)

- Command handlers and runtime logic are in `src-tauri/src/lib.rs`; `src-tauri/src/main.rs` only calls `devhub_lib::run()`.
- `AppState` stores in-memory tool state and settings; settings and logs are persisted under `~/.devhub/`.
- Backend responsibilities:
  - Detect installed CLI tools and versions (`get_tools_state`, `refresh_latest_versions`).
  - Run install/update/uninstall commands (`start_action`, `batch_update`) with streamed stdout/stderr.
  - Emit UI events for progress/log/result updates.
  - Manage Claude PATH fix/cleanup markers in shell rc files (`apply_path_fix`, `apply_path_cleanup`).
  - Preflight source availability checks (`check_sources`) and proxy env injection from persisted settings.

### Integration boundary

- Frontend never executes system install/update commands directly.
- All side-effectful operations are delegated to backend Tauri commands.
- UI state is eventually consistent with backend state through command responses + emitted events.

## Important project-specific behavior

- `tauri.conf.json` runs `pnpm dev` before `tauri dev`, and `pnpm build` before `tauri build`; frontend and desktop workflows are intentionally coupled.
- Claude install path handling is special-case logic: install uses official script fetch/validation (`https://claude.ai/install.sh`) and optional PATH marker writes (`# devhub`) for reversible cleanup.
- Log persistence and retention are runtime-configurable via settings and implemented in backend log pruning.