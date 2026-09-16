# Repository guidance

## Commands and verification
- Run npm commands from this directory; use `npm ci` with `package-lock.json`. Locked Vitest 5 requires Node `^22.12.0 || ^24.0.0 || >=26.0.0` (stricter than Vite's requirement).
- `npm run app` is the Windows desktop shortcut: it adds Rustup's Cargo directory to `PATH`, launches Tauri, and starts Vite automatically. `npm run dev` is frontend-only and cannot exercise IPC.
- Vite uses port **1420** with `strictPort: true`, matching `src-tauri/tauri.conf.json`. Do not start a second Vite server before `tauri dev`; keep both configs aligned if changing the port.
- `npm run build` runs TypeScript checking followed by the frontend build into `dist/`; it does not compile Rust. `npx tsc --noEmit` is the focused frontend typecheck.
- `npm run tauri build` runs the frontend build automatically, then builds/bundles the native app. Rust-only checking from the root: `cargo check --manifest-path src-tauri/Cargo.toml`.
- Vitest tests run with `npx vitest run`; Rust tests run with `cargo test --manifest-path src-tauri/Cargo.toml`. There are no configured lint/formatter scripts or CI checks; do not assume `npm test` or `npm run lint` exists.

## Wiring and constraints
- Rust is the timer authority; React polls snapshots through `src/flowmodoro.ts` and must not increment elapsed time itself. Domain/service code is under `src-tauri/src/`; see the approved spec and plan before changing time or recovery behavior.
- Native setup lives in `src-tauri/src/lib.rs`; `src-tauri/src/main.rs` only calls `workbreak_98_lib::run()`. Register new callable Rust commands in `tauri::generate_handler![...]` as well as marking them `#[tauri::command]`.
- Keep the `workbreak_98_lib` library name distinct from the `workbreak-98` binary name (Windows linking constraint documented in `Cargo.toml`), and preserve the release-only `windows_subsystem` attribute in `main.rs`.
- The window has `decorations: false`; the 98.css title bar provides drag/minimize/close behavior. Keep `src-tauri/capabilities/default.json` aligned with those window APIs.
- Vite CSS minification is disabled because 98.css 0.1.21 contains a legacy media query rejected by Vite 8; do not remove that workaround without proving `npm run build`.
- The opener plugin is initialized in `lib.rs`; main-window permissions live in `src-tauri/capabilities/default.json`. Check both native registration and capability permissions when adding plugin APIs.
- `dist/`, `src-tauri/target/`, and `src-tauri/gen/schemas/` are generated outputs. Edit Tauri config/capabilities rather than generated schemas.
