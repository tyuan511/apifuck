---
name: release-tag-publish
description: Publishes a requested release version by first fetching and inspecting all remote git tags, stopping for user confirmation if the target version already exists, drafting release notes from the diff since the previous version, then committing all local release-related changes, syncing them to main, and finally creating and pushing the git tag to trigger release CI. Use when the user asks to publish, release, cut a version, push a tag, or prepare release notes for a specific version.
metadata:
  short-description: Safely prepare notes and push release tags
---

# Release Tag Publish

Use this skill when the user wants to publish a specific version and expects a safe release workflow rather than a blind `git tag && git push`.

## Workflow

1. Confirm the requested version string and normalize the release tag.
2. Inspect the repo state before making release changes.
3. Fetch remote tags and inspect the complete remote tag set with Git.
4. If the remote version already exists, stop and tell the user exactly which tag already exists.
5. Ask the user to choose between deleting the remote tag/release or using a different version.
6. If the version is available, find the previous version tag from the fetched tag list and collect the git diff since that version.
7. Draft release notes from the actual changes instead of guessing.
8. Update or add any release-related files required by the repo, then commit all local release-related changes together.
9. Push that commit to the `main` branch before creating the release tag.
10. Create the annotated tag locally and push it to the remote that should trigger release CI.
11. Report the pushed commit, tag, previous version used for comparison, and the final release notes summary.

## Rules

- Always fetch remote tags with Git before deciding whether a version is available.
- Use `gh` to inspect GitHub Releases after the git-level tag check, not instead of it.
- Treat an existing remote version as a hard stop. Do not delete or overwrite it unless the user explicitly asks.
- Be explicit about the exact version string you checked, for example `v1.2.3` or `1.2.3`.
- Prefer comparing against the latest reachable remote semantic version tag before the requested version.
- If no prior tag exists, say that the release notes are based on the full history currently available.
- Build release notes from git history and changed files, then compress them into user-facing bullets.
- Keep release notes tied to user-visible or developer-relevant changes. Avoid noisy commit-by-commit dumps.
- Do not create or push the tag until the remote version conflict check passes and the release-related local changes have been committed.
- Use annotated tags by default.
- Push the release preparation commit to `main` before pushing the tag.
- Commit all current local release-related changes together when preparing the release. Do not leave the repo half-updated.

## Remote Version Check

First sync the remote tag state with Git:

```bash
git fetch --tags origin
git ls-remote --tags origin
```

Then verify the requested version against the fetched remote tag list. After that, if the repo uses GitHub Releases, inspect the GitHub release state too:

```bash
gh release view v1.2.3
```

If the remote tag list already contains the target version:

- Stop immediately.
- Tell the user that the remote already has that version.
- Ask the user whether they want to delete the remote version/tag first or switch to a new version number.

Do not choose for them.

## Release Notes Drafting

Gather context from:

- `git fetch --tags origin`
- `git tag --sort=-version:refname`
- `git log --oneline <previous-tag>..HEAD`
- `git diff --stat <previous-tag>..HEAD`
- targeted file reads for the most important changed areas

Then draft release notes in a concise structure like:

- Highlights
- Fixes
- Internal or maintenance changes

If the repo follows Conventional Commits, use that as a hint, but verify against the actual diff.

## Tagging And Push

After the remote conflict check passes and the notes are ready, commit the local release changes and sync them to `main` first:

```bash
git add -A
git commit -m "chore: release 1.2.3"
git push origin HEAD:main
```

Then create and push the tag:

```bash
git tag -a v1.2.3 -m "v1.2.3"
git push origin v1.2.3
```

If the repository expects a different remote than `origin`, detect it from the current branch tracking info before pushing.

## Response Expectations

When using this skill, the final response should include:

- The exact version that was checked
- Whether the remote already had that version
- The previous tag used for the diff, if any
- The drafted release notes
- Whether local release changes were committed and pushed to `main`
- Whether the tag was created and pushed

If blocked by an existing remote version, end with a short decision request instead of proceeding.
