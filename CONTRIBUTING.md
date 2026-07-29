# Contributing to Aegis RWA Contracts

We welcome open-source contributions to make the Aegis Protocol more robust!

## Development Workflow
1. **Fork & Clone:** Fork the repo and clone it locally.
2. **Branching:** Use `feat/`, `fix/`, or `chore/` prefixes.
3. **Testing:** You MUST write unit tests in `src/test.rs` for any new logic added. PRs without test coverage will be rejected.
4. **Formatting:** Ensure `cargo fmt` and `cargo clippy` pass before opening a PR.

## Releasing
Maintainers cutting a contract release MUST work through
[`docs/release-checklist.md`](docs/release-checklist.md). Run `make release-check`
for the automated portion, then complete the manual compliance, roles, and audit
sections and record the sign-off.

If your PR changes the **storage layout** (`DataKey`) or the **event surface**
(`src/events.rs`), say so explicitly in the PR description — both are breaking
changes for downstream consumers, and `make compat-check` will fail until the
off-chain service in `monitoring/` is updated to match.

## Discussion
Join our ecosystem discussion before undertaking large architectural changes. Find the `// TODO:` comments in the source code for good places to start contributing.