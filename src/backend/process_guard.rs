//! Windows Job Object 进程树清理守卫。
//!
//! 将 PTY 派生的子进程及其后续通过 Shell 或构建工具启动的子进程绑定到 Job Object，
//! 并配置 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`。当 Tab 关闭或程序退出时，
//! Windows 内核原子地杀光该 Job 内的全部子进程树，防止孤儿进程脱离驻留并狂吃 CPU。

#[cfg(windows)]
pub struct ProcessJobGuard {
    job_handle: *mut std::ffi::c_void,
}

#[cfg(windows)]
unsafe impl Send for ProcessJobGuard {}
#[cfg(windows)]
unsafe impl Sync for ProcessJobGuard {}

#[cfg(windows)]
impl ProcessJobGuard {
    pub fn new() -> Option<Self> {
        use std::ptr::null_mut;

        #[repr(C)]
        struct IO_COUNTERS {
            read_operation_count: u64,
            write_operation_count: u64,
            other_operation_count: u64,
            read_transfer_count: u64,
            write_transfer_count: u64,
            other_transfer_count: u64,
        }

        #[repr(C)]
        struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
            per_process_user_time_limit: i64,
            per_job_user_time_limit: i64,
            limit_flags: u32,
            minimum_working_set_size: usize,
            maximum_working_set_size: usize,
            active_process_limit: u32,
            affinity: usize,
            priority_class: u32,
            scheduling_class: u32,
        }

        #[repr(C)]
        struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
            basic_limit_information: JOBOBJECT_BASIC_LIMIT_INFORMATION,
            io_info: IO_COUNTERS,
            process_memory_limit: usize,
            job_memory_limit: usize,
            peak_process_memory_used: usize,
            peak_job_memory_used: usize,
        }

        const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: u32 = 0x2000;
        const JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS: i32 = 9;

        unsafe extern "system" {
            fn CreateJobObjectW(
                lpJobAttributes: *mut std::ffi::c_void,
                lpName: *const u16,
            ) -> *mut std::ffi::c_void;
            fn SetInformationJobObject(
                hJob: *mut std::ffi::c_void,
                JobObjectInformationClass: i32,
                lpJobObjectInformation: *mut std::ffi::c_void,
                cbJobObjectInformationLength: u32,
            ) -> i32;
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
        }

        unsafe {
            let handle = CreateJobObjectW(null_mut(), null_mut());
            if handle.is_null() {
                return None;
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.basic_limit_information.limit_flags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let res = SetInformationJobObject(
                handle,
                JOB_OBJECT_EXTENDED_LIMIT_INFORMATION_CLASS,
                &mut info as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if res == 0 {
                CloseHandle(handle);
                return None;
            }

            Some(Self { job_handle: handle })
        }
    }

    /// 将目标进程 PID 绑定到当前 Job Object
    pub fn assign_process_by_id(&self, pid: u32) -> bool {
        const PROCESS_SET_QUOTA: u32 = 0x0100;
        const PROCESS_TERMINATE: u32 = 0x0001;

        unsafe extern "system" {
            fn OpenProcess(
                dwDesiredAccess: u32,
                bInheritHandle: i32,
                dwProcessId: u32,
            ) -> *mut std::ffi::c_void;
            fn AssignProcessToJobObject(
                hJob: *mut std::ffi::c_void,
                hProcess: *mut std::ffi::c_void,
            ) -> i32;
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
        }

        unsafe {
            let proc_handle = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
            if proc_handle.is_null() {
                return false;
            }
            let success = AssignProcessToJobObject(self.job_handle, proc_handle) != 0;
            CloseHandle(proc_handle);
            success
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessJobGuard {
    fn drop(&mut self) {
        unsafe extern "system" {
            fn CloseHandle(hObject: *mut std::ffi::c_void) -> i32;
        }
        if !self.job_handle.is_null() {
            unsafe {
                CloseHandle(self.job_handle);
            }
        }
    }
}

#[cfg(not(windows))]
pub struct ProcessJobGuard;

#[cfg(not(windows))]
impl ProcessJobGuard {
    pub fn new() -> Option<Self> {
        Some(Self)
    }
    pub fn assign_process_by_id(&self, _pid: u32) -> bool {
        true
    }
}
