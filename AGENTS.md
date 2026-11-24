# AGENTS.md

Hard rules for humans and coding agents in this repo.

- Never build, test, or run Docker, emulators or simulators on a developer's machine. GitHub Actions compiles and tests everything. Commit, push to `master`, and read the results with `gh run`.
- Commit small changes, each with a short conventional message and no attribution lines.
- Comments are 1-2 lines and state a non-obvious constraint. Never narrate the next line.
- Docs read like a competent human wrote them: direct, concrete, no marketing filler.
- Dead code is deleted. No speculative abstractions.
- Keep the Rust and TypeScript versions' semantics the same. A behavior change lands in both, with a test in both.
- An authentication call resolves exactly once: success, a coded failure, or a cancel. It never hangs.
- The main branch is `master`.

## Releasing

Add a `## [X.Y.Z] - YYYY-MM-DD` entry at the top of `CHANGELOG.md` and push to `master`. The Changelog Release workflow bumps the versions, tags `vX.Y.Z`, creates the GitHub release with that entry as its notes and publishes to crates.io. Do not bump versions or push tags by hand. A crates.io version cannot be replaced, so CI must be green before the entry lands. To finish a release that stopped halfway: `gh workflow run changelog-release.yml`, or `gh workflow run publish.yml -f tag=vX.Y.Z` for an older tag.
