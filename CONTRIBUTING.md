# Contributing to Aegis RWA Contracts

We welcome open-source contributions to make the Aegis Protocol more robust!

## Development Workflow
1. **Fork & Clone:** Fork the repo and clone it locally.
2. **Setup:** Run `rustup target add wasm32v1-none` and `cp .env.example .env`. See the [Local Deployment Guide](docs/local-deployment.md) for the full environment setup, including running a local Stellar network.
3. **Branching:** Use `feat/`, `fix/`, or `chore/` prefixes.
4. **Testing:** You MUST write unit tests in `src/test.rs` for any new logic added. PRs without test coverage will be rejected. Run them with `make test` — no network or Docker required.
5. **Formatting:** Ensure `cargo fmt` and `cargo clippy` pass before opening a PR. `make ci` runs the complete gate (`fmt-check` + `clippy` + `test` + `build`) in one command.
6. **PR Evidence Checklist:** Before requesting review, complete the [PR Evidence Checklist](docs/pr-evidence-checklist.md) in your PR description. This covers issue reference, implementation summary, tests, commands run, CI status, and acceptance criteria coverage — making review more objective.
7. **Self-Review:** Before requesting review, fill out the [Contributor Self-Review Form](docs/contributor-self-review-form.md) and include it in your PR. This covers requirements, implementation completeness, testing evidence, CI status, documentation updates, and known limitations.
8. **Completion Table:** You MUST include a [Completion Table](docs/traceability-mapping.md#completion-table-format) in your PR description, mapping every acceptance criterion to its status (Complete/Partial/Not Met), implementation evidence, and test evidence. For complex PRs, also include the [Detailed Traceability Mapping Table](docs/traceability-mapping.md#detailed-traceability-table-format-advanced).

## Discussion
Join our ecosystem discussion before undertaking large architectural changes. Find the `// TODO:` comments in the source code for good places to start contributing.