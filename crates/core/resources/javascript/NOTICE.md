# Embedded JavaScript resources

`bridge.bundle.js` implements Bru/OpenCollection conversion. `runtime.bundle.js`
provides Chai assertions, json-query and AJV for QuickJS scripts. Cargo embeds both
files; normal builds and application execution require neither Node.js/Bun nor a
Bruno source checkout.

The converter is based on https://github.com/usebruno/bruno at commit
`23fcb63383afdfa16df0da61476a6051930ada5b`. Its MIT notice is retained in `LICENSE`
and also covers the public Bruno fixtures under `crates/core/tests/fixtures/`.
Upstream source snapshots are not checked into this repository.

To rebuild, use Bun and Git with network access:

```sh
cd scripts/javascript
bun install --frozen-lockfile --ignore-scripts
bun run build
```

The build script fetches the pinned upstream commit into an ignored temporary
directory, applies the request-scope compatibility patch, bundles the conversion
facades and script runtime, and removes the downloaded source in a finally block.
The patch restores `local` for response variables with request scope. Import
normalization supplies sequence 1 when omitted and preserves explicit zero.

Facades restrict imports to conversion utilities and omit dotenv filesystem
loading. Nanoid uses its non-secure variant only for ephemeral conversion IDs.
The converter has no filesystem/network host APIs and does not execute scripts
contained in imported documents.

Third-party dependency license texts are retained in `licenses/`: lodash (MIT),
ohm-js (MIT), yaml (ISC), nanoid (MIT), chai (MIT), json-query (MIT), arcsecond
(MIT), dotenv (BSD-2-Clause), ajv (MIT), ajv-formats (MIT), and their dependencies.
Keep these texts in sync when changing the locked dependencies. After rebuilding,
review the bundles and licenses and run the core conversion and script tests.
