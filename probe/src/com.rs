//! COM 늦은 바인딩(IDispatch) 헬퍼.
//!
//! 한글·Excel 자동화 객체는 타입 라이브러리 없이 이름으로 부른다:
//! `get("PageCount")`, `put("Visible", false)`, `call("Open", &[path, "HWP", ""])`.
//! 모든 호출은 이 객체를 만든 스레드(STA)에서만 해야 한다 — `Dispatch` 는 `Send` 가 아니다.

use std::fmt;

use windows::core::{BSTR, GUID, HRESULT, PCWSTR};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::System::Com::{
    CLSIDFromProgID, CoCreateInstance, CoInitializeEx, CoUninitialize, IDispatch,
    CLSCTX_INPROC_SERVER, CLSCTX_LOCAL_SERVER, COINIT_APARTMENTTHREADED,
    COINIT_DISABLE_OLE1DDE, DISPATCH_FLAGS, DISPATCH_METHOD, DISPATCH_PROPERTYGET,
    DISPATCH_PROPERTYPUT, DISPPARAMS, EXCEPINFO,
};
use windows::Win32::System::Ole::DISPID_PROPERTYPUT;

const LOCALE_USER_DEFAULT: u32 = 0x0400;

/// COM 호출 실패
#[derive(Debug, Clone)]
pub struct ComError {
    pub hresult: i32,
    /// 무엇을 부르다 실패했는지 (예: "SaveAs")
    pub member: String,
    /// 서버가 준 설명(있으면) 또는 HRESULT 설명
    pub message: String,
}

impl ComError {
    pub fn hresult_hex(&self) -> String {
        format!("0x{:08X}", self.hresult as u32)
    }

    /// 서버(hwp.exe)가 사라져서 난 오류인지 — 시간 초과로 강제 종료했을 때 이렇게 돌아온다
    pub fn is_server_gone(&self) -> bool {
        matches!(
            self.hresult as u32,
            0x800706BA | // RPC_S_SERVER_UNAVAILABLE
            0x800706BE | // RPC_S_CALL_FAILED
            0x800706BF | // RPC_S_CALL_FAILED_DNE
            0x80010108 | // RPC_E_DISCONNECTED
            0x80010007 | // RPC_E_SERVER_DIED
            0x80010012 | // RPC_E_SERVER_DIED_DNE
            0x80080005   // CO_E_SERVER_EXEC_FAILURE (기동 실패)
        )
    }
}

impl fmt::Display for ComError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} 실패 ({}): {}", self.member, self.hresult_hex(), self.message)
    }
}

impl std::error::Error for ComError {}

fn com_err(member: &str, e: windows::core::Error) -> ComError {
    ComError {
        hresult: e.code().0,
        member: member.to_string(),
        message: e.message().trim().to_string(),
    }
}

/// 이 스레드에서 COM(STA) 을 초기화하고, 버려질 때 해제한다.
pub struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    pub fn init_sta() -> Result<Self, ComError> {
        // S_FALSE(이미 초기화됨)도 성공으로 본다
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        if hr.is_err() {
            return Err(ComError {
                hresult: hr.0,
                member: "CoInitializeEx".into(),
                message: hr.message().to_string(),
            });
        }
        Ok(Self { initialized: true })
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

/// IDispatch 를 이름으로 다루는 얇은 껍데기
pub struct Dispatch {
    inner: IDispatch,
}

impl Dispatch {
    /// ProgID 로 자동화 서버를 만든다 (프로세스 밖 서버면 그 프로세스가 뜬다).
    pub fn create(prog_id: &str) -> Result<Self, ComError> {
        let wide: Vec<u16> = prog_id.encode_utf16().chain(std::iter::once(0)).collect();
        let clsid: GUID = unsafe { CLSIDFromProgID(PCWSTR(wide.as_ptr())) }
            .map_err(|e| com_err("CLSIDFromProgID", e))?;
        let inner: IDispatch = unsafe {
            CoCreateInstance(&clsid, None, CLSCTX_LOCAL_SERVER | CLSCTX_INPROC_SERVER)
        }
        .map_err(|e| com_err("CoCreateInstance", e))?;
        Ok(Self { inner })
    }

    pub fn from_idispatch(inner: IDispatch) -> Self {
        Self { inner }
    }

    fn dispid(&self, name: &str) -> Result<i32, ComError> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let names = [PCWSTR(wide.as_ptr())];
        let mut id: i32 = 0;
        unsafe {
            self.inner
                .GetIDsOfNames(&GUID::zeroed(), names.as_ptr(), 1, LOCALE_USER_DEFAULT, &mut id)
        }
        .map_err(|e| ComError {
            hresult: e.code().0,
            member: name.to_string(),
            message: format!("그런 이름이 없습니다 ({})", e.message().trim()),
        })?;
        Ok(id)
    }

    fn invoke(&self, name: &str, flags: DISPATCH_FLAGS, args: &[VARIANT]) -> Result<VARIANT, ComError> {
        let id = self.dispid(name)?;
        // COM 은 인자를 거꾸로 받는다
        let mut rev: Vec<VARIANT> = args.iter().rev().cloned().collect();
        let mut named = DISPID_PROPERTYPUT;
        let is_put = flags == DISPATCH_PROPERTYPUT;
        let params = DISPPARAMS {
            rgvarg: if rev.is_empty() { std::ptr::null_mut() } else { rev.as_mut_ptr() },
            rgdispidNamedArgs: if is_put { &mut named } else { std::ptr::null_mut() },
            cArgs: rev.len() as u32,
            cNamedArgs: if is_put { 1 } else { 0 },
        };
        let mut result = VARIANT::default();
        let mut excep = EXCEPINFO::default();
        let mut arg_err: u32 = 0;
        let r = unsafe {
            self.inner.Invoke(
                id,
                &GUID::zeroed(),
                LOCALE_USER_DEFAULT,
                flags,
                &params,
                Some(&mut result),
                Some(&mut excep),
                Some(&mut arg_err),
            )
        };
        match r {
            Ok(()) => Ok(result),
            Err(e) => {
                let mut message = e.message().trim().to_string();
                // DISP_E_EXCEPTION 이면 서버가 준 설명이 더 쓸모 있다
                if e.code() == HRESULT(0x80020009u32 as i32) {
                    let desc = excep.bstrDescription.to_string();
                    let src = excep.bstrSource.to_string();
                    if !desc.is_empty() {
                        message = if src.is_empty() { desc } else { format!("{src}: {desc}") };
                    }
                    let scode = if excep.scode != 0 { excep.scode } else { excep.wCode as i32 };
                    return Err(ComError {
                        hresult: if scode != 0 { scode } else { e.code().0 },
                        member: name.to_string(),
                        message,
                    });
                }
                Err(ComError {
                    hresult: e.code().0,
                    member: name.to_string(),
                    message,
                })
            }
        }
    }

    pub fn get(&self, name: &str) -> Result<VARIANT, ComError> {
        self.invoke(name, DISPATCH_PROPERTYGET, &[])
    }

    pub fn put(&self, name: &str, value: VARIANT) -> Result<(), ComError> {
        self.invoke(name, DISPATCH_PROPERTYPUT, &[value]).map(|_| ())
    }

    pub fn call(&self, name: &str, args: &[VARIANT]) -> Result<VARIANT, ComError> {
        // 메서드 없이 프로퍼티만 있는 멤버도 있어 둘 다 허용한다
        self.invoke(name, DISPATCH_METHOD | DISPATCH_PROPERTYGET, args)
    }

    /// 자식 객체 (예: `HParameterSet`, `XHwpWindows`)
    pub fn get_object(&self, name: &str) -> Result<Dispatch, ComError> {
        let v = self.get(name)?;
        to_dispatch(name, &v)
    }

    pub fn call_object(&self, name: &str, args: &[VARIANT]) -> Result<Dispatch, ComError> {
        let v = self.call(name, args)?;
        to_dispatch(name, &v)
    }
}

