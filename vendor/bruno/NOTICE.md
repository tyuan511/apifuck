# Bruno compatibility code

Upstream: https://github.com/usebruno/bruno
Pinned commit: 23fcb63383afdfa16df0da61476a6051930ada5b
License: MIT; upstream notice is retained in LICENSE.

`upstream/` contains the parser and serialization sources required for Bru/OpenCollection conversion. App UI source and branding are not copied. `bridge.ts` exposes conversion as a pure function. Facades restrict imports to conversion utilities and omit dotenv filesystem loading. Nanoid uses its non-secure variant only for ephemeral conversion IDs; these are not credentials. Bundled code has no filesystem/network host APIs and does not execute scripts contained in converted documents.

Rebuild with `bun install --frozen-lockfile --ignore-scripts` and `bun run build` in this directory. Bun is a development-time bundler only; the Rust application embeds the resulting bundles and executes them using QuickJS.

Third-party dependencies: lodash (MIT), ohm-js (MIT), yaml (ISC), nanoid (MIT), chai (MIT), json-query (MIT), arcsecond (MIT), dotenv (BSD-2-Clause), ajv (MIT), ajv-formats (MIT). Installed dependency license texts must be kept in `licenses/` when regenerating distribution bundles.

Local compatibility patch: `formats/yml/common/actions.ts` restores the `local` flag when an OpenCollection response variable has request scope. Upstream unconditionally set it to false, losing scope on Bru round trips. This is covered by the public request fixture test.

Import normalization supplies Bruno’s default sequence 1 when a request omits it, and retains an explicit sequence including zero.
