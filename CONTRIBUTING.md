# Contributing to Aegis RWA Contracts

We welcome open-source contributions to make the Aegis Protocol more robust!

## Development Workflow
1. **Fork & Clone:** Fork the repo and clone it locally.
2. **Setup:** Run `rustup target add wasm32v1-none` and `cp .env.example .env`. See the [Local Deployment Guide](docs/local-deployment.md) for the full environment setup, including running a local Stellar network.
3. **Branching:** Use `feat/`, `fix/`, or `chore/` prefixes.
4. **Testing:** You MUST write unit tests in `src/test.rs` for any new logic added. PRs without test coverage will be rejected. Run them with `make test` — no network or Docker required.
5. **Formatting:** Ensure `cargo fmt` and `cargo clippy` pass before opening a PR. `make ci` runs the complete gate (`fmt-check` + `clippy` + `test` + `build`) in one command.
6. **Traceability:** You MUST include a [Traceability Mapping Table](docs/traceability-mapping.md) in your PR description, mapping every acceptance criterion to its implementation, storage changes, and tests.

## Discussion
Join our ecosystem discussion before undertaking large architectural changes. Find the `// TODO:` comments in the source code for good places to start contributing.