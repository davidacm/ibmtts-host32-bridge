use std::sync::{OnceLock};
use std::sync::{LazyLock, Mutex};
use std::collections::HashMap;
use worker_macros::api;
use crate::shared_memory::{SharedHeader, SharedMemory};
use crate::libLoader::EciApi;
use crate::defs::{ECICallbackReturn, ECIDictHand };
use std::os::raw::c_char;
use crate::defs::{ECIHand, ECIInputText};
use windows::Win32::Foundation::WAIT_OBJECT_0;
use core::ffi::c_void;
use std::cell::RefCell;
use windows::Win32::System::Threading::{SetEvent, WaitForSingleObject};

// --- Protocol Constants ---
/// Offset where the 2 bytes of the message ID end and the parameters begin
const REQ_PARAMS_OFFSET: usize = 2;
/// Buffer size to obtain the library version
const VERSION_BUFFER_SIZE: usize= 256;

struct EciHandleGuard {
    handle: ECIHand,
}

impl Drop for EciHandleGuard {
    fn drop(&mut self) {
        if let Some(api) = eci() {
            unsafe {
                //(api.eciDelete)(self.handle);
                // todo: eciDelete blocks the finishing process, I guess the library was unloaded so the call is failing.
            }
        }
    }
}

thread_local! {
    pub static CURRENT_SHM: RefCell<Option<SharedMemory>> = RefCell::new(None);
    pub static CURRENT_ECI_GUARD: RefCell<Option<EciHandleGuard>> = RefCell::new(None);
}

static ECI_API: OnceLock<EciApi> = OnceLock::new();
fn eci() -> Option<&'static EciApi> {
    ECI_API.get()
}

// --- Request parsing helpers ---
pub struct RequestContext<'a> {
    data: &'a [u8],
}

impl<'a> RequestContext<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    // Read 4-byte little-endian int parameter at index `pos` (0-based)
    pub fn get_int(&self, pos: usize) -> i32 {
        let off = (pos).checked_mul(4).unwrap_or(usize::MAX)+REQ_PARAMS_OFFSET;
        if off + 4 > self.data.len() {
            return 0;
        }
        let b = &self.data[off..off + 4];
        i32::from_le_bytes([b[0], b[1], b[2], b[3]])
    }

    // Read 4-byte parameter and return as i16 (lower 2 bytes)
    pub fn get_short(&self, pos: usize) -> i16 {
        let val = self.get_int(pos);
        val as i16
    }

    // Read parameter at `pos` as offset (u32) into the request data, then
    // return bytes of the C-style null-terminated string starting at that offset, including the end of string (0).
    // If no null terminator is found, returns bytes until end.
    pub fn get_string(&self, pos: usize) -> &'a [u8] {
        let off = self.get_int(pos) as usize;

        // Case 1: Offset out of bounds -> we return a static slice with only the NUL
        if off >= self.data.len() {
            return &[0]; 
        }

        let bytes_from_off = &self.data[off..];
        
        // Case 2: Find the null terminator
        if let Some(nul_pos) = bytes_from_off.iter().position(|&b| b == 0) {
            // return the slice including the 0
            &bytes_from_off[..=nul_pos]
        } else {
            // Case 3: There is no 0 (should not happen with well-defined clients)
            // return everything that remains until the end of the buffer
            bytes_from_off
        }
    }

    pub fn get_body(&self) -> &[u8] {
        &self.data[2..]
    }
}

// Response data types. Matches `client.parse_response` plus UTF-8 string type (3).
#[derive(Debug, Copy, Clone)]
pub enum ResponseType {
    SignedInt = 0,
    UnsignedInt = 1,
    ByteString = 2,
    Utf8String = 3,
}

// Pack a signed 32-bit integer response: first byte = type, then 4-byte LE int.
pub fn pack_int(v: i32) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 4);
    out.push(ResponseType::SignedInt as u8);
    out.extend_from_slice(&v.to_le_bytes());
    out
}

// Pack an unsigned 32-bit integer response: first byte = type, then 4-byte LE uint.
pub fn pack_uint(v: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 4);
    out.push(ResponseType::UnsignedInt as u8);
    out.extend_from_slice(&v.to_le_bytes());
    out
}

// Pack raw bytes: first byte = type, then bytes unchanged.
pub fn pack_bytes(mut b: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + b.len());
    out.push(ResponseType::ByteString as u8);
    out.append(&mut b);
    out
}

// Pack a UTF-8 string: first byte = Utf8String (3), then the UTF-8 bytes (no modification).
pub fn pack_utf_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(1 + bytes.len());
    out.push(ResponseType::Utf8String as u8);
    out.extend_from_slice(bytes);
    out
}


pub type Handler = fn(&RequestContext) -> Vec<u8>;

static REGISTRY: LazyLock<Mutex<HashMap<u16, Handler>>> = LazyLock::new(|| {
    Mutex::new(HashMap::new())
});

pub fn register_handler(id: u16, h: Handler) {
    if let Ok(mut map) = REGISTRY.lock() {
        map.insert(id, h);
    }
}

