<p align="center">
  <img src="assets/apifuck.png" width="96" height="96" alt="ApiFuck Logo" />
</p>

# ApiFuck

[![CI](https://github.com/tyuan511/apifuck/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/tyuan511/apifuck/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/tyuan511/apifuck?label=release)](https://github.com/tyuan511/apifuck/releases/latest)
[![Rust](https://img.shields.io/badge/Rust-1.95-DEA584?logo=rust)](rust-toolchain.toml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**English** | [简体中文](README_zh-CN.md)

A native desktop API workbench built with Rust and GPUI Kit. Keep requests, environments, and folder configuration in Git-friendly OpenCollection YAML, with drafts and device settings stored locally. No account required.

The workspace has three Rust crates: `desktop` for the native interface, `core` for storage and request execution, and `demo` for a local Playground. Assets, Bruno converters, and script dependencies are embedded in the application; running a downloaded build requires neither Node.js nor Bun.

[Download](#download) · [Quick start](#quick-start) · [Features](#features) · [Your data](#your-data) · [Development and documentation](#development-and-documentation)

## Download

Native builds are distributed through [GitHub Releases](https://github.com/tyuan511/apifuck/releases) for macOS Apple Silicon, Windows x86_64, and Linux x86_64.

<!-- release-downloads:start -->
| Platform | Latest stable build |
| --- | --- |
| macOS Apple Silicon | [Download](https://github.com/tyuan511/apifuck/releases/latest) |
| Windows x86_64 | [Download EXE](https://github.com/tyuan511/apifuck/releases/latest) |
| Linux x86_64 | [Download AppImage](https://github.com/tyuan511/apifuck/releases/latest) |

These links resolve to matching assets in the latest stable release. If native assets are not yet available there, run from source below. Older versions are listed in [Releases](https://github.com/tyuan511/apifuck/releases).
<!-- release-downloads:end -->

On macOS, open the DMG and drag `ApiFuck.app` into Applications before launching it. If you use v0.1.0 or v0.1.1, download v0.1.2 manually once: those clients only recognize the legacy filenames. On Windows, run the `.exe`. On Linux, make the AppImage executable first (replace the example below with the actual downloaded filename; older releases use legacy names):

```sh
chmod +x ApiFuck-0.1.2.AppImage
./ApiFuck-0.1.2.AppImage
```

Releases provide a signed and notarized macOS DMG, a Windows EXE and a Linux AppImage. Automatic updates use those same files; no separate macOS ZIP or update archive is published. Intel Mac and Linux ARM64 assets are not currently produced. See the [development guide](docs/development.md) for Linux system dependencies.

The workspace version lives in [Cargo.toml](Cargo.toml); source code in this repository does not imply a published release. [Historical release notes](docs/release) describe their original versions, whose architecture and packaging may differ from the current native application.

## Quick start

Running from source requires **Rust 1.95**. The Demo launcher also requires **Node.js 22+**. Cargo uses the root `rust-toolchain.toml`; Linux prerequisites are in the [development guide](docs/development.md).

```sh
git clone https://github.com/tyuan511/apifuck.git
cd apifuck

# Start the local API, prepare an editable project, and open the desktop app
cargo demo

# Run loopback integration tests without opening the desktop app
cargo demo --check

# Open an existing OpenCollection project directly
cargo run --locked -p apifuck-desktop -- examples/demo
```

The [Playground](examples/playground/README.md) includes 32 ready-to-send requests covering authentication, CRUD, uploads, GraphQL, SSE, WebSocket, and expected errors. After launching the Demo, send the health-check request (`服务健康检查`): expect **200** and passing assertions in Tests.

`cargo demo` preserves your edits in `.demo-workspace/`. Use `--fresh` to back up and recreate it, or `--headless` to start only the local API and prepare the workspace.

## Features

| Area | Available today |
| --- | --- |
| Projects and navigation | Create/open projects, 10 recent projects, collection tree, separate environment sidebar, search, and synchronized tab/navigation selection |
| Documents | Create HTTP, GraphQL, and WebSocket requests; create folders and environments by display name; duplicate, move, rename, trash, and reorder documents |
| Workspace | Reorder tabs by dragging, close tabs in bulk, browse long titles, and switch between vertical and horizontal request/response splits |
| Request editing | Query/Path, Headers, Vars, and Assert tables; authentication forms; multiple body modes; GraphQL Query/Variables; Script, Tests, Docs, and YAML Source editors |
| Variables | Environment inheritance, project/folder/request scopes, Base URL, nested templates, value tooltips, badges, and floating completion |
| Editor assistance | Syntax highlighting, background formatting, built-in LSP syntax diagnostics, and offline script API completion and hover help |
| Protocols and responses | HTTP, GraphQL, incremental SSE with cancellation, WebSocket messaging; status, timing, size, headers, body, timeline, and test results |
| Scripts and tests | Synchronous QuickJS hooks, Chai, AJV JSON Schema, declarative assertions, response variable extraction, and local environment writeback |
| Import and export | Import local OpenAPI 2.0/3.0/3.1 and Postman 2.0/2.1 collections with a preview; paste cURL into the URL bar; convert Bru requests, environments, and folders; generate and copy cURL, JavaScript, Python, and Go code |
| Collection runner | Run a collection or folder with CSV/JSON data, environment, iteration, delay, cancellation and per-request results; export JSON, JUnit and HTML reports |
| Local session data | Project request history with bounded response previews; environment-isolated Cookie jar with inspection and clearing |
| OAuth 2.0 | Client credentials and authorization code + PKCE, local token caching and refresh |
| Desktop preferences | Recover raw drafts; English and Simplified Chinese; light/dark/system themes; six accent colors; network/proxy settings, custom CA and PEM client certificates; in-app updates |

Use the project menu to open or create projects, access recent projects, import `.bru` files, or import an OpenAPI/Postman collection into a new project. Collection variables and authentication belong in the settings button beside the collection sidebar title. Application preferences live in the Settings page; the project menu has no project-settings or project-delete actions.

Opening an environment edits its document; the top-right selector determines which environment requests use. Variable previews reflect static configuration and do not run pre-request scripts.

## Your data

```text
project/
├── opencollection.yml       # Collection metadata and project defaults
├── environments/local.yml  # Environment variables and inheritance
├── users/
│   ├── folder.yml           # Folder defaults
│   └── get-users.yml        # Request document
└── .apifuck/                # Local drafts, variable overrides, and trashed documents
```

Edits automatically save as local drafts, including incomplete or invalid input. Explicitly saving writes validated shared YAML. Drafts survive closing tabs, switching editor modes, and restarting. Shared saves preserve comments and formatting outside edited nodes and reject conflicting external changes.

> [!NOTE]
> Commit shared YAML to Git and ignore `.apifuck/` in your project's `.gitignore`. New projects add this ignore rule automatically; check it yourself for existing projects.

Language, theme, network preferences, and recent projects live in `~/.apifuck-native/`. Legacy JSON projects have no automatic migration layer. See [storage](docs/storage.md) for file identity and persistence rules.

## Keyboard shortcuts

Use Cmd on macOS and Ctrl on other platforms.

| Action | Shortcut |
| --- | --- |
| Open project | Cmd/Ctrl+O |
| New request | Cmd/Ctrl+N |
| Save current document or settings | Cmd/Ctrl+S |
| Send / stop request | Cmd/Ctrl+Enter |
| Close current tab / leave Settings | Cmd/Ctrl+W |
| Quit | Cmd/Ctrl+Q |
| Trigger code editor completion | Ctrl+Space |

## MCP and AI agents

Open **Settings → MCP** to enable the native MCP server and copy setup instructions for **Claude Code, Codex, OpenCode, or Pi**. The service is off by default, listens only on loopback, and uses token authentication. When enabled, it starts with the application; keep the desktop running while using it.

Agents can manage OpenCollection documents and environments, execute HTTP/GraphQL/SSE/WebSocket requests, cancel jobs, run collections, and import/export requests. MCP reuses native storage and execution rules and protects unsaved drafts and conflicting external edits. See the [MCP guide](docs/mcp.md) for tools and setup.

## Current limitations

Bru import and selected script API compatibility do not imply full Bruno client compatibility. gRPC, other authentication flows (OAuth 1, OAuth implicit/password grants, AWS, Digest, and others), full asynchronous scripting, Node modules, external secrets, dotenv, PKCS#12/encrypted client keys, and side-by-side project workspaces are not currently available. See [execution rules](docs/execution.md) and [Bruno compatibility](docs/bruno-parity.md).

The collection runner is available in the desktop, core library and MCP. Desktop runs use saved documents and require unsaved document drafts to be saved or discarded; WebSocket requests are skipped. OAuth authorization code uses an HTTP callback on 127.0.0.1 and requires PKCE S256. OAuth code export is not currently supported. There is no standalone CLI product; Demo commands are development tools.

## Development and documentation

The detailed guides below are currently written in Chinese.

| Guide | Contents |
| --- | --- |
| [MCP integration](docs/mcp.md) | Native service, four agent configurations, tools, and execution boundaries |
| [Architecture](docs/architecture.md) | Crate boundaries, modules, and editing/execution/diagnostic flows |
| [Workspace interactions](docs/postman-layout.md) | Navigation, forms, variable hints, settings, and manual acceptance checks |
| [Collection import](docs/import.md) | Supported OpenAPI/Postman conversion, previews and limits |
| [Storage](docs/storage.md) | OpenCollection, drafts, environment overrides, and atomic writes |
| [Execution and scripting](docs/execution.md) | Inheritance, templates, script APIs, extensions, and limits |
| [Development and verification](docs/development.md) | Toolchain, builds, Demo, CI, and non-GUI checks |
| [Updates and releases](docs/auto-update.md) | Versioning, release gates, asset validation, and in-app updates |
| [AGENTS.md](AGENTS.md) | Repository collaboration and implementation conventions |

Run checks from the repository root:

```sh
cargo test --workspace --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
node --test scripts/release/*.test.mjs
```

This repository prohibits Computer Use for verification. Automated tests, builds, and static checks do not establish visual or interactive correctness. Manual acceptance steps are documented in [workspace interactions](docs/postman-layout.md).

## Releases and automatic updates

Pushing a `v<semver>` tag triggers GitHub Actions validation and builds for macOS, Linux, and Windows. Publication waits for all three builds and asset verification. The repository's `$release-tag-publish` skill covers release preparation, versioning, notes, and CI verification; see the [release guide](docs/auto-update.md).

Release builds support quiet update checks. The update panel shows release notes and download progress, with an update entry in the sidebar. After downloading, you can keep working or choose **Restart and install**, which saves drafts first. Development builds do not check automatically.
