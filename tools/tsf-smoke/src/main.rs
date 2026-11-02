//! End-to-end check of the *installed* bikey text service (run
//! `scripts/dev-install.ps1` first).
//!
//! Usage: `tsf-smoke [edit|richedit|richedit-tsf|password|vni]` (default `edit`).
//! `password` checks that bikey leaves a password field alone. `vni` uses an
//! `EDIT` control and expects `HKCU\Software\bikey\Method` = 1.
//!
//! Opens its own window, switches this process to bikey and sends real
//! keystrokes with `SendInput`, the same path physical typing takes. Those keys
//! go to whichever window is in front, so before every key it checks that its
//! own window is still there and stops if not. Don't type while it runs.
//!
//! Exit code: 0 all passed, 1 a case failed, 2 inconclusive or setup error.
//! Set `BIKEY_SMOKE_VERBOSE=1` to print the text and caret after every key.

#[cfg(not(windows))]
fn main() {
    eprintln!("tsf-smoke only runs on Windows");
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    match win::run() {
        Ok(true) => std::process::ExitCode::SUCCESS,
        Ok(false) => std::process::ExitCode::FAILURE,
        Err(e) => {
            eprintln!("inconclusive: {e}");
            std::process::ExitCode::from(2)
        }
    }
}

#[cfg(windows)]
mod win {
    use std::time::{Duration, Instant};

