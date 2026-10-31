# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `bikey-core` engine: Telex (default) and VNI, marks typeable anywhere in a
  word, traditional/modern tone placement, syllable validation, smart English
  detection with a reviewed list of 3,052 English exceptions, double-key
  cancel (`carr` → `car`), Backspace that re-places the tone.
- `tools/gen-dict` to find English words that Telex would turn Vietnamese.
- Unit, table, round-trip (every valid syllable) and randomized tests; a
  cargo-fuzz target run in CI.
- Cargo workspace, pinned toolchain, CI (Windows/macOS/Linux), cargo-deny,
  Dependabot, security policy.
