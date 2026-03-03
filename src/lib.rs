mod defs;
mod ipc;
mod shared_memory;
mod worker;
mod libLoader;
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::PCWSTR;
use std::os::raw::{c_int, c_char};
use std::thread;
use std::sync::atomic::Ordering;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MsgWaitForMultipleObjectsEx, PeekMessageW,
    TranslateMessage, MSG, PM_REMOVE, QS_ALLINPUT,
    MWMO_ALERTABLE, MWMO_INPUTAVAILABLE, WM_QUIT,
};

use windows::Win32::Foundation::{HANDLE, HWND, CloseHandle, WAIT_IO_COMPLETION, WAIT_FAILED };

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

        // Launch first read overlapped
        if !ipc::launch_read_ex(ctx_ptr) {
            let _ = Box::from_raw(ctx_ptr);
            CloseHandle(handle);
            return;
        }

        let mut msg = MSG::default();

        loop {
            let wait = MsgWaitForMultipleObjectsEx(
                None,
                u32::MAX, // INFINITE
                QS_ALLINPUT,
                MWMO_INPUTAVAILABLE | MWMO_ALERTABLE,
            );
            if wait == WAIT_FAILED {
                eprintln!("MsgWaitForMultipleObjectsEx failed");
                break;
            }
            // APC exec (ReadFileEx completed)
            if wait == WAIT_IO_COMPLETION {
                let ctx = &*ctx_ptr;
                // If completion marked as dead, exit
                if !ctx.alive.load(Ordering::Acquire) {
                    break;
                }
                continue;
            }
            // WINDOWS MESSAGES
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).into() {
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
        let _ = windows::Win32::System::IO::CancelIoEx(handle, None);
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
        let _ = CreateMutexW(None, true, PCWSTR(mutex_name_w.as_ptr()));
        if windows::Win32::Foundation::GetLastError() == windows::Win32::Foundation::ERROR_ALREADY_EXISTS {
            return; // There is already an instance running
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
                    ipc::close_handle(handle);
                    continue;
                }

                // Transfer raw pointer to thread to avoid HANDLE Send issues
                let raw = handle.0 as isize;
                thread::spawn(move || {
                    let h = HANDLE(raw as *mut _);
                    client_thread_loop(h);
                });
            }
            Err(err) => {
                eprintln!("CreateNamedPipeW failed: {}", err);
                std::thread::sleep(std::time::Duration::from_secs(1));
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


use windows::Win32::System::Threading::GetProcessVersion;

fn start_parent_monitor(pid: u32) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            
            // GetProcessVersion returns 0 if the PID is invalid or has ended
            let version = unsafe { GetProcessVersion(pid) };
            
            if version == 0 {
                // The parent process disappeared
                std::process::exit(0);
            }
        }
    });
}