# Contributing to Aegis RWA Contracts

We welcome open-source contributions to make the Aegis Protocol more robust!

## Development Workflow
1. **Fork & Clone:** Fork the repo and clone it locally.
2. **Branching:** Use `feat/`, `fix/`, or `chore/` prefixes.
3. **Testing:** You MUST write unit tests in `src/test.rs` for any new logic added. PRs without test coverage will be rejected.
4. **Formatting:** Ensure `cargo fmt` and `cargo clippy` pass before opening a PR.

## Public API changes

Before changing a contract entry point, event, authorization/precondition,
failure behavior, accounting semantics, or storage layout, follow the
[change review requirements and maintainer checklist](docs/public-api.md#change-review-requirements).
The PR must classify compatibility, update the public reference, compare the
released and candidate contract specs, test event/state behavior, and obtain
SDK/dashboard owner review when required. Breaking changes also require the
version, migration, rollout, and deprecation steps in the policy.

## Discussion
Join our ecosystem discussion before undertaking large architectural changes. Find the `// TODO:` comments in the source code for good places to start contributing.