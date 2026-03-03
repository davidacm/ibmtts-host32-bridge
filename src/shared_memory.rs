use windows::core::PCWSTR;
use windows::Win32::Foundation::{HANDLE, CloseHandle, INVALID_HANDLE_VALUE };
use windows::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, FILE_MAP_WRITE, PAGE_READWRITE
};
use windows::Win32::System::Threading::{CreateEventW};

#[repr(C, packed)]
pub struct SharedHeader {
    pub h_engine: u32, // Host writes EciHandle -> Client writes ECICallbackReturn (int) here upon completion
    pub msg: u32,
    pub lparam: i32,
}

pub struct SharedMemory {
    pub h_map: HANDLE,
    pub view: *mut u8,
    pub h_evt_ready: HANDLE,
    pub h_evt_processed: HANDLE,
    pub size: usize
}

impl SharedMemory {
    pub unsafe fn create(eci_id: u32, size: usize) -> Result<Self, String> {
        let shm_name = format!("Local\\eci_shm_{:x}\0", eci_id);
        let evt_ready_name = format!("Local\\eci_ready_{:x}\0", eci_id);
        let evt_proc_name = format!("Local\\eci_proc_{:x}\0", eci_id);

        let to_w = |s: &str| s.encode_utf16().collect::<Vec<u16>>();

        let h_map = CreateFileMappingW(
            INVALID_HANDLE_VALUE, None, PAGE_READWRITE, 0, size as u32, 
            PCWSTR(to_w(&shm_name).as_ptr())
        ).map_err(|e| e.to_string())?;

        let view = MapViewOfFile(h_map, FILE_MAP_WRITE, 0, 0, size);
        if view.Value.is_null() {
            let _ = CloseHandle(h_map);
            return Err("MapViewOfFile Null".into());
        }

        let h_evt_ready = CreateEventW(None, false, false, PCWSTR(to_w(&evt_ready_name).as_ptr())).unwrap();
        let h_evt_processed = CreateEventW(None, false, false, PCWSTR(to_w(&evt_proc_name).as_ptr())).unwrap();

        Ok(Self { h_map, view: view.Value as *mut u8, h_evt_ready, h_evt_processed, size })
    }

    pub unsafe fn get_eci_buffer_ptr(&self) -> *mut i16 {
        self.view.add(12) as *mut i16 // 12 bytes header
    }
}

use windows::Win32::System::Memory::{UnmapViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS};

impl Drop for SharedMemory {
    fn drop(&mut self) {
        eprintln!("Cleaning up shared memory resources... ");
        unsafe {
            // Unmap the view by wrapping the pointer in the required structure
            if !self.view.is_null() {
                let addr = MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.view as *mut _,
                };
                let _ = UnmapViewOfFile(addr);
            }
            
            // Close event handlers
            let _ = CloseHandle(self.h_evt_ready);
            let _ = CloseHandle(self.h_evt_processed);
            
            // Close file mapping handle
            let _ = CloseHandle(self.h_map);
        }
    }
}