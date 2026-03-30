# Repository Guidelines

## Project Structure & Module Organization
The app is split between a Vite/React frontend and a Tauri v2 backend. Frontend code lives in `src/`: `app.tsx` is the main screen, `main.tsx` boots React, `components/ui/` holds shadcn-based primitives, `lib/` contains shared helpers, and `assets/` stores bundled static files. Public web assets live in `public/`. Native desktop code lives in `src-tauri/`, with Rust entry points under `src-tauri/src/`, app capabilities in `src-tauri/capabilities/`, and packaging metadata in `src-tauri/tauri.conf.json`.

## Build, Test, and Development Commands
Use Bun for local work because `bun.lock` is checked in.

- `bun install`: install frontend dependencies.
- `bun run dev`: start the Vite dev server for web UI work.
- `bun run build`: run TypeScript checks and produce a production web build.
- `bun run lint`: run ESLint on the TypeScript/React codebase.
- `bun run lint:fix`: apply safe ESLint fixes.
- `bun shadcn add <component>`: add shadcn/ui components as source files.
- `bun run tauri dev`: launch the desktop app in Tauri development mode.
- `bun run tauri build`: create native desktop bundles.
- `cd src-tauri && cargo test`: run Rust tests when backend logic is added.

## Coding Style & Naming Conventions
Follow the existing style: TypeScript uses ES modules and React function components. React component filenames should use camel-case, while component identifiers use CamelCase, for example `user-card.tsx` exporting `UserCard`. Use named exports only; do not add default exports. Utility functions use camelCase. Prefer path aliases from `components.json`, for example `@/components` and `@/lib`, and keep shared UI in `src/components/ui/`. When styling, prefer shadcn semantic theme tokens such as `bg-primary`, `text-muted-foreground`, and `border-border` instead of hard-coded colors so light/dark theme switching stays consistent. Run `bun run lint` before submitting; ESLint uses `@antfu/eslint-config` and currently ignores `src-tauri/**`.

## Testing Guidelines
There is no committed frontend test runner yet, so do not claim automated coverage that does not exist. For UI changes, verify behavior manually in `bun run dev` or `bun run tauri dev`. For Rust logic, add focused unit tests alongside the code and run `cargo test`. If you introduce a frontend test stack, keep test filenames consistent with `*.test.ts` or `*.test.tsx`.

## Commit & Pull Request Guidelines
The current history uses Conventional Commit prefixes such as `feat: init project`; continue with `feat:`, `fix:`, `refactor:`, `docs:`, and similar scopes. Keep commits narrow and descriptive. Pull requests should include a short summary, note any Tauri capability or config changes, link related issues, and attach screenshots or recordings for visible UI changes.

## Collaboration Rules
If you open a file and find it has already changed from what you expected, treat the current on-disk version as the source of truth. Do not overwrite, revert, or "clean up" user changes unless the task explicitly requires it. When editing around existing changes, preserve them and make the smallest compatible update.
