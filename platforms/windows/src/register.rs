//! `DllRegisterServer` / `DllUnregisterServer`: COM class + TSF profile.
//!
//! Run by the installer with administrator rights. The 32-bit DLL is
//! registered the same way from a 32-bit process (WOW64 redirects the
//! registry keys), so 32-bit apps get bikey too.

use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, RPC_E_CHANGED_MODE};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoUninitialize,
};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_LOCAL_MACHINE, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_CategoryMgr, CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT, GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED, ITfCategoryMgr, ITfInputProcessorProfileMgr,
};
use windows_core::{GUID, HSTRING, PCWSTR, Result};

use crate::tsf::wide;
use crate::{CLSID_BIKEY, PROFILE_BIKEY, module};

/// Vietnamese (Vietnam).
const LANG_VI_VN: u16 = 0x042A;
/// Keyboard layout handle: US keys (high word) under Vietnamese (low word).
const LAYOUT_US: usize = 0x0409_042A;

/// Keyboard text service, works in Store/immersive apps and the system tray,
/// and has no UI of its own (so it is fine in UI-less mode). Deliberately not
/// `SECUREMODE`: bikey is not loaded on the secure desktop (UAC, sign-in).
const CATEGORIES: [GUID; 4] = [
    GUID_TFCAT_TIP_KEYBOARD,
    GUID_TFCAT_TIPCAP_IMMERSIVESUPPORT,
    GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT,
    GUID_TFCAT_TIPCAP_UIELEMENTENABLED,
];

fn clsid_key() -> String {
    format!("Software\\Classes\\CLSID\\{{{CLSID_BIKEY:?}}}")
}

fn module_path() -> Result<String> {
    let mut buf = vec![0u16; 32_768];
    // SAFETY: valid buffer; `module()` is this DLL's handle.
    let len = unsafe { GetModuleFileNameW(Some(module().into()), &mut buf) } as usize;
    if len == 0 || len >= buf.len() {
        return Err(windows_core::Error::from_thread());
    }
    Ok(String::from_utf16_lossy(&buf[..len]))
}

/// Run `f` with COM initialised on this thread.
fn with_com<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
    // SAFETY: balanced with CoUninitialize below when we initialised it.
    let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let ours = hr.is_ok();
    if !ours && hr != RPC_E_CHANGED_MODE {
        hr.ok()?;
    }
    let result = f();
    if ours {
        // SAFETY: pairs with the successful CoInitializeEx above.
        unsafe { CoUninitialize() };
    }
    result
}

fn set_string(key: HKEY, name: Option<&str>, value: &str) -> Result<()> {
    let name = name.map(HSTRING::from);
    let mut data: Vec<u8> = wide(value).iter().flat_map(|u| u.to_le_bytes()).collect();
    data.extend_from_slice(&[0, 0]);
    // SAFETY: `key` is open for writing; `data` is a NUL-terminated UTF-16 string.
    unsafe {
        RegSetValueExW(
            key,
            name.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr())),
            None,
            REG_SZ,
            Some(&data),
        )
    }
    .ok()
}

fn register_com_class(dll: &str) -> Result<()> {
    let path = HSTRING::from(format!("{}\\InprocServer32", clsid_key()));
    let mut key = HKEY::default();
    // SAFETY: creating/opening a key we then close below.
    unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            &path,
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &raw mut key,
            None,
        )
    }
    .ok()?;
    let result = set_string(key, None, dll)
        .and_then(|()| set_string(key, Some("ThreadingModel"), "Apartment"));
    // SAFETY: `key` was opened above.
    let _ = unsafe { RegCloseKey(key) };
    result
}

pub fn register() -> Result<()> {
    let dll = module_path()?;
    register_com_class(&dll)?;
    with_com(|| {
        // SAFETY: plain COM calls.
        unsafe {
            let profiles: ITfInputProcessorProfileMgr =
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?;
            profiles.RegisterProfile(
                &CLSID_BIKEY,
                LANG_VI_VN,
                &PROFILE_BIKEY,
                &wide("bikey"),
                &[],
                0,
                // Keep the US layout underneath. The Windows Vietnamese layout
                // turns the number row into letters, breaking digits and VNI.
                HKL(LAYOUT_US as *mut core::ffi::c_void),
                0,
                true,
                0,
            )?;
            let categories: ITfCategoryMgr =
                CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)?;
            for category in &CATEGORIES {
                categories.RegisterCategory(&CLSID_BIKEY, category, &CLSID_BIKEY)?;
            }
        }
        Ok(())
    })
}

/// Best effort: removes whatever is there and ignores what is already gone.
pub fn unregister() -> Result<()> {
    let _ = with_com(|| {
        // SAFETY: plain COM calls; errors mean "not registered".
        unsafe {
            if let Ok(profiles) = CoCreateInstance::<_, ITfInputProcessorProfileMgr>(
                &CLSID_TF_InputProcessorProfiles,
                None,
                CLSCTX_INPROC_SERVER,
            ) {
                let _ = profiles.UnregisterProfile(&CLSID_BIKEY, LANG_VI_VN, &PROFILE_BIKEY, 0);
            }
            if let Ok(categories) = CoCreateInstance::<_, ITfCategoryMgr>(
                &CLSID_TF_CategoryMgr,
                None,
                CLSCTX_INPROC_SERVER,
            ) {
                for category in &CATEGORIES {
                    let _ = categories.UnregisterCategory(&CLSID_BIKEY, category, &CLSID_BIKEY);
                }
            }
        }
        Ok(())
    });
    let path = HSTRING::from(clsid_key());
    // SAFETY: deleting our own CLSID subtree.
    let status = unsafe { RegDeleteTreeW(HKEY_LOCAL_MACHINE, &path) };
    if status.is_ok() || status == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        status.ok()
    }
}
