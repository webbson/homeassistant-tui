---
name: insta-review
description: Run cargo tests and review changed insta snapshots. Use after any change to dashboard YAML structures, widget rendering, query parser, or config loading.
---

Run the test suite and handle insta snapshot changes:

1. Run `cargo test 2>&1` and capture output
2. If snapshots changed (look for "snapshot updated" or test failures mentioning `.snap`):
   - Show which snapshot files changed: `git diff --name-only src/**/*.snap`
   - Run `cargo insta review` for interactive review, OR
   - If running non-interactively, run `cargo insta accept` to accept all pending snapshots, then re-run `cargo test` to confirm green
3. If tests fail for non-snapshot reasons, report the failure with the exact error
4. Report final status: pass/fail + count of snapshots accepted