pub fn handle_request(request: &[u8]) -> Vec<u8> {
    // Expects request to be full payload with first two bytes as u16 id (little-endian)
    if request.len() < 2 {
        return pack_utf_string("ERR:short request");
    }
    let id = u16::from_le_bytes([request[0], request[1]]);
    let ctx = RequestContext::new(request);
    let handler = {
        let map = REGISTRY.lock().unwrap();
        map.get(&id).copied()
    };
    if let Some(h) = handler {
        h(&ctx)
    } else {
        let mut v = b"ERR:unknown id ".to_vec();
        v.extend_from_slice(&id.to_le_bytes());
        pack_bytes(v)
    }
}


// ECI callback invoked in the thread where it was registered.
unsafe extern "system" fn eci_callback(
    h_engine: ECIHand, 
    msg: u32, 
    lparam: i32, 
    _p_data: *mut c_void
) -> ECICallbackReturn {
    CURRENT_SHM.with(|cell| {
        if let Some(ref shm) = *cell.borrow() {
            // 1. Prepare 12-byte Header
            let header = &mut *(shm.view as *mut SharedHeader);
            header.h_engine = h_engine as u32;
            header.msg = msg;
            header.lparam = lparam;

            // 2. Notify the Client
            SetEvent(shm.h_evt_ready);

            // 3. Wait for the Client (maximum 5000ms for sure, typically this should be very fast)
            let wait = WaitForSingleObject(shm.h_evt_processed, 5000);
            if wait == WAIT_OBJECT_0 {
                // 4. The client has written the result in the first 4 bytes (h_engine)
                let result_code = *(shm.view as *mut u32);
                return match result_code {
                    1 => ECICallbackReturn::eciDataProcessed,
                    2 => ECICallbackReturn::eciDataAbort,
                    _ => ECICallbackReturn::eciDataNotProcessed,
                };
            }
        }
        ECICallbackReturn::eciDataAbort
    })
}

// Define handlers using the attribute-style proc-macro from `worker_macros`.
// Example:
// #[api(1)]
// fn hello(req: &RequestContext) -> Vec<u8> { ... }


#[api(1)]
fn load_library(req: &RequestContext) -> Vec<u8> {
    let path_bytes = req.get_body();
    let path_str = String::from_utf8_lossy(path_bytes).trim_matches('\0').to_string();

    // try to initialize the global API only once
    if ECI_API.get().is_some() {
        return pack_int(1); // Already loaded
    }

    match EciApi::load(&path_str) {
        Ok(api) => {
            let _ = ECI_API.set(api); // If the set fails, it's because another thread won the race
            pack_int(1)
        }
        Err(e) => pack_utf_string(&format!("ERR:load:{}", e)),
    }
}

#[api(2)]
fn eci_version(_req: &RequestContext) -> Vec<u8> {
    // Allocate buffer for version string
    let mut buf = vec![0u8; VERSION_BUFFER_SIZE];
    if let Some(api) = eci() {
        unsafe { (api.eciVersion)(buf.as_mut_ptr() as *mut c_char) };
        // find null terminator
        let nul_pos = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        buf.truncate(nul_pos);
        pack_bytes(buf)
    } else {
        pack_utf_string("ERR:no lib")
    }
}


