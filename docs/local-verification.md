# Local Verification Command

Run **one command** before pushing contract changes to avoid failing CI.

```bash
make verify
```

`make verify` runs, in order:

| Step | Command | What it checks |
|------|---------|----------------|
| Formatting | `cargo fmt --all -- --check` (`fmt-check`) | No unformatted code. |
| Lint | `cargo clippy --all-targets -- -D warnings` (`clippy`) | No lint warnings. |
| Tests | `cargo test` (`test`) | The full unit suite passes. |
| Build | `cargo build --target wasm32v1-none --release` (`build`) | The contract compiles to WASM. |

If any step fails, `make verify` stops there — fix the reported issue and re-run.
All four must pass locally before opening or updating a PR.

## Why
Contributors sometimes submit changes without running build/tests locally,
which then fail in automation and block approval. `make verify` is the single
pre-push gate that catches all of those before you push.

## Related targets
- `make ci` — the stricter gate CI enforces (same steps); use `make verify` for
  fast local feedback, `make ci` to mirror the pipeline exactly.
- `make test` — run only the test suite.
- `make fmt` — auto-format in place (run this if `fmt-check` fails).
- `make all` — format + test + build (formats instead of checking).

## Requirements
- Rust with the `wasm32v1-none` target: `rustup target add wasm32v1-none`.
- `cargo clippy` installed (usually bundled with the Rust toolchain; if
  missing, `rustup component add clippy`).

See also: [Failing CI Response Guide](./failing-ci-guide.md) for how to
reproduce and fix each failure class.
