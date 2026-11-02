# Security policy

An input method sees everything a user types, so bikey is built to hold as
little as possible for as short a time as possible.

## Guarantees

- **No keystroke logging.** Typed text is never written to disk, logs, crash
  reports or the clipboard. The engine only holds the word currently being
  composed (at most 32 keys) and clears it on commit, reset and focus change. On
  Windows the word just finished is kept in memory only until the next key,
  click or focus change, so that Esc can put back what was typed. Between words
  it remembers only whether the last word was Vietnamese or English, never the
  word itself.
- **No network access.** bikey contains no networking code: no telemetry,
  analytics or auto-update. New versions come only from signed GitHub releases.
- **Password fields are left alone.** bikey stands aside when the OS marks a
  field as secure (Windows TSF password input scope, macOS secure input, Fcitx5
  `Password`/`Sensitive` capabilities).
- **Official OS frameworks only.** bikey uses TSF, InputMethodKit and Fcitx5.
  It never installs global keyboard hooks and never asks for Accessibility
  permission. On Windows a mouse hook limited to the app's own UI thread ends the
  current word on a click; it reads only that a button was pressed.
- **Simulated input on Windows.** Apps handle TSF compositions too differently,
  so bikey types with `SendInput`: Backspaces and Unicode characters for the
  current word, and the key you pressed sent again when it ends a word. It only
  receives keys through TSF, while selected as the input method in a text field
  that is not marked secure, and it sends nothing else.
- **Memory safety.** The engine is written in Rust with
  `#![forbid(unsafe_code)]`. Any `unsafe` code is confined to the FFI and
  platform crates, and each block has a `// SAFETY:` comment.

## Supported versions

Only the latest release receives security fixes.

## Reporting a vulnerability

Please **do not** open a public issue. Report privately through
[GitHub Security Advisories](https://github.com/levuthanhbinh/bikey/security/advisories/new).
You should get an acknowledgement within 7 days. Please allow up to 90 days
for a fix before disclosing publicly.
