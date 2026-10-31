# Contributing

## Setup

- Rust via [rustup](https://rustup.rs). The toolchain version is pinned in
  `rust-toolchain.toml` and installs automatically.
- Windows: Visual Studio 2022 Build Tools with the **Desktop development with
  C++** workload (for the MSVC linker).

## Before opening a PR

```sh
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

CI runs the same checks on Windows, macOS and Linux, plus `cargo deny`.

## Rules

- `bikey-core` stays free of `unsafe`, I/O and runtime dependencies.
- Every new `unsafe` block needs a `// SAFETY:` comment explaining why it holds.
- Never log, print or persist typed text, not even in debug builds or tests
  that run in CI.
- New dependencies need a reason in the PR description and must pass
  `cargo deny`.
- Engine behaviour changes come with table tests (raw keys → expected output).
