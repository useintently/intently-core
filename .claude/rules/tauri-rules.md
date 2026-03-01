# Tauri v2 Rules

Rules governing the Tauri desktop shell, IPC commands, and native integrations.

## Architecture
- Tauri v2 with Rust backend and React frontend
- Backend code lives in `apps/desktop/src-tauri/src/`
- Commands defined in `apps/desktop/src-tauri/src/commands/` — one file per domain
- State managed via `tauri::State<>` with `Mutex` or `RwLock` for shared state
- App configuration in `apps/desktop/src-tauri/tauri.conf.json`

## Commands (IPC)
- All commands MUST return `Result<T, String>` or a typed error serializable to the frontend
- Use `#[tauri::command]` attribute on all command functions
- Commands are registered in `main.rs` via `tauri::Builder::invoke_handler`
- NEVER execute arbitrary shell commands from user input
- Validate and sanitize all inputs on the Rust side before processing
- Use `async` commands for I/O operations to avoid blocking the main thread

## Security
- NEVER embed secrets, API keys, or credentials in frontend code
- Filesystem access: scoped to the opened repository via allowlist
- Shell plugin: restricted to predefined commands only
- CSP (Content Security Policy): configured in `tauri.conf.json`, no `unsafe-eval`
- IPC: treat all frontend input as untrusted — validate on the Rust side

## Permissions & Plugins
- `fs` plugin: read/write scoped to project directory and app data
- `shell` plugin: restricted — only predefined binaries (cargo, git, just)
- `process` plugin: for spawning and managing child processes (workers)
- `dialog` plugin: native file/folder dialogs for project open/save
- `updater` plugin: auto-update mechanism for releases
- NEVER add plugins without evaluating their permission scope

## Events
- Use Tauri event system for Rust-to-frontend communication
- Event names: snake_case, prefixed by domain: `core:twin_updated`, `policy:report_ready`
- Payload: always JSON-serializable structs
- Frontend listens via `listen()` from `@tauri-apps/api/event`
- Unlisten on component unmount to prevent memory leaks

## Window Management
- Main window configuration in `tauri.conf.json`
- Support multi-window for diff views and reports
- Window state (size, position) persisted via app data directory
- Use `WebviewWindow` API for programmatic window creation

## File System
- Use `tauri::api::path` for resolving platform-specific directories
- App data: `app_data_dir()` for settings, caches, logs
- Project files: accessed via scoped fs plugin, never absolute paths from frontend
- Temp files: use `temp_dir()`, clean up on session end

## Error Handling
- Rust command errors: map domain errors to serializable error types
- Frontend: wrap all `invoke()` calls in try/catch with user-facing error messages
- Log errors on the Rust side via `tracing` before returning to frontend
- NEVER expose internal error details (stack traces, paths) to the user

## Development
- `cargo tauri dev` for development with hot-reload
- `cargo tauri build` for production builds
- Test commands independently with unit tests before wiring to frontend
- Use `tauri::test` utilities for integration testing of commands
