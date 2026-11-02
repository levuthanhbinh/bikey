//! Display attributes: tells apps to draw the word being typed with a dotted
//! underline (TSF expects every text service that makes compositions to
//! provide these).

use std::cell::Cell;

use windows::Win32::Foundation::{E_INVALIDARG, E_NOTIMPL, S_FALSE};
use windows::Win32::UI::TextServices::{
    IEnumTfDisplayAttributeInfo, IEnumTfDisplayAttributeInfo_Impl, ITfDisplayAttributeInfo,
    ITfDisplayAttributeInfo_Impl, TF_ATTR_INPUT, TF_CT_NONE, TF_DA_COLOR, TF_DISPLAYATTRIBUTE,
    TF_LS_DOT,
};
use windows_core::{BSTR, GUID, Result, implement};

use crate::guard;

/// The "being typed" style.
pub const GUID_ATTR_INPUT: GUID = GUID::from_u128(0xeb54d707_bc41_4e83_b940_78541849a59c);

fn input_attribute() -> TF_DISPLAYATTRIBUTE {
    let none = TF_DA_COLOR {
        r#type: TF_CT_NONE,
        ..Default::default()
    };
    TF_DISPLAYATTRIBUTE {
        crText: none,
        crBk: none,
        lsStyle: TF_LS_DOT,
        fBoldLine: false.into(),
        crLine: none,
        bAttr: TF_ATTR_INPUT,
    }
}

#[implement(ITfDisplayAttributeInfo)]
#[derive(Debug)]
pub struct InputAttribute;

impl ITfDisplayAttributeInfo_Impl for InputAttribute_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(GUID_ATTR_INPUT)
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("bikey input"))
    }

    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        if pda.is_null() {
            return Err(E_INVALIDARG.into());
        }
        // SAFETY: non-null out pointer supplied by TSF.
        unsafe { pda.write(input_attribute()) };
        Ok(())
    }

    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> {
        Err(E_NOTIMPL.into())
    }

    fn Reset(&self) -> Result<()> {
        Ok(())
    }
}

/// Enumerates bikey's single display attribute.
#[implement(IEnumTfDisplayAttributeInfo)]
#[derive(Debug, Default)]
pub struct AttributeEnum {
    position: Cell<u32>,
}

impl IEnumTfDisplayAttributeInfo_Impl for AttributeEnum_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        Ok(AttributeEnum {
            position: Cell::new(self.position.get()),
        }
        .into())
    }

    fn Next(
        &self,
        count: u32,
        info: *mut Option<ITfDisplayAttributeInfo>,
        fetched: *mut u32,
    ) -> Result<()> {
        guard(|| {
            if info.is_null() {
                return Err(E_INVALIDARG.into());
            }
            let give = u32::from(count > 0 && self.position.get() == 0);
            if give == 1 {
                // SAFETY: TSF passes room for at least `count` (> 0) entries.
                unsafe { info.write(Some(InputAttribute.into())) };
                self.position.set(1);
            }
            if !fetched.is_null() {
                // SAFETY: non-null out pointer.
                unsafe { fetched.write(give) };
            }
            if give == count {
                Ok(())
            } else {
                Err(S_FALSE.into())
            }
        })
    }

    fn Reset(&self) -> Result<()> {
        self.position.set(0);
        Ok(())
    }

    fn Skip(&self, count: u32) -> Result<()> {
        let skipped = self.position.get().saturating_add(count).min(1);
        self.position.set(skipped);
        Ok(())
    }
}
