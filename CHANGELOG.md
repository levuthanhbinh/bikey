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
- `bikey-ffi`: C API for the macOS and Linux shims.
- Windows TSF text service (`platforms/windows`): types directly with
  `SendInput` instead of compositions, Alt+Z on/off, stands aside in password
  fields, 64-bit and 32-bit DLLs; `scripts/dev-install.ps1` and the
  `tools/tsf-smoke` end-to-end test.
- Windows input-mode button near the clock: click to turn Vietnamese on or off,
  menu for Telex, VNI and tone style. Settings apply in every app from the next
  word.
- Logo: E and V sharing one tile, used for the input method, the installer and
  the README. The V and E buttons reuse its colours (`scripts/gen-icons.py`).
- Words that are both English and Vietnamese (`car`/`cả`) follow the language of
  the previous word.
- Code and addresses stay as typed (`obj.dir`, `dir_name`, `user@gmail.com`).
- Esc on a converted word, or right after finishing one, puts back the exact keys.
- Installer option, on by default, that adds bikey to the keyboard list and makes
  it the default, so it stays on after a restart. Uninstalling removes it.
- Windows installer (Inno Setup): fixed install folder in Program Files, never
  closes running apps, upgrades without a reboot, clean uninstall.
- `tools/gen-dict` to find English words that Telex would turn Vietnamese.
- Unit, table, round-trip (every valid syllable) and randomized tests; a
  cargo-fuzz target run in CI.
- Cargo workspace, pinned toolchain, CI (Windows/macOS/Linux), cargo-deny,
  Dependabot, security policy.
