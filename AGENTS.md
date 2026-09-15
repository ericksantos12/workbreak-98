# Repository guidance

## Commands and verification
- Run npm commands from this directory; use `npm ci` with `package-lock.json`. Locked Vitest 5 requires Node `^22.12.0 || ^24.0.0 || >=26.0.0` (stricter than Vite's requirement).
- `npm run tauri dev` launches the desktop app and starts Vite automatically. `npm run dev` is frontend-only; the greeting's `invoke("greet", ...)` needs the Tauri runtime.
- Vite uses port **1420** with `strictPort: true`, matching `src-tauri/tauri.conf.json`. Do not start a second Vite server before `tauri dev`; keep both configs aligned if changing the port.
- `npm run build` runs TypeScript checking followed by the frontend build into `dist/`; it does not compile Rust. `npx tsc --noEmit` is the focused frontend typecheck.
- `npm run tauri build` runs the frontend build automatically, then builds/bundles the native app. Rust-only checking from the root: `cargo check --manifest-path src-tauri/Cargo.toml`.
- Vitest is installed, but there are currently no tests, test script, or dedicated test config. There are also no configured lint/formatter scripts or CI checks; do not assume `npm test` or `npm run lint` exists.

## Wiring and constraints
- This is still the React/Tauri starter: `src/main.tsx` mounts `src/App.tsx` in StrictMode; no Flowmodoro timer logic exists yet. `98.css` is installed but not imported; current styling comes from `src/App.css`.
- Native setup lives in `src-tauri/src/lib.rs`; `src-tauri/src/main.rs` only calls `flowmodoro_lib::run()`. Register new callable Rust commands in `tauri::generate_handler![...]` as well as marking them `#[tauri::command]`.
- Keep the `flowmodoro_lib` library name distinct from the binary name (Windows linking constraint documented in `Cargo.toml`), and preserve the release-only `windows_subsystem` attribute in `main.rs`.
- The opener plugin is initialized in `lib.rs`; main-window permissions live in `src-tauri/capabilities/default.json`. Check both native registration and capability permissions when adding plugin APIs.
- `dist/`, `src-tauri/target/`, and `src-tauri/gen/schemas/` are generated outputs. Edit Tauri config/capabilities rather than generated schemas.