    use bikey::{CLSID_BIKEY, PROFILE_BIKEY};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::Graphics::Gdi::ClientToScreen;
    use windows::Win32::System::Com::{
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    };
    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Controls::RichEdit::{EM_SETEDITSTYLE, SES_USECTF};
    use windows::Win32::UI::Controls::{EM_GETSEL, EM_SETSEL};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetFocus, HKL, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT,
        KEYEVENTF_KEYUP, MAPVK_VK_TO_VSC, MapVirtualKeyW, SendInput, SetFocus, VIRTUAL_KEY,
        VK_BACK, VK_LEFT, VK_OEM_PERIOD, VK_RETURN, VK_SHIFT, VK_SPACE,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEINPUT,
    };
    use windows::Win32::UI::TextServices::{
        CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, ITfInputProcessorProfileMgr,
        TF_INPUTPROCESSORPROFILE, TF_IPPMF_FORPROCESS, TF_PROFILETYPE_INPUTPROCESSOR,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, ES_MULTILINE,
        ES_PASSWORD, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, MSG, PM_REMOVE,
        PeekMessageW, RegisterClassW, SendMessageW, SetForegroundWindow, SetWindowTextW,
        TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW, WS_CHILD, WS_OVERLAPPEDWINDOW,
        WS_VISIBLE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, SetCursorPos};
    use windows_core::{PCWSTR, w};

    type Result<T> = std::result::Result<T, String>;

    /// Keys to type (`\u{8}` = Backspace) and the text expected afterwards.
    /// Assumes default settings (Telex, traditional tones, smart English).
    /// `tieengs` comes first: if bikey is not processing keys it fails here,
    /// whereas the English cases would pass without bikey involved at all.
    const CASES: &[(&str, &str)] = &[
        ("tieengs ", "tiếng "),
        ("Vieetj Nam ", "Việt Nam "),
        ("nguwowif ", "người "),
        ("dduwowcj ", "được "),
        ("class ", "class "),
        ("new ", "new "),
        ("these ", "these "),
        ("yes ", "yes "),
        ("carr ", "car "),
        ("toans\u{8} ", "tóa "),
        ("text\u{8}\u{8} ", "te "),
        ("hoaf.", "hòa."),
        ("hoaf2", "hòa2"),
        ("Vieetj\n", "Việt\r\n"),
        ("ddi\u{2190}b ", "đb i"),
    ];

    /// A password control must get exactly the keys typed.
    const PASSWORD_CASES: &[(&str, &str)] = &[("tieengs ", "tieengs "), ("dd", "dd")];

    const VNI_CASES: &[(&str, &str)] = &[
        ("tie6ng1 ", "tiếng "),
        ("Vie65t ", "Việt "),
        ("d9u7o7c5 ", "được "),
        ("hoa2 ", "hòa "),
        ("2026 ", "2026 "),
    ];

    fn verbose() -> bool {
        std::env::var_os("BIKEY_SMOKE_VERBOSE").is_some()
    }

    extern "system" fn window_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: forwarding a window message to the default handler.
        unsafe { DefWindowProcW(window, message, wparam, lparam) }
    }

    fn pump(duration: Duration) {
        let end = Instant::now() + duration;
        let mut msg = MSG::default();
        while Instant::now() < end {
            // SAFETY: standard message loop on this thread's queue.
            unsafe {
                while PeekMessageW(&raw mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                    let _ = TranslateMessage(&raw const msg);
                    DispatchMessageW(&raw const msg);
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn text(edit: HWND) -> String {
        // SAFETY: plain window-text calls on our own control.
        unsafe {
            let len = usize::try_from(GetWindowTextLengthW(edit)).unwrap_or(0);
            let mut buf = vec![0u16; len + 1];
            let n = usize::try_from(GetWindowTextW(edit, &mut buf)).unwrap_or(0);
            String::from_utf16_lossy(&buf[..n])
        }
    }

    fn trace(edit: HWND, key: char) {
        if !verbose() {
            return;
        }
        let (mut start, mut end) = (0u32, 0u32);
        // SAFETY: EM_GETSEL writes two u32s through the pointers we pass.
        unsafe {
            SendMessageW(
                edit,
                EM_GETSEL,
                Some(WPARAM((&raw mut start) as usize)),
                Some(LPARAM((&raw mut end) as isize)),
            );
        }
        println!("    {key:?} -> {:?} caret {start}..{end}", text(edit));
    }

    fn key_input(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    // SAFETY: trivial Win32 call. Physical keys carry a scan code.
                    wScan: u16::try_from(unsafe {
                        MapVirtualKeyW(u32::from(vk.0), MAPVK_VK_TO_VSC)
                    })
                    .unwrap_or(0),
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn type_keys(window: HWND, edit: HWND, keys: &str) -> Result<()> {
        for c in keys.chars() {
            // SAFETY: trivial Win32 call.
            if unsafe { GetForegroundWindow() } != window {
                return Err("test window lost focus, stopped typing".into());
            }
            let (vk, shift) = match c {
                '\u{8}' => (VK_BACK, false),
                ' ' => (VK_SPACE, false),
                '\n' => (VK_RETURN, false),
                '\u{2190}' => (VK_LEFT, false),
                '.' => (VK_OEM_PERIOD, false),
                c if c.is_ascii_digit() => (VIRTUAL_KEY(u16::from(c as u8)), false),
                c if c.is_ascii_alphabetic() => (
                    VIRTUAL_KEY(u16::from(c.to_ascii_uppercase() as u8)),
                    c.is_ascii_uppercase(),
                ),
                c => return Err(format!("unsupported test key {c:?}")),
            };
            let mut inputs = Vec::new();
            if shift {
                inputs.push(key_input(VK_SHIFT, false));
            }
            inputs.push(key_input(vk, false));
            inputs.push(key_input(vk, true));
            if shift {
                inputs.push(key_input(VK_SHIFT, true));
            }
            let size = i32::try_from(size_of::<INPUT>()).expect("small struct");
            // SAFETY: well-formed keyboard inputs.
            unsafe { SendInput(&inputs, size) };
            pump(Duration::from_millis(40));
            trace(edit, c);
        }
        Ok(())
    }

    /// Click at the top-left of `edit`, moving the caret to the start of the text.
    fn click_start_of(window: HWND, edit: HWND) -> Result<()> {
        // SAFETY: trivial Win32 calls; the point is inside our own control.
        unsafe {
            if GetForegroundWindow() != window {
                return Err("test window lost focus, stopped".into());
            }
            let mut point = POINT { x: 1, y: 4 };
            let _ = ClientToScreen(edit, &raw mut point);
            SetCursorPos(point.x, point.y).map_err(|e| e.to_string())?;
            let press = |flags| INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dwFlags: flags,
                        ..Default::default()
                    },
                },
            };
            let size = i32::try_from(size_of::<INPUT>()).expect("small struct");
            SendInput(
                &[press(MOUSEEVENTF_LEFTDOWN), press(MOUSEEVENTF_LEFTUP)],
                size,
            );
        }
        Ok(())
    }

    /// Windows only lets the app the user is in hand over the foreground, which
    /// this tool is not when started from a script. Sharing input state with that
    /// app for one call lifts the restriction.
    fn bring_to_front(window: HWND) {
        // SAFETY: trivial Win32 calls on valid handles, attached only for this call.
        unsafe {
            let ours = GetCurrentThreadId();
            let theirs = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let attached =
                theirs != 0 && theirs != ours && AttachThreadInput(ours, theirs, true).as_bool();
            let _ = SetForegroundWindow(window);
            if attached {
                let _ = AttachThreadInput(ours, theirs, false);
            }
        }
    }

    fn create_windows(control: &str) -> windows_core::Result<(HWND, HWND)> {
        let class: PCWSTR = if matches!(control, "edit" | "password" | "vni") {
            w!("EDIT")
        } else {
            w!("RICHEDIT50W")
        };
        // SAFETY: Win32 setup calls on this (UI) thread.
        unsafe {
            LoadLibraryW(w!("Msftedit.dll"))?;
            let instance = GetModuleHandleW(None)?;
            let window_class = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance.into(),
                lpszClassName: w!("bikeySmokeTest"),
                ..Default::default()
            };
            RegisterClassW(&raw const window_class);
            let window = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("bikeySmokeTest"),
                w!("bikey smoke test - please don't type"),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                200,
                200,
                640,
                240,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            let edit = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!(""),
                WS_CHILD
                    | WS_VISIBLE
                    | WINDOW_STYLE(
                        u32::try_from(if control == "password" {
                            ES_PASSWORD
                        } else {
                            ES_MULTILINE
                        })
                        .unwrap_or(0),
                    ),
                10,
                10,
                600,
                170,
                Some(window),
                None,
                Some(instance.into()),
                None,
            )?;
            if control == "richedit-tsf" {
                let ctf = SES_USECTF as usize;
                SendMessageW(
                    edit,
                    EM_SETEDITSTYLE,
                    Some(WPARAM(ctf)),
                    Some(LPARAM(isize::try_from(ctf).unwrap_or(0))),
                );
            }
            Ok((window, edit))
        }
    }

    /// Switch this process to bikey, as the input switcher does, and confirm it took.
    fn activate_bikey() -> Result<()> {
        // SAFETY: plain COM calls with live interfaces and valid out pointers.
        unsafe {
            let profiles: ITfInputProcessorProfileMgr =
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)
                    .map_err(|e| e.to_string())?;
            profiles
                .ActivateProfile(
                    TF_PROFILETYPE_INPUTPROCESSOR,
                    0x042A,
                    &CLSID_BIKEY,
                    &PROFILE_BIKEY,
                    HKL::default(),
                    TF_IPPMF_FORPROCESS,
                )
                .map_err(|e| format!("cannot activate bikey (is it installed?): {e}"))?;
            pump(Duration::from_millis(300));
            let mut active = TF_INPUTPROCESSORPROFILE::default();
            profiles
                .GetActiveProfile(&GUID_TFCAT_TIP_KEYBOARD, &raw mut active)
                .map_err(|e| e.to_string())?;
            if active.clsid != CLSID_BIKEY {
                return Err("bikey did not become the active input method".into());
            }
        }
        Ok(())
    }

    pub fn run() -> Result<bool> {
        let control = std::env::args().nth(1).unwrap_or_else(|| "edit".into());
        if !["edit", "richedit", "richedit-tsf", "password", "vni"].contains(&control.as_str()) {
            return Err(format!("unknown control {control:?}"));
        }
        // SAFETY: first COM call on this thread.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .map_err(|e| e.to_string())?;
        let (window, edit) = create_windows(&control).map_err(|e| e.to_string())?;
        activate_bikey()?;
        bring_to_front(window);
        // SAFETY: our own control.
        let _ = unsafe { SetFocus(Some(edit)) };
        pump(Duration::from_millis(300));
        // SAFETY: trivial Win32 calls.
        let in_front = unsafe { GetForegroundWindow() == window && GetFocus() == edit };
        if !in_front {
            return Err("Windows would not bring the test window to the front".into());
        }

        // SAFETY: trivial Win32 call.
        let layout = unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout(0) };
        println!(
            "control: {control}, keyboard layout {:#x}",
            layout.0 as usize
        );
        let mut ok = true;
        let cases = match control.as_str() {
            "password" => PASSWORD_CASES,
            "vni" => VNI_CASES,
            _ => CASES,
        };
        for (index, (keys, want)) in cases.iter().enumerate() {
            // SAFETY: our own control.
            let _ = unsafe { SetWindowTextW(edit, w!("")) };
            pump(Duration::from_millis(50));
            type_keys(window, edit, keys)?;
            pump(Duration::from_millis(100));
            let got = text(edit);
            // Raw keys back on the first case: bikey was not converting at all.
            if control != "password" && index == 0 && got == *keys {
                return Err("bikey was not processing keys".into());
            }
            let pass = got == *want;
            ok &= pass;
            println!(
                "{} {:<14} -> {:?}{}",
                if pass { "ok  " } else { "FAIL" },
                keys.replace('\u{8}', "<BS>")
                    .replace('\n', "<Enter>")
                    .replace('\u{2190}', "<Left>"),
                got,
                if pass {
                    String::new()
                } else {
                    format!(" (want {want:?})")
                }
            );
        }
        if control != "password" {
            // Typing over selected text must replace it (the Explorer rename box).
            // SAFETY: our own control.
            unsafe {
                let _ = SetWindowTextW(edit, w!("old name"));
                SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
            }
            pump(Duration::from_millis(50));
            type_keys(
                window,
                edit,
                if control == "vni" {
                    "tie6ng1 "
                } else {
                    "tieengs "
                },
            )?;
            pump(Duration::from_millis(100));
            let got = text(edit);
            let pass = got == "tiếng ";
            ok &= pass;
            println!(
                "{} over selection -> {got:?}",
                if pass { "ok  " } else { "FAIL" }
            );

            // A real click mid-word must not delete anything: the next key starts
            // a new word at the new position. The click lands inside our own
            // control, at the far left so the caret goes to the start.
            if control != "vni" {
                // SAFETY: our own control.
                let _ = unsafe { SetWindowTextW(edit, w!("")) };
                pump(Duration::from_millis(50));
                type_keys(window, edit, "ddi")?;
                click_start_of(window, edit)?;
                pump(Duration::from_millis(100));
                type_keys(window, edit, "b ")?;
                pump(Duration::from_millis(100));
                let got = text(edit);
                let pass = got == "b đi";
                ok &= pass;
                println!(
                    "{} caret moved mid-word -> {got:?}{}",
                    if pass { "ok  " } else { "FAIL" },
                    if pass { "" } else { " (want \"b đi\")" }
                );
            }
        }
        // SAFETY: our own window.
        let _ = unsafe { DestroyWindow(window) };
        Ok(ok)
    }
}
