use std::ptr::{null_mut};
use crate::win_api::{PAGE_READWRITE, FILE_MAP_WRITE, HANDLE, INVALID_HANDLE_VALUE, CreateFileMappingW, GetLastError, MapViewOfFile, UnmapViewOfFile, CloseHandle, CreateEventW};




#[repr(C, packed)]
pub struct SharedHeader {
    pub h_engine: u32,
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

        let to_w = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();

        // CreateFileMappingW(hFile, lpAttributes, flProtect, sizeHigh, sizeLow, lpName)
        let h_map = CreateFileMappingW(
            INVALID_HANDLE_VALUE,
            null_mut(),
            PAGE_READWRITE,
            0,
            size as u32,
            to_w(&shm_name).as_ptr()
        );

        if h_map.is_null() || h_map == INVALID_HANDLE_VALUE {
            return Err(format!("CreateFileMappingW failed: {}", GetLastError()));
        }

        // MapViewOfFile(hMapping, dwAccess, offsetHigh, offsetLow, bytesToMap)
        let view = MapViewOfFile(h_map, FILE_MAP_WRITE, 0, 0, size);
        if view.is_null() {
            CloseHandle(h_map);
            return Err(format!("MapViewOfFile failed: {}", GetLastError()));
        }

        // CreateEventW(lpAttributes, bManualReset, bInitialState, lpName)
        let h_evt_ready = CreateEventW(null_mut(), 0, 0, to_w(&evt_ready_name).as_ptr());
        let h_evt_processed = CreateEventW(null_mut(), 0, 0, to_w(&evt_proc_name).as_ptr());

        Ok(Self { 
            h_map, 
            view: view as *mut u8, 
            h_evt_ready, 
            h_evt_processed, 
            size 
        })
    }

    pub unsafe fn get_eci_buffer_ptr(&self) -> *mut i16 {
        self.view.add(12) as *mut i16 // 12 bytes header (SharedHeader)
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        unsafe {
            if !self.view.is_null() {
                UnmapViewOfFile(self.view as *const std::os::raw::c_void);
            }
            if !self.h_evt_ready.is_null() { CloseHandle(self.h_evt_ready); }
            if !self.h_evt_processed.is_null() { CloseHandle(self.h_evt_processed); }
            if !self.h_map.is_null() { CloseHandle(self.h_map); }
        }
    }
}