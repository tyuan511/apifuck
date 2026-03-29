# ApiFuck

A powerful desktop API client built with Tauri v2, React, and TypeScript. Manage projects, collections, and API requests with a professional high-density workspace interface.

[中文版](./README_zh.md)

## Features

- **Project-based Organization**: Organize your APIs into projects and collections
- **Multi-tab Request Editor**: Work on multiple requests simultaneously with tabbed interface
- **Request Builder**: Support for Query params, Headers, Body (Raw/JSON), Auth (Basic/Bearer/API Key)
- **Response Viewer**: Automatic JSON formatting with syntax highlighting, text fallback
- **Workspace Storage**: File-based workspace model with Git-friendly JSON files
- **Desktop Native**: Native window controls, macOS traffic lights support

## Tech Stack

- **Backend**: Tauri v2 (Rust)
- **Frontend**: React 19 + TypeScript + Vite
- **UI**: shadcn/ui components + Tailwind CSS v4
- **State**: Zustand
- **Editor**: Monaco Editor
- **HTTP Client**: Rust reqwest

## Getting Started

### Prerequisites

- Node.js 18+
- Bun (recommended) or npm
- Rust 1.70+

### Installation

```bash
# Install dependencies
bun install

# Run in development mode (web)
bun run dev

# Run as desktop app
bun run tauri dev

# Build for production
bun run tauri build
```

### Available Scripts

| Command | Description |
|---------|-------------|
| `bun run dev` | Start Vite dev server |
| `bun run build` | TypeScript check + production build |
| `bun run lint` | ESLint check |
| `bun run lint:fix` | Apply ESLint fixes |
| `bun run tauri dev` | Launch desktop app in dev mode |
| `bun run tauri build` | Build native desktop bundle |
| `cd src-tauri && cargo test` | Run Rust tests |

## Project Structure

```
apifuck/
├── src/                      # React frontend
│   ├── app.tsx               # Main app entry
│   ├── main.tsx              # React bootstrapping
│   ├── components/ui/        # shadcn/ui primitives
│   ├── features/workbench/    # Main workbench feature
│   │   ├── components/       # UI components
│   │   ├── hooks/           # Custom React hooks
│   │   ├── store/           # Zustand store
│   │   ├── types.ts         # Feature types
│   │   └── utils.ts         # Utilities
│   └── lib/                  # Shared utilities
├── src-tauri/                # Tauri/Rust backend
│   ├── src/
│   │   ├── main.rs          # Entry point
│   │   ├── http.rs          # HTTP request handling
│   │   └── error.rs         # Error types
│   ├── capabilities/        # Tauri capabilities
│   └── tauri.conf.json      # Tauri configuration
├── docs/                     # Design documents
└── CLAUDE.md                 # Project guidelines
```

## Workspace Storage Model

The app uses a workspace folder-based storage system:

```
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

## Development

### Adding shadcn Components

```bash
bun shadcn add <component>
```

### Code Style

- React components: named exports, PascalCase filenames
- Utilities: camelCase
- Uses `@antfu/eslint-config` for linting
- Prettier formatting via ESLint

## License

Private project
