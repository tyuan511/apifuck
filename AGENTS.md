# Repository Guidelines

## Source of Truth and Scope

ApiFuck is a native Rust desktop application. Treat the current on-disk code as the source of truth and preserve existing user changes. Do not restore the retired Tauri/React application or introduce a frontend build pipeline based on historical release notes.

Read [README.md](README.md) for the product, [docs/architecture.md](docs/architecture.md) for module boundaries, and the focused documents linked there before changing related behavior. Keep documentation in sync with shipped code; distinguish implemented core APIs, desktop features, and planned work. Native MCP integration is planned; no standalone CLI product is maintained.

## Workspace and Responsibilities

The root Cargo workspace contains:

- `crates/core/`: OpenCollection loading/validation, comment-preserving YAML edits, atomic storage, environment and URL resolution, HTTP/GraphQL/SSE/WebSocket execution, QuickJS scripts, tests, Bru/cURL conversion, code generation, collection runner, settings and updates.
- `crates/desktop/`: GPUI Kit UI. `workbench.rs` coordinates projects, navigation, tabs, drafts and settings; `document_tab.rs` owns document editing and execution state; forms, tables, variable inputs, response rendering, updates and editor services live in dedicated modules.
- `crates/demo/`: loopback Playground API, workspace preparation and integration tests. The `launch` example forwards `cargo demo` to `scripts/demo.mjs`.

Shared assets are in `assets/`, schema snapshots in `schema/`, examples in `examples/`, and embedded converters/runtime bundles in `vendor/bruno/`. `scripts/release/` implements versioning, release validation and artifact publication. See [docs/architecture.md](docs/architecture.md) for the module map.

## Toolchain and Commands

Run commands from the repository root. `rust-toolchain.toml` selects Rust 1.95; GPUI Kit is pinned in `crates/desktop/Cargo.toml`. Commit `Cargo.lock` and use `--locked` for verification.

Node.js 22+ is needed for Demo and release scripts, not ordinary Cargo builds or the running application. Bun is only needed to install/rebuild the vendored JavaScript dependencies. Do not rebuild bundles for unrelated UI or documentation work.

Verification commands:

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
node --test scripts/release/*.test.mjs
cargo demo --check
```

Use focused tests during implementation and workspace checks before submitting code changes. Run Node checks when modifying release/Demo scripts. For documentation-only work, verify commands, source references and local links; do not claim runtime behavior was tested merely by checking documentation. CI configuration is in `.github/workflows/ci.yml` and covers macOS, Linux and Windows plus script and headless Demo tests.

Manual development commands, not agent verification commands:

- `cargo demo`: launch the local API and desktop with `.demo-workspace/`.
- `cargo demo --headless`: prepare the workspace and run only the local API.
- `cargo run --locked -p apifuck-desktop -- <project-directory>`: open a project.

Linux prerequisites and Demo flags are documented in [docs/development.md](docs/development.md).

## Verification Policy: No Computer Use

Computer Use is strictly prohibited for verification.

- Never use desktop GUI automation, screenshots, screen coordinates, accessibility APIs, AppleScript UI scripting, simulated mouse/keyboard input, or manually launch applications to verify results.
- Use automated tests first, then type checking, formatting, linting, builds and static analysis, then CLI commands, scripts, HTTP requests, logs and structured APIs.
- Browser automation such as Playwright is allowed for web interfaces only when it does not rely on desktop-level Computer Use. It does not validate this native GPUI interface.
- If non-GUI methods cannot verify a requirement, report what was checked, what could not be checked, why, and the exact manual verification still needed. Never claim visual or interactive verification without an allowed method that actually verified it.

`cargo demo --check` does not open the desktop. Keep manual UI acceptance instructions separate from automated test results; see [docs/postman-layout.md](docs/postman-layout.md).

## UI and Editor Conventions

Use GPUI Kit components, shared surfaces and theme colors. Palette literals belong in `crates/desktop/src/appearance.rs`; shared icons and embedded resources belong in `icons.rs` and `brand.rs`. Use `feedback.rs` for consistent in-app operation notifications. Product copy uses `i18n::tr` with a matching English entry in `crates/desktop/locales/en.json`. Do not translate user data, protocol names, code or server output.

Keep application preferences in the Settings page and collection configuration in the collection sidebar action. The project menu contains open/recent/new/import actions; it has no project-settings or project-delete entries. Editing an environment must not silently activate it.

Reuse `key_value_editor.rs` and `request_form.rs` for structured inputs. Variable badge/tooltip/completion behavior belongs in `variable_input.rs`; completion panels must float outside normal layout so they do not change table row heights. Display-only changes must preserve source text, focus, selection and undo semantics. Static variable previews must use core resolution and must not execute scripts.

Code editors use `surfaces.rs`, `languages.rs`, `editor_tools.rs`, `language_server.rs` and `script_completion.rs`. Keep syntax diagnostics distinct from schema/type checking; discard stale background formatting and diagnostic results. Application editor services must work offline without a Node runtime.

## Storage and Execution Contracts

OpenCollection YAML is the shared project format; see [docs/storage.md](docs/storage.md) and [docs/execution.md](docs/execution.md). Keep `.apifuck/` state out of shared files. Display names are metadata, while generated paths provide file identity; editing a name must not rename the file implicitly.

Preserve comments and formatting outside the edited YAML nodes. Retain schema validation, same-directory atomic writes and external-change detection. Auto-saving a raw draft is different from saving a valid shared document: incomplete input, disabled/empty rows and mode-specific drafts must remain recoverable. Do not overwrite newer external edits with clean snapshots.

Keep variable precedence, URL/Query synchronization, auth/header inheritance, scripts and request execution in `crates/core/`. Desktop sending, code exports, tests and future integrations must reuse these rules. Script-written environment values belong in `.apifuck/variables.json`; retain locking for concurrent writes. Never treat an importable or editable protocol/auth configuration as supported execution unless the core implements it.

## Releases and Documentation

The version is `[workspace.package].version` in root `Cargo.toml`; members inherit it and `Cargo.lock` must agree. Use the existing scripts under `scripts/release/`. A release requires a matching `v<semver>` tag and non-empty `docs/release/<version>.md`; publication waits for all three platform builds and artifact verification. Do not replace an already public release. Follow [docs/auto-update.md](docs/auto-update.md).

Preserve the `release-downloads` marker pair in README. The download-link updater is a separate helper, not a step in the current release workflow. Historical files in `docs/release/` describe their original versions; do not rewrite them as current architecture documentation.

Use Conventional Commit prefixes such as `feat:`, `fix:`, `refactor:` and `docs:`. Keep changes focused, avoid unrelated cleanup, and describe the user-visible result, verification performed and remaining limits.
