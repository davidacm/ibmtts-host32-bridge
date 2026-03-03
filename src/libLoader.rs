#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use libloading::{Library};
use std::sync::Arc;
use std::os::raw::{c_char, c_short, c_void};

use super::defs::*;

macro_rules! define_eci_api {
    ($( $name:ident ( $($arg_name:ident : $arg_ty:ty),* ) $(-> $ret:ty)? ; )*) => {
        
        // 1. generating the types of function pointers
        $(
            pub type $name = unsafe extern "system" fn($($arg_ty),*) $(-> $ret)?;
        )*

        // defining the Struct containing the pointers as public fields
        pub struct EciApi {
            $( pub $name: $name, )*
            _lib: Arc<Library>,
        }

        impl EciApi {
            pub fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
                unsafe {
                    let lib = Arc::new(Library::new(path)?);
                    
                    Ok(EciApi {
                        $(
                            // load each symbol using its name as a string
                            $name: *lib.get(concat!(stringify!($name), "\0").as_bytes())?,
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