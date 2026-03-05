use std::os::raw::{c_void, c_char,};

// Basic Windows types
pub type HANDLE = *mut c_void;
pub type HWND = *mut c_void;
pub type HMODULE = *mut c_void;
pub type BOOL = i32;
pub type FARPROC = *mut c_void;

// Constants.
pub const INVALID_HANDLE_VALUE: HANDLE = -1isize as *mut c_void;
pub const PIPE_ACCESS_DUPLEX: u32 = 0x00000003;
pub const FILE_FLAG_OVERLAPPED: u32 = 0x40000000;
pub const PIPE_TYPE_MESSAGE: u32 = 0x00000004;
pub const PIPE_READMODE_MESSAGE: u32 = 0x00000002;
pub const PIPE_WAIT: u32 = 0x00000000;
pub const PIPE_UNLIMITED_INSTANCES: u32 = 255;
pub const ERROR_PIPE_CONNECTED: u32 = 535;
pub const ERROR_BROKEN_PIPE: u32 = 109;
pub const QS_ALLINPUT: u32 = 0x04FF;
pub const MWMO_INPUTAVAILABLE: u32 = 0x0004;
pub const MWMO_ALERTABLE: u32 = 0x0002;
pub const WAIT_IO_COMPLETION: u32 = 0x000000C0;
pub const WAIT_FAILED: u32 = 0xFFFFFFFF;
pub const PM_REMOVE: u32 = 0x0001;
pub const WM_QUIT: u32 = 0x0012;
pub const ERROR_ALREADY_EXISTS: u32 = 183;
pub const PAGE_READWRITE: u32 = 0x04;
pub const FILE_MAP_WRITE: u32 = 0x0002;
pub const WAIT_OBJECT_0: u32 = 0;
#[repr(C)]
#[derive(Default)]
pub struct MSG {
    pub hwnd: HWND,
    pub message: u32,
    pub wparam: usize,
    pub lparam: isize,
    pub time: u32,
    pub pt: POINT,
}

#[repr(C)]
#[derive(Default)]
pub struct POINT { pub x: i32, pub y: i32 }

#[repr(C)]
pub struct OVERLAPPED {
    pub internal: usize,
    pub internal_high: usize,
    pub offset: u32,
    pub offset_high: u32,
    pub h_event: HANDLE,
}

// Function Signatures
#[link(name = "kernel32")]
extern "system" {
    pub fn GetLastError() -> u32;
    pub fn CloseHandle(h: HANDLE) -> BOOL;
    pub fn CreateMutexW(lpMutexAttributes: *mut c_void, bInitialOwner: BOOL, lpName: *const u16) -> HANDLE;
    pub fn CreateNamedPipeW(lpName: *const u16, dwOpenMode: u32, dwPipeMode: u32, nMaxInstances: u32, nOutBufSize: u32, nInBufSize: u32, nDefaultTimeOut: u32, lpSecurityAttrs: *mut c_void) -> HANDLE;
    pub fn ConnectNamedPipe(hNamedPipe: HANDLE, lpOverlapped: *mut c_void) -> BOOL;
    pub fn WriteFile(hFile: HANDLE, lpBuffer: *const u8, nNumberOfBytesToWrite: u32, lpNumberOfBytesWritten: *mut u32, lpOverlapped: *mut c_void) -> BOOL;
    pub fn ReadFileEx(hFile: HANDLE, lpBuffer: *mut u8, nNumberOfBytesToRead: u32, lpOverlapped: *mut OVERLAPPED, lpCompletionRoutine: Option<unsafe extern "system" fn(u32, u32, *mut OVERLAPPED)>) -> BOOL;
    pub fn CancelIoEx(hFile: HANDLE, lpOverlapped: *mut c_void) -> BOOL;
    pub fn GetProcessVersion(processId: u32) -> u32;
    pub fn CreateFileMappingW(hFile: HANDLE, lpFileMappingAttributes: *mut c_void, flProtect: u32, dwMaximumSizeHigh: u32, dwMaximumSizeLow: u32, lpName: *const u16) -> HANDLE;
    pub fn MapViewOfFile(hFileMappingObject: HANDLE, dwDesiredAccess: u32, dwFileOffsetHigh: u32, dwFileOffsetLow: u32, dwNumberOfBytesToMap: usize) -> *mut c_void;
    pub fn UnmapViewOfFile(lpBaseAddress: *const c_void) -> BOOL;
    pub fn CreateEventW(lpEventAttributes: *mut c_void, bManualReset: BOOL, bInitialState: BOOL, lpName: *const u16) -> HANDLE;
    pub fn SetEvent(hEvent: HANDLE) -> BOOL;
    pub fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: u32) -> u32;
    pub fn LoadLibraryW(lpLibFileName: *const u16) -> HMODULE;
    pub fn GetProcAddress(hModule: HMODULE, lpProcName: *const c_char) -> FARPROC;
    pub fn FreeLibrary(hModule: HMODULE) -> i32;
}

#[link(name = "user32")]
extern "system" {
    pub fn MsgWaitForMultipleObjectsEx(nCount: u32, pHandles: *const HANDLE, dwMilliseconds: u32, dwWakeMask: u32, dwFlags: u32) -> u32;
    pub fn PeekMessageW(lpMsg: *mut MSG, hWnd: HWND, wMsgFilterMin: u32, wMsgFilterMax: u32, wRemoveMsg: u32) -> BOOL;
    pub fn TranslateMessage(lpMsg: *const MSG) -> BOOL;
    pub fn DispatchMessageW(lpMsg: *const MSG) -> isize;
}