mod defs;
mod ipc;
mod shared_memory;
mod worker;
mod libLoader;
mod win_api;

use std::os::raw::{c_int, c_char, c_void};
use std::thread;
use std::sync::atomic::Ordering;

pub const WAIT_FAILED: u32 = 0xFFFFFFFF;
use crate::win_api::{HANDLE, CloseHandle, MSG, GetLastError, PeekMessageW, TranslateMessage, DispatchMessageW, CancelIoEx, MsgWaitForMultipleObjectsEx, CreateMutexW, HWND, GetProcessVersion, QS_ALLINPUT, MWMO_INPUTAVAILABLE, MWMO_ALERTABLE, WAIT_IO_COMPLETION , PM_REMOVE, WM_QUIT, ERROR_ALREADY_EXISTS};
use ipc::to_pcwstr;



fn client_thread_loop(handle: HANDLE) {
    unsafe {
        let ctx = Box::new(ipc::PipeContext {
            overlapped: std::mem::zeroed(),
            handle,
            buffer: [0u8; 65536],
            alive: std::sync::atomic::AtomicBool::new(true),
        });

        let ctx_ptr = Box::into_raw(ctx);

        if !ipc::launch_read_ex(ctx_ptr) {
            let _ = Box::from_raw(ctx_ptr);
            CloseHandle(handle);
            return;
        }

        let mut msg = MSG::default();

        loop {
            // pass null_mut() for the handles array since we only wait for messages/APCs
            let wait = MsgWaitForMultipleObjectsEx(
                0,
                std::ptr::null(),
                u32::MAX, // INFINITE
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE | MWMO_ALERTABLE,
            );

            if wait == WAIT_FAILED {
                eprintln!("MsgWaitForMultipleObjectsEx failed: {}", GetLastError());
                break;
            }

            // APC executed (ReadFileEx completed)
            if wait == WAIT_IO_COMPLETION {
                let ctx = &*ctx_ptr;
                if !ctx.alive.load(Ordering::Acquire) {
                    break;
                }
                continue;
            }

            // Process Windows messages (required for COM or SAPI if used)
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    eprintln!("WM_QUIT received");
                    break;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            let ctx = &*ctx_ptr;
            if !ctx.alive.load(Ordering::Acquire) {
                break;
            }
        }
        
        eprintln!("Client thread exiting cleanly, cleaning up IO...");
        CancelIoEx(handle, std::ptr::null_mut());
        CloseHandle(handle);
        if !ctx_ptr.is_null() {
            let _ = Box::from_raw(ctx_ptr);
        }
        eprintln!("Client thread finished.");
    }
}

pub fn run_host() {
    let mutex_name_w = to_pcwstr("Global\\IBMTTS_Host_Unique_Mutex");
    unsafe {
        let _ = CreateMutexW(std::ptr::null_mut(), 1, mutex_name_w.as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            eprintln!("There is already an instance running.");
            return;
        }
    }

    let pipe_name = r"\\.\pipe\ibmtts_host32";
    let pipe_name_w = to_pcwstr(pipe_name);

    println!("Starting 32-bit named-pipe host on {}", pipe_name);

    loop {
        match ipc::create_pipe_instance(&pipe_name_w) {
            Ok(handle) => {
                println!("Waiting for client...");
                if let Err(err) = ipc::connect_instance(handle) {
                    eprintln!("ConnectNamedPipe failed: {}", err);
                    unsafe { CloseHandle(handle); }
                    continue;
                }

                let handle_raw = handle as isize;
                thread::spawn(move || {
                    client_thread_loop(handle_raw as *mut c_void);
                });
            }
            Err(err) => {
                eprintln!("CreateNamedPipeW failed: {}", err);
                thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn StartHost(
    _hwnd: HWND,
    _hinst: isize,
    lpsz_cmd_line: *const c_char,
    _n_cmd_show: c_int,
) {
    let _ = std::panic::catch_unwind(|| {
        if !lpsz_cmd_line.is_null() {
            unsafe {
                let c_str = std::ffi::CStr::from_ptr(lpsz_cmd_line).to_string_lossy();
                let clean_pid = c_str.chars().filter(|c| c.is_ascii_digit()).collect::<String>();
                if let Ok(parent_pid) = clean_pid.parse::<u32>() {
                    start_parent_monitor(parent_pid);
                }
            }
        }
        run_host();
    });
}

fn start_parent_monitor(pid: u32) {
    thread::spawn(move || {
        loop {
            thread::sleep(std::time::Duration::from_millis(500));
            // GetProcessVersion returns 0 if the PID is invalid or terminated
            let version = unsafe { GetProcessVersion(pid) };
            if version == 0 {
                std::process::exit(0);
            }
        }
    });
}