#[api(3)]
fn eci_new(_ctx: &RequestContext) -> Vec<u8> {
    if let Some(api) = eci() {
        let handle = unsafe { (api.eciNew)() };
        CURRENT_ECI_GUARD.with(|cell| {
            *cell.borrow_mut() = Some(EciHandleGuard { 
                handle, 
            });
        });
        pack_uint(handle as u32)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(4)]
fn eci_new_ex(ctx: &RequestContext) -> Vec<u8> {
    // Expect first parameter as 4-byte int (language dialect enum)
    let val = ctx.get_int(0) as i32;
    if let Some(api) = eci() {
        let handle: ECIHand = unsafe { (api.eciNewEx)(val) };
        pack_uint(handle as u32)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(5)]
fn set_buffer(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as u32;
    let samples = ctx.get_int(1) as u32 as usize;
    
    // calculate size: samples * 2 (i16) + 12 bytes header
    let size = (samples * 2).saturating_add(12);
    if let Some(api) = eci() {
        match unsafe { SharedMemory::create(eci_handle, size) } {
            Ok(shm) => {
                unsafe {
                    let eci_buf_ptr = shm.get_eci_buffer_ptr();
                    (api.eciRegisterCallback)(eci_handle as ECIHand, eci_callback, std::ptr::null_mut());
                    // Configure Output Buffer (ECI writes from byte 12)
                    (api.eciSetOutputBuffer)(eci_handle as ECIHand, samples as i32, eci_buf_ptr);

                    // Save in TLS for callback
                    let shm_name = format!("Local\\eci_shm_{:x}", eci_handle);
                    let evt_name = format!("Local\\eci_ready_{:x}", eci_handle);
                    
                    CURRENT_SHM.with(|cell| *cell.borrow_mut() = Some(shm));

                    let resp = format!("SHM:{};EVT:{}", shm_name, evt_name);
                    pack_utf_string(&resp)
                }
            },
            Err(e) => pack_utf_string(&format!("ERR:{}", e))
        }
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(6)]
fn eci_add_text(ctx: &RequestContext) -> Vec<u8> {
    // param 0: eciHandle (as 32-bit value)
    // param 1: pointer/offset to text string
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    let text_bytes = ctx.get_string(1);
    if let Some(api) = eci() {
        let result = unsafe {
            (api.eciAddText)(eci_handle, text_bytes.as_ptr() as ECIInputText)
        };
        pack_int(result)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(7)]
fn eci_insert_index(ctx: &RequestContext) -> Vec<u8> {
    // param 0: eciHandle (as 32-bit value)
    // param 1: index value
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    let index = ctx.get_int(1);
    if let Some(api) = eci() {
        let result = unsafe {
            (api.eciInsertIndex)(eci_handle, index)
        };
        pack_int(result as i32)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(8)]
fn eci_synthesize(ctx: &RequestContext) -> Vec<u8> {
    // param 0: eciHandle (as 32-bit value)
    let eci_handle = ctx.get_int(0) as usize as ECIHand;

    if let Some(api) = eci() {
        let result = unsafe { (api.eciSynthesize)(eci_handle) };
        pack_int(result as i32)
    } else {
        pack_utf_string("ERR:no lib")
    }
}



#[api(9)]
fn eci_get_available_languages(_ctx: &RequestContext) -> Vec<u8> {
    if let Some(api) = eci() {
        unsafe {
            let mut n_langs: i32 = 0;
            (api.eciGetAvailableLanguages)(std::ptr::null_mut(), &mut n_langs);

            if n_langs <= 0 {
                return pack_int(n_langs);
            }
            let mut langs = vec![0i32; n_langs as usize];
            let result = (api.eciGetAvailableLanguages)(
                langs.as_mut_ptr() as *mut i32, 
                &mut n_langs
            );

            if result >= 0 {
                let mut out = Vec::with_capacity(4 + (langs.len() * 4));
                out.extend_from_slice(&(langs.len() as i32).to_le_bytes());
                for lang in langs {
                    out.extend_from_slice(&lang.to_le_bytes());
                }
                pack_bytes(out)
            } else {
                pack_int(result)
            }
        }
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(10)]
fn eci_stop(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    if let Some(api) = eci() {
        let result = unsafe { (api.eciStop)(eci_handle) };
        pack_int(result)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(11)]
fn eci_get_param(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    let param_id = ctx.get_int(1);
    if let Some(api) = eci() {
        let result = unsafe { (api.eciGetParam)(eci_handle, param_id) };
        pack_int(result)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(12)]
fn eci_set_param(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    let param_id = ctx.get_int(1);
    let value = ctx.get_int(2);
    if let Some(api) = eci() {
        let result = unsafe { (api.eciSetParam)(eci_handle, param_id, value) };
        pack_int(result)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(13)]
fn eci_get_voice_param(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    let voice_idx = ctx.get_int(1);
    let param_id = ctx.get_int(2);
    if let Some(api) = eci() {
        let result = unsafe { (api.eciGetVoiceParam)(eci_handle, voice_idx, param_id) };
        pack_int(result)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(14)]
fn eci_set_voice_param(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    let voice_id = ctx.get_int(1);
    let param_id = ctx.get_int(2);
    let value = ctx.get_int(3);
    if let Some(api) = eci() {
        let result = unsafe { (api.eciSetVoiceParam)(eci_handle, voice_id, param_id, value) };
        pack_int(result)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(15)]
fn eci_copy_voice(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    let from_idx = ctx.get_int(1);
    let to_idx = ctx.get_int(2);
    if let Some(api) = eci() {
        let result = unsafe { (api.eciCopyVoice)(eci_handle, from_idx, to_idx) };
        pack_int(result)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(16)]
fn eci_new_dict(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    if let Some(api) = eci() {
        let dict_handle = unsafe { (api.eciNewDict)(eci_handle) };
        pack_uint(dict_handle as u32)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(17)]
fn eci_load_dict(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    let dict_handle = ctx.get_int(1) as usize as ECIDictHand;
    let dict_vol = ctx.get_int(2);
    let filename_bytes = ctx.get_string(3);
    
    if let Some(api) = eci() {
        let result = unsafe {
            (api.eciLoadDict)(eci_handle, dict_handle,  dict_vol, filename_bytes.as_ptr() as *const c_void)
        };
        pack_int(result as i32)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(18)]
fn eci_set_dict(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    let dict_handle = ctx.get_int(1) as usize as ECIDictHand;
    if let Some(api) = eci() {
        let result = unsafe { (api.eciSetDict)(eci_handle, dict_handle) };
        pack_int(result as i32)
    } else {
        pack_utf_string("ERR:no lib")
    }
}

#[api(19)]
fn eci_delete(ctx: &RequestContext) -> Vec<u8> {
    let eci_handle = ctx.get_int(0) as usize as ECIHand;
    if let Some(api) = eci() {
        let result = unsafe { (api.eciDelete)(eci_handle) as i32};
        pack_int(result)
    } else {
        pack_utf_string("ERR:no lib")
    }
}
