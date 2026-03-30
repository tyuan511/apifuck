# ApiFuck

[![GitHub Release](https://img.shields.io/github/v/release/tyuan511/apifuck?label=release)](https://github.com/tyuan511/apifuck/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](./LICENSE)

A desktop API client built with Tauri v2, React, and TypeScript for local debugging, project-based API organization, and a dense workspace experience.

[中文说明](./README.md)

## Features

- **Project-based organization**: Manage APIs with a project, collection, and request tree.
- **Multi-tab request editing**: Open, switch, reorder, close, and restore multiple request tabs.
- **Full request builder**: Configure HTTP method, URL, query, headers, auth, and body in one workspace.
- **Authentication support**: Includes `None`, `Basic`, `Bearer`, and `API Key`.
- **Response inspection**: View status, duration, size, headers, and response body with formatted JSON output.
- **Environment management**: Maintain multiple environments and switch the active environment per project.
- **Request scripting**: Supports pre-request and post-request scripts with environment and script variable access.
- **File-based workspace storage**: Stores projects as folders and JSON files for portability and Git friendliness.
- **Desktop auto update**: Ships updates through GitHub Releases.
- **Native desktop experience**: Built on Tauri v2 with macOS window controls and drag region support.

## Usage

### Download

<!-- release-downloads-en:start -->
- Latest release: [v0.0.6](https://github.com/tyuan511/apifuck/releases/tag/v0.0.6)
- macOS: [ApiFuck_0.0.6_aarch64.dmg](https://github.com/tyuan511/apifuck/releases/download/v0.0.6/ApiFuck_0.0.6_aarch64.dmg)
- Windows: [ApiFuck_0.0.6_x64-setup.exe](https://github.com/tyuan511/apifuck/releases/download/v0.0.6/ApiFuck_0.0.6_x64-setup.exe)
- Linux: [ApiFuck_0.0.6_amd64.AppImage](https://github.com/tyuan511/apifuck/releases/download/v0.0.6/ApiFuck_0.0.6_amd64.AppImage)
<!-- release-downloads-en:end -->

### Requirements

- Node.js 18+
- Bun
- Rust 1.70+

### Local development

```bash
# Install dependencies
bun install

# Start the frontend dev server
bun run dev

# Start the desktop app in development
bun run tauri dev
```

### Production build

```bash
bun run build
bun run tauri build
```

### Common commands

| Command | Description |
|---------|-------------|
| `bun install` | Install dependencies |
| `bun run dev` | Start the Vite dev server |
| `bun run build` | Type-check and build the frontend |
| `bun run lint` | Run ESLint |
| `bun run lint:fix` | Apply safe ESLint fixes |
| `bun run tauri dev` | Start the desktop app in development |
| `bun run tauri build` | Build the desktop app |
| `cd src-tauri && cargo test` | Run Rust tests |

## How It Works

### Stack

- **Desktop shell**: Tauri v2
- **Frontend**: React 19 + TypeScript + Vite
- **UI**: shadcn/ui + Tailwind CSS v4
- **State management**: Zustand
- **Editor**: Monaco Editor
- **HTTP execution**: Rust `reqwest`

### Project layout

```text
apifuck/
├── src/                      # React frontend
│   ├── app.tsx               # App entry
│   ├── main.tsx              # React bootstrap
│   ├── components/ui/        # Shared UI components
│   ├── features/workbench/   # Main workbench feature
│   └── lib/                  # Shared frontend logic
├── src-tauri/                # Tauri / Rust backend
│   ├── src/
│   ├── capabilities/
│   └── tauri.conf.json
├── docs/                     # Docs and release notes
├── scripts/                  # Release helper scripts
└── AGENTS.md                 # Collaboration guidelines
```

### Storage model

Workspace data is stored as folders and JSON files:

```text
<workspace>/
├── workspace.json
└── <project>/
    ├── metadata.json
    └── items/
        api_<ulid>__<slug>.json
        <collection>/
            ├── metadata.json
            └── items/
```

This keeps the data:

- readable
- portable
- easy to version with Git

## Contributing

Issues and pull requests are welcome.

Before contributing, it is helpful to read:

- [AGENTS.md](./AGENTS.md)
- [docs/auto-update.md](./docs/auto-update.md)

Recommended checks before opening a PR:

```bash
bun run lint
bun run build
```

If you changed Rust code, also run:

```bash
cd src-tauri && cargo test
```

## License

This project is licensed under the [MIT License](./LICENSE).
