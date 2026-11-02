# bikey

A Vietnamese input method (bộ gõ tiếng Việt) for Windows that recognises
English as you type, so you never need to switch modes.

> **Status:** early development, Windows only for now.

## Install on Windows

Run `bikey-setup-<version>.exe` (Windows 10/11, 64-bit, needs administrator
rights), then:

1. Turn off UniKey, EVKey or OpenKey if you use them.
2. Fully quit and reopen apps that were already running. Closing a window is
   often not enough: quit Zalo from its tray icon.
3. If bikey is not selected, press **Win+Space** and choose it. The installer's
   "default keyboard" option, on by default, keeps it on after a restart.

A button near the clock shows **V** (Vietnamese) or **E** (English). Click it or
press **Alt+Z** to switch; its menu picks Telex, VNI and tone style. If a word
converts by mistake (`cos` to `có`), press **Esc** while typing it or right after
it to get your exact keys back. Remove bikey
from Settings → Apps. Files still in use are deleted at the next restart.

The installer is not code-signed yet, so Windows may show a warning.

## Features

- **Telex** (default) and **VNI**
- **Smart English detection:** `text`, `class` and `these` stay English, while
  `tieengs` becomes `tiếng`
- Reads ambiguous words from context: `taats car` gives `tất cả`, `this car` stays
  `this car`
- Leaves code and addresses alone: `obj.dir`, `dir_name` and `user@gmail.com` stay
  as typed, while `và/hoặc` and `Hà-Nội` still convert
- Traditional (`hòa`) or modern (`hoà`) tone placement
- A proper Windows input method (TSF) with no global keyboard hook
- No network access and no keystroke logging. See [SECURITY.md](SECURITY.md).

**Use one Vietnamese typing tool at a time.** Hook-based tools such as UniKey,
EVKey and OpenKey rewrite keystrokes system-wide, so running one alongside bikey
scrambles text. Turn it off (or switch it to English) while bikey is selected.

## Tiếng Việt

bikey là bộ gõ tiếng Việt cho Windows. Bộ gõ tự nhận ra khi bạn gõ tiếng Anh nên
không cần bật/tắt chế độ. Hỗ trợ **Telex** (mặc định) và **VNI**, dùng framework
bộ gõ chính thức của Windows, không theo dõi bàn phím toàn hệ thống và không kết
nối mạng.

## Layout

```
crates/bikey-core   platform-independent engine (Rust, no unsafe)
crates/bikey-ffi    C ABI, kept for possible other platforms
platforms/windows   TSF text service
installer/          Windows installer (Inno Setup)
```

## Building

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your
option.
