# bikey

A Vietnamese input method (bộ gõ tiếng Việt) for Windows, macOS and Linux that
recognises English as you type, so you never need to switch modes.

> **Status:** early development, not usable yet.

## Features (planned)

- **Telex** (default) and **VNI**
- **Smart English detection:** `text`, `class` and `these` stay English, while
  `tieengs` becomes `tiếng`
- Traditional (`hòa`) or modern (`hoà`) tone placement
- Uses each OS's official input-method framework (TSF, InputMethodKit, Fcitx5):
  no global keyboard hooks, no Accessibility permission, works on Wayland
- No network access and no keystroke logging. See [SECURITY.md](SECURITY.md).

**Use one Vietnamese typing tool at a time.** Hook-based tools such as UniKey,
EVKey and OpenKey rewrite keystrokes system-wide, so running one alongside bikey
scrambles text. Turn it off (or switch it to English) while bikey is selected.

## Tiếng Việt

bikey là bộ gõ tiếng Việt cho Windows, macOS và Linux. Bộ gõ tự nhận ra khi bạn
gõ tiếng Anh nên không cần bật/tắt chế độ. Hỗ trợ **Telex** (mặc định) và
**VNI**, dùng framework bộ gõ chính thức của từng hệ điều hành, không theo dõi
bàn phím toàn hệ thống và không kết nối mạng.

## Layout

```
crates/bikey-core   platform-independent engine (Rust, no unsafe)
crates/bikey-ffi    C ABI for the platform shims          (planned)
platforms/windows   TSF text service                      (planned)
platforms/macos     InputMethodKit app                    (planned)
platforms/linux-*   Fcitx5 addon                          (planned)
```

## Building

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option.
