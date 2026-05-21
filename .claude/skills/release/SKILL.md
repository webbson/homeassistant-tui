---
name: release
description: Cut a new tagged release of ha-tui. Determines the next semver number from conventional-commit prefixes since the last tag, moves CHANGELOG's Unreleased section into a new versioned section, bumps Cargo.toml, commits, and creates an annotated tag. Use when the user says "release", "cut a release", "tag a release", or "/release [major|minor|patch]".
---

# Release skill

This skill cuts a new ha-tui release end-to-end. It is **safe-by-default** — every step prints what will change and waits for explicit confirmation before pushing or applying anything destructive.

## Inputs

The user may pass an explicit bump level:

- `/release` — auto-detect from commits since the last tag.
- `/release major` — force major bump.
- `/release minor` — force minor bump.
- `/release patch` — force patch bump.

## Steps

Execute these in order. Stop and report any failure.

### 1. Verify a clean working tree on `main`

```bash
git rev-parse --abbrev-ref HEAD
git status --porcelain
```

- Branch MUST be `main`. If not, abort and tell the user to switch.
- Working tree MUST be empty. If not, abort with the list of dirty files.

### 2. Find the previous tag + commits since it

```bash
PREV_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
git log --format='%s' "${PREV_TAG:+$PREV_TAG..}HEAD"
```

- If `PREV_TAG` is empty, treat the entire history as "since last release".

### 3. Determine the bump level

If the user gave an explicit level, use it. Otherwise scan the commit subjects from step 2:

- Any commit body containing `BREAKING CHANGE:` OR subject starting with `feat!:` / `fix!:` (any type with `!`) → **major**.
- Any subject starting with `feat(` or `feat:` → **minor**.
- Otherwise → **patch**.

Print the detected level and the commits that drove it. Wait for the user's "go" / "ok" / "yes" before proceeding.

### 4. Compute the next version

Read the current `version = "X.Y.Z"` line from `Cargo.toml`. Apply the bump:

- major → `(X+1).0.0`
- minor → `X.(Y+1).0`
- patch → `X.Y.(Z+1)`

Print the new version. Confirm with the user.

### 5. Check formatting

```bash
cargo fmt -- --check
```

If it fails, run `cargo fmt`, then re-run the check. Abort if still failing.

### 6. Update `Cargo.toml`

Use the `Edit` tool to replace the `version = "X.Y.Z"` line under `[package]` with the new version. Run `cargo build --quiet` afterwards so `Cargo.lock` updates.

### 7. Update `CHANGELOG.md`

Open `CHANGELOG.md` and:

1. Insert a new section under the existing `## [Unreleased]` header. The new section header is `## [X.Y.Z] – YYYY-MM-DD` using today's date (UTC).
2. Move the body content from `## [Unreleased]` into the new section — leave `## [Unreleased]` empty (with the four standard subheadings: `### Added`, `### Changed`, `### Fixed`, `### Removed`, all empty for now).
3. Update the bottom link references:
   - Replace the `[Unreleased]:` URL's `v<old>...HEAD` with `v<new>...HEAD`.
   - Add a new `[X.Y.Z]:` link below the `[Unreleased]:` line pointing at `<repo-url>/compare/v<prev>...v<new>` (or `/releases/tag/v<new>` if there was no previous tag).

If the Unreleased section is empty, try to build a changelog with the commits that will be included, if unable ask user what to populate it with.

### 8. Print the diff for review

Run `git diff Cargo.toml Cargo.lock CHANGELOG.md` and show it to the user. Wait for confirmation before committing.

### 9. Commit + tag

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore(release): vX.Y.Z"
git tag -a vX.Y.Z -m "Release X.Y.Z"
```

Use a HEREDOC for the tag message body — paste the body of the new CHANGELOG section into the annotation so `git show vX.Y.Z` displays the release notes.

### 10. Report

Print:

- The new version
- The tag SHA
- A reminder to push with `git push --follow-tags origin main`

Do **NOT** push automatically. The user pushes when ready.

## Safety rules

- Never force-push.
- Never delete a tag without explicit user confirmation.
- Never bypass hooks (`--no-verify`).
- Never re-tag an existing version.
- If `cargo build` fails after the version bump, roll the version change back with `git checkout -- Cargo.toml Cargo.lock` and report the build error.
- If `cargo fmt -- --check` fails, run `cargo fmt` to fix, then verify clean before proceeding.

## What this skill does NOT do

- It does not write release notes from scratch — it relies on the curated `## [Unreleased]` section in `CHANGELOG.md`. If that section is empty the release aborts.
- It does not run tests. Run them yourself before invoking this skill.
- It does not publish to crates.io. Run `cargo publish` separately.
