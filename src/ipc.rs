use std::sync::atomic::{AtomicBool, Ordering};
use std::ffi::OsStr;
use std::os::windows::prelude::OsStrExt;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_BROKEN_PIPE, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{WriteFile, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_OVERLAPPED, ReadFileEx};
use windows::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, NAMED_PIPE_MODE, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_WAIT, PIPE_UNLIMITED_INSTANCES};
use windows::Win32::System::IO::OVERLAPPED;
#[repr(C)]
pub struct PipeContext {
    pub overlapped: OVERLAPPED,
    pub handle: HANDLE,
    pub buffer: [u8; 65536],
    pub alive: AtomicBool,
}

/// Rutina de completado que Windows llama en estado alerta
pub unsafe extern "system" fn completed_read_routine(
    dw_error: u32,
    dw_bytes_transfered: u32,
    lp_overlapped: *mut OVERLAPPED,
) {
    let ctx_ptr = lp_overlapped as *mut PipeContext;
    // USAMOS REFERENCIA, NO BOX. No queremos liberar la memoria aquí.
    let ctx = &mut *ctx_ptr; 

    // Cliente desconectado o error
    if dw_error != 0 || dw_bytes_transfered == 0 {
        ctx.alive.store(false, Ordering::Release);
        return; 
    }

    let data = &ctx.buffer[..dw_bytes_transfered as usize];
    let resp = crate::worker::handle_request(data);

    if crate::ipc::write_message(ctx.handle, &resp).is_err() {
        ctx.alive.store(false, Ordering::Release);
        return;
    }

    // Relanzar lectura usando el mismo puntero que recibimos
    if !launch_read_ex(ctx_ptr) {
        ctx.alive.store(false, Ordering::Release);
    }
}

pub unsafe fn launch_read_ex(ctx_ptr: *mut PipeContext) -> bool {
    let ctx = &mut *ctx_ptr;
    // Resetear la estructura overlapped para la nueva operación
    ctx.overlapped = std::mem::zeroed();
    
    let res = ReadFileEx(
        ctx.handle,
        Some(&mut ctx.buffer),
        &mut ctx.overlapped,
        Some(completed_read_routine),
    );
    
    res.is_ok()
}


pub fn to_pcwstr(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

pub fn create_pipe_instance(pipe_name_w: &[u16]) -> Result<HANDLE, u32> {
    let open_mode: u32 = 0x00000003u32 | FILE_FLAG_OVERLAPPED.0; // PIPE_ACCESS_DUPLEX
    
    // Configured for Message Type and Message Read Mode.
    // This allows the OS to preserve message boundaries.
    let pipe_mode = NAMED_PIPE_MODE(PIPE_TYPE_MESSAGE.0 | PIPE_READMODE_MESSAGE.0 | PIPE_WAIT.0);

    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(pipe_name_w.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(open_mode),
            pipe_mode,
            PIPE_UNLIMITED_INSTANCES,
            65536, // 64KB Out buffer
            65536, // 64KB In buffer
            0,
            None,
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(unsafe { GetLastError().0 });
    }

    Ok(handle)
}

pub fn connect_instance(handle: HANDLE) -> Result<(), u32> {
    let conn_res = unsafe { ConnectNamedPipe(handle, None) };
    if let Err(_) = conn_res {
        let err = unsafe { GetLastError().0 };
        if err as u32 == ERROR_PIPE_CONNECTED.0 as u32 {
            return Ok(());
        }
        return Err(err);
    }
    Ok(())
}


/// Writes a payload as a single atomic Message.
pub fn write_message(handle: HANDLE, payload: &[u8]) -> Result<(), u32> {
    if payload.is_empty() { return Ok(()); }

    let mut written: u32 = 0;
    // In Message Mode, WriteFile encapsulates the entire buffer as one message
    let res = unsafe { WriteFile(handle, Some(payload), Some(&mut written), None) };
    
    if res.is_err() {
        return Err(unsafe { GetLastError().0 });
    }
    
    if written == 0 {
        return Err(ERROR_BROKEN_PIPE.0 as u32);
    }
    
    Ok(())
}

pub fn close_handle(handle: HANDLE) {
    let _ = unsafe { CloseHandle(handle) };
}