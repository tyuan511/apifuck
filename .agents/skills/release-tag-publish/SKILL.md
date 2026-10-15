---
name: release-tag-publish
description: "Prepare and publish ApiFuck releases: check code, generate or validate a semantic version, write release notes, sync Cargo versions, push an annotated v tag, and verify GitHub Actions publishes complete release assets. Use for release preparation or publishing requests; setting up the workflow alone does not authorize a release."
metadata:
  short-description: Check, version, document and publish ApiFuck releases
---

# Release Tag Publish

Work from the ApiFuck repository root. Version authority is `[workspace.package].version` in `Cargo.toml`; every workspace crate inherits it and `Cargo.lock` is committed. Read [the release contract](../../../docs/auto-update.md) when changing packaging, asset names or the updater.

## Inspect and check first

1. Inspect `git status`, the current branch, tracking remote, and the on-disk changes. Preserve unrelated edits. A request to prepare infrastructure or notes is not permission to push a version; an explicit request to publish is authorization for the normal release steps below.
2. Fetch branch and tag state from the intended remote, normally `origin`:
   ```sh
   git fetch origin main --tags
   git ls-remote --tags origin
   ```
   Inspect remote refs even when local tags exist. Never force fetch, move, delete or overwrite an existing version tag.
3. Review the release candidate's diff for regressions and run:
   ```sh
   cargo fmt --all --check
   cargo test --workspace --locked
   cargo clippy --workspace --all-targets --locked -- -D warnings
   node --test scripts/release/*.test.mjs
   ```
   Fix relevant failures and rerun affected checks. Do not publish with failed checks or silently alter unrelated work. Verification must remain non-GUI as required by `AGENTS.md`.

## Version and release notes

- Honor an explicit version. For a requested patch/minor/major bump, use `node scripts/release/next-version.mjs patch` (or `minor` / `major`). Without a requested number, use `node scripts/release/next-version.mjs auto` after fetching tags: it keeps an unreleased workspace version when newer than all known versions, otherwise proposes the next stable patch. The command prints a version; it does not modify files or contact GitHub.
- Normalize to `v<semver>` and recheck that exact remote tag with `git ls-remote --tags origin refs/tags/<tag> refs/tags/<tag>^{}`. Also inspect `gh release view <tag>`. If either version exists, stop and report it; ask for another version unless the user explicitly requests resuming an existing draft workflow. Do not choose deletion as a default remedy.
- Find the highest reachable prior version tag below the target. Inspect `git log <previous-tag>..HEAD`, `git diff <previous-tag>` and relevant changed files, including intended uncommitted release changes. For a first release use the available history and say so.
- Write `docs/release/<version>.md` from that evidence: user-visible improvements, fixes, compatibility/migration notes and real limitations. Do not invent changes, claim unrun platform tests, or substitute a raw commit dump. Keep prerelease notes explicit about their status.
- Synchronize the version and lockfile only after notes exist:
   ```sh
   node scripts/release/sync-version.mjs <version> --verify-release-notes
   node scripts/release/validate-release.mjs v<version>
   cargo build --release --locked -p apifuck-desktop
   ```
   The sync helper rolls back both version files if Cargo fails. If source or behavior changes after checks, rerun affected checks. Never tag an unchecked final candidate.

## Publish and verify

When publication was requested:

1. Stage explicit release-related paths, inspect `git diff --cached`, and commit with `chore: release <version>`. Include the approved implementation in the release, but do not blindly `git add -A` when unrelated work is present.
2. Ensure the commit includes the current remote `main`; reconcile any concurrent changes and validate the resulting candidate. Push normally to `main` (`git push origin HEAD:main`), respecting branch protection. If a PR is required, prepare it and wait for merge before tagging the merged commit. Do not force-push or bypass branch protection.
3. Recheck the remote version immediately before creating an annotated tag on the checked release commit. Push only that tag:
   ```sh
   git tag -a v<version> -m "v<version>"
   git push origin v<version>
   ```
4. Find the `Release` workflow for this exact tag and commit, and follow it using `gh run list` / `gh run view`. Report progress without a long blocking wait. The workflow validates notes/version, checks and builds all three platforms, stages artifacts, verifies local and uploaded checksums, and publishes only after every platform succeeds. Prereleases never become `latest`; already public releases are never overwritten.
5. On success, verify `gh release view <tag> --json url,isDraft,isPrerelease,assets` contains all desktop assets, the signed/notarized macOS ZIP and `SHA256SUMS`, and `isDraft` is false. A pushed tag or a created draft is not a completed release.
6. On failure, report the failed job and cause. Do not recreate the tag or loop over retries. After a concrete fix, use a new version for source changes; for a transient failure, resume the existing workflow/draft only when authorized. An infrastructure setup request must not dispatch this publishing workflow as a test.

Report version, comparison baseline, checks, release commit/tag, CI status and the Release link. Clearly distinguish prepared, pushed, building and published states.