fn to_dispatch(name: &str, v: &VARIANT) -> Result<Dispatch, ComError> {
    let d: IDispatch = IDispatch::try_from(v).map_err(|e| ComError {
        hresult: e.code().0,
        member: name.to_string(),
        message: "객체가 아닙니다".to_string(),
    })?;
    Ok(Dispatch::from_idispatch(d))
}

// ---------------------------------------------------------------------------
// VARIANT 변환 도우미
// ---------------------------------------------------------------------------

pub fn v_str(s: &str) -> VARIANT {
    VARIANT::from(s)
}

pub fn v_i32(n: i32) -> VARIANT {
    VARIANT::from(n)
}

pub fn v_bool(b: bool) -> VARIANT {
    VARIANT::from(b)
}

/// 넘기지 않은 인자 (COM 의 "생략" 표시). 가운데 인자를 건너뛸 때 쓴다.
pub fn v_missing() -> VARIANT {
    use windows::Win32::System::Variant::{VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_ERROR};
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: std::mem::ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_ERROR,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                // DISP_E_PARAMNOTFOUND
                Anonymous: VARIANT_0_0_0 { scode: 0x8002_0004u32 as i32 },
            }),
        },
    }
}

/// VARIANT 의 숫자·불 값을 직접 읽는다.
///
/// `windows` 크레이트의 `bool::try_from` 은 VARIANT_TRUE(-1) 만 참으로 보는데,
/// **한글은 참을 1 로 돌려준다** (VT_BOOL, 값 1). 그래서 vt 를 보고 원시 값을 읽는다.
fn raw_number(v: &VARIANT) -> Option<f64> {
    use windows::Win32::System::Variant::{
        VT_BOOL, VT_I2, VT_I4, VT_I8, VT_INT, VT_R4, VT_R8, VT_UI2, VT_UI4, VT_UI8, VT_UINT,
    };
    unsafe {
        let inner = &*v.Anonymous.Anonymous;
        let u = &inner.Anonymous;
        let vt = inner.vt;
        Some(match vt {
            x if x == VT_BOOL => u.boolVal.0 as f64,
            x if x == VT_I4 || x == VT_INT => u.lVal as f64,
            x if x == VT_I2 => u.iVal as f64,
            x if x == VT_UI4 || x == VT_UINT => u.ulVal as f64,
            x if x == VT_UI2 => u.uiVal as f64,
            x if x == VT_I8 => u.llVal as f64,
            x if x == VT_UI8 => u.ullVal as f64,
            x if x == VT_R8 => u.dblVal,
            x if x == VT_R4 => u.fltVal as f64,
            _ => return None,
        })
    }
}

/// 서버가 돌려준 값을 bool 로 (VARIANT_BOOL · 정수 모두 허용, 0 이 아니면 참)
pub fn as_bool(v: &VARIANT) -> bool {
    match raw_number(v) {
        Some(n) => n != 0.0,
        None => bool::try_from(v).unwrap_or(false),
    }
}

pub fn as_i32(v: &VARIANT) -> Option<i32> {
    raw_number(v).map(|n| n as i32)
}

pub fn as_string(v: &VARIANT) -> String {
    if let Ok(b) = BSTR::try_from(v) {
        return b.to_string();
    }
    v.to_string()
}

impl Dispatch {
    /// 다른 호출의 인자로 넘길 때 (예: `GetDefault(set)`)
    pub fn as_variant(&self) -> VARIANT {
        VARIANT::from(self.inner.clone())
    }
}

