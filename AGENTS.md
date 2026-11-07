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
