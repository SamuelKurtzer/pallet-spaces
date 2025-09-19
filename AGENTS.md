# Repository Guidelines

## Project Structure & Modules
- `backend/src/`: Rust API and views (Axum + Maud).
  - `controller/`: route wiring via `RouteProvider` and `Routes` traits.
  - `model/`: database integration (SQLx `Database`, auth backend).
  - `plugins/`: feature modules (`users.rs`, `posts.rs`) with model/control/view sections.
  - `views/`: HTML rendering helpers and pages.
- `frontend/public/`: static assets served at `/public`.
- `test.db`: local SQLite (gitignored). Delete to reset dev data.

## Build, Test, Run
- Build: `cargo build --bin backend` — compiles the server.
- Run: `RUST_LOG=info cargo run --bin backend` — starts on `http://127.0.0.1:37373`.
- Test: `cargo test` — runs Rust unit/integration tests.
- Optional dev shell (Nix): `nix develop` — provides Rust toolchain, SQLx deps.

## Coding Style & Naming
- Language: Rust 2024 edition; 4‑space indent.
- Naming: types `CamelCase`; functions/modules `snake_case`.
- Formatting: `cargo fmt` before commits.
- Linting: `cargo clippy --all-targets -- -D warnings` (keep warning‑free builds).
- Organization: add new features under `backend/src/plugins/<feature>.rs`; expose routes via `RouteProvider` and register with `Router.add_routes::<Feature>()`.

## Testing Guidelines
- Framework: Rust `#[tokio::test]` with Axum `ServiceExt` for route tests.
- Location: colocate tests in the module using `#[cfg(test)]` blocks (see `backend/src/main.rs`).
- Scope: cover route status codes, happy/edge paths, and DB interactions with an isolated `test.db`.
- Run: `cargo test` locally; ensure deterministic tests (no reliance on existing data).

## Commits & Pull Requests
- Commits: concise, imperative subjects (e.g., "add posts index filter"); group related changes.
- PRs: include description, rationale, and testing instructions. Link issues. Add screenshots for UI/HTML changes.
- Checklist: build passes; `cargo fmt` + `clippy` clean; tests updated/added; note schema changes (if `initialise_table` altered).

## Security & Configuration
- Secrets: none required for dev; SQLite file is local and ignored by Git.
- Logs: tracing enabled; avoid logging passwords/PII.
- Data resets: remove `test.db` to reinitialize tables on next run.

## Styling Guide
- Approach: CUBE CSS with BEM-like modifiers.
  - Tokens: CSS variables in `/frontend/public/css/main.css` under `:root` (`--color-*`, `--space-*`, `--radius-*`, `--shadow-*`, `--fs-*`).
  - Layers: reset → base → layout utilities → components.
- Key utilities:
  - `.container` (max-width + padding), `.stack` (vertical rhythm), `.cluster` (row with wrap+gap), `.grid`, `.grid--2` (1–2 cols responsive), `.list` (vertical list).
  - Helpers: `.mt-*`, `.text-muted`, `.visually-hidden`.
- Components:
  - `.nav`, `.card`, `.btn` (+ `--primary`, `--secondary`, `--success`, `--ghost`, `--danger`).
  - Forms: `.form`, `.field`, `.label`, `.input`, `.select`, `.textarea`, `.help`, `.error`.
  - Badges: `.badge` (+ `--hidden`, `--muted`).
- Accessibility:
  - `:focus-visible` outlines for links, buttons, and inputs; ensure contrast via tokens.
  - Prefer semantic HTML in views; keep actions as `<button>` inside forms for POSTs.
- Templating conventions (Maud):
  - No inline styles. Use classes and tokens.
  - Wrap body with `body.page` and page content in `.container`.
  - For grids/forms, compose utilities over bespoke styles.
- Adding new UI:
  1) If it’s a one-off layout need, prefer a utility.
  2) If it’s a reusable block, add/extend a component class.
  3) Use modifiers (`.btn--secondary`) rather than editing base component rules.
- Local dev checklist:
  - Add/modify HTML to use the component/utility classes.
  - Keep tests passing (`cargo test`) — most assertions are content-based and not style-sensitive.
  - Optional: run a11y checks manually (focus order, keyboard navigation).
