use std::sync::atomic::{AtomicBool, Ordering};
use std::ffi::OsStr;
use std::os::windows::prelude::OsStrExt;

use crate::win_api::{OVERLAPPED, HANDLE, PIPE_ACCESS_DUPLEX, FILE_FLAG_OVERLAPPED, PIPE_TYPE_MESSAGE, PIPE_READMODE_MESSAGE, PIPE_WAIT, PIPE_UNLIMITED_INSTANCES, INVALID_HANDLE_VALUE, ERROR_PIPE_CONNECTED, ERROR_BROKEN_PIPE, GetLastError, ConnectNamedPipe, CreateNamedPipeW, ReadFileEx, WriteFile, CloseHandle};

// Constants.
const PIPE_BUFFER_SIZE: u32 = 65536;

#[repr(C)]
pub struct PipeContext {
    pub overlapped: OVERLAPPED,
    pub handle: HANDLE,
    pub buffer: [u8; 65536],
    pub alive: AtomicBool,
}

/// Completion routine that Windows calls in alert state
pub unsafe extern "system" fn completed_read_routine(
    dw_error: u32,
    dw_bytes_transfered: u32,
    lp_overlapped: *mut OVERLAPPED,
) {
    let ctx_ptr = lp_overlapped as *mut PipeContext;
    let ctx = &mut *ctx_ptr; 

    // Client disconnected or error
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

    // Relaunch reading using the same pointer received
    if !launch_read_ex(ctx_ptr) {
        ctx.alive.store(false, Ordering::Release);
    }
}

pub unsafe fn launch_read_ex(ctx_ptr: *mut PipeContext) -> bool {
    let ctx = &mut *ctx_ptr;
    std::ptr::write_bytes(&mut ctx.overlapped, 0, 1);
    
    let res = ReadFileEx(
        ctx.handle,
        ctx.buffer.as_mut_ptr(),
        ctx.buffer.len() as u32,
        &mut ctx.overlapped,
        Some(completed_read_routine),
    );
    
    res != 0
}


pub fn to_pcwstr(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

pub fn create_pipe_instance(pipe_name_w: &[u16]) -> Result<HANDLE, u32> {
    let open_mode = PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED;
    let pipe_mode = PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT;

    let handle = unsafe {
        CreateNamedPipeW(
            pipe_name_w.as_ptr(),
            open_mode,
            pipe_mode,
            PIPE_UNLIMITED_INSTANCES,
            PIPE_BUFFER_SIZE, // Out buffer
            PIPE_BUFFER_SIZE, // In buffer
            0,     // Default timeout
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(unsafe { GetLastError() });
    }

    Ok(handle)
}

pub fn connect_instance(handle: HANDLE) -> Result<(), u32> {
    let success = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };

    if success == 0 {
        let err = unsafe { GetLastError() };
        if err == ERROR_PIPE_CONNECTED {
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
    let success = unsafe { 
        WriteFile(
            handle, 
            payload.as_ptr(),
            payload.len() as u32,
            &mut written,
            std::ptr::null_mut()
        ) 
    };
    
    if success == 0 {
        return Err(unsafe { GetLastError() });
    }
    
    if written == 0 {
        return Err(ERROR_BROKEN_PIPE);
    }
    
    Ok(())
}

pub fn close_handle(handle: HANDLE) {
    let _ = unsafe { CloseHandle(handle) };
}