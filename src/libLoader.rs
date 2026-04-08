#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use crate::defs::*;
use crate::win_api::{FreeLibrary, GetLastError, GetProcAddress, LoadLibraryW, FARPROC};
use std::os::raw::{c_char, c_short, c_void};

type HMODULE = *mut c_void;

/// Simple wrapper to manage the lifecycle of a Windows DLL handle.
pub struct Library {
    handle: HMODULE,
}

impl Library {
    /// Loads a library from the specified path.
    pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        unsafe {
            let mut path_encoded: Vec<u16> = path.encode_utf16().collect();
            path_encoded.push(0);

            let h_lib = LoadLibraryW(path_encoded.as_ptr());
            if h_lib.is_null() {
                let err_code = GetLastError();
                return Err(format!(
                    "Failed to load DLL at: {} (Windows Error Code: {})",
                    path, err_code
                )
                .into());
            }
            Ok(Self { handle: h_lib })
        }
    }

    pub fn handle(&self) -> HMODULE {
        self.handle
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { FreeLibrary(self.handle) };
        }
    }
}

// Safety traits for cross-thread usage.
unsafe impl Send for Library {}
unsafe impl Sync for Library {}

macro_rules! define_eci_api {
    ($( $name:ident ( $($arg_name:ident : $arg_ty:ty),* ) $(-> $ret:ty)? ; )*) => {

        // Define function pointer types.
        $(
            pub type $name = unsafe extern "system" fn($($arg_ty),*) $(-> $ret)?;
        )*

        pub struct EciApi {
            $( pub $name: $name, )*
            _lib: Library,
        }

        unsafe impl Send for EciApi {}
        unsafe impl Sync for EciApi {}

        impl EciApi {
            pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
                let lib = Library::load(path)?;
                let h_lib = lib.handle();

                unsafe {
                    Ok(EciApi {
                        $(
                            $name: {
                                let sym = GetProcAddress(h_lib, concat!(stringify!($name), "\0").as_ptr() as *const c_char);
                                if sym.is_null() {
                                    let err_code = GetLastError();
                                    return Err(format!(
                                        "Symbol not found: {} (Windows Error Code: {})",
                                        stringify!($name), err_code
                                    ).into());
                                }
                                std::mem::transmute::<FARPROC, $name>(sym)
                            },
                        )*
                        _lib: lib,
                    })
                }
            }
        }
    };
}

// Api definitions.
define_eci_api! {
    eciSpeakTextEx(pText: ECIInputText, bAnnotationsInTextPhrase: i32, ECILanguageDialect: i32) -> i32;
    eciVersion(pBuffer: *mut c_char);
    eciGetAvailableLanguages(aLanguages: *mut i32, nLanguages: *mut i32) -> i32;
    eciNew() -> ECIHand;
    eciNewEx(Value: i32) -> ECIHand;
    eciDelete(hEngine: ECIHand) -> ECIHand;
    eciRegisterCallback(hEngine: ECIHand, Callback: EciCallback, pData: *mut c_void);
    eciSetOutputBuffer(hEngine: ECIHand, iSize: i32, psBuffer: *mut c_short) -> i32;
    eciAddText(hEngine: ECIHand, pText: ECIInputText) -> i32;
    eciSynthesize(hEngine: ECIHand) -> i32;
    eciStop(hEngine: ECIHand) -> i32;
    eciGetParam(hEngine: ECIHand, Param: i32) -> i32;
    eciSetParam(hEngine: ECIHand, Param: i32, iValue: i32) -> i32;
    eciGetVoiceParam(hEngine: ECIHand, iVoice: i32, Param: i32) -> i32;
    eciSetVoiceParam(hEngine: ECIHand, iVoice: i32, Param: i32, iValue: i32) -> i32;
    eciCopyVoice(hEngine: ECIHand, iVoiceFrom: i32, iVoiceTo: i32) -> i32;
    eciInsertIndex(hEngine: ECIHand, iIndex: i32) -> i32;
    eciSynchronize(hEngine: ECIHand) -> i32;
    eciNewDict(hEngine: ECIHand) -> ECIDictHand;
    eciLoadDict(hEngine: ECIHand, hDict: ECIDictHand, DictVol: i32, pFilename: *const c_void) -> i32;
    eciSetDict(hEngine: ECIHand, hDict: ECIDictHand) -> i32;
    eciSetOutputDevice(hEngine: ECIHand, iDevNum: i32) -> i32;
}
