//! 系统休眠阻止器。
//!
//! 在长时间运行 AI 编译、推理或后台长任务时，阻止 Windows 系统进入休眠/待机状态，
//! 确保后台任务不被打断；全部空闲后自动恢复系统常规省电策略。

#[cfg(windows)]
pub struct SleepInhibitor {
    active: bool,
}

#[cfg(windows)]
impl SleepInhibitor {
    pub fn new() -> Self {
        Self { active: false }
    }

    /// 阻止系统休眠（允许屏幕正常关闭省电）
    pub fn prevent_sleep(&mut self) {
        if !self.active {
            unsafe {
                unsafe extern "system" {
                    fn SetThreadExecutionState(esFlags: u32) -> u32;
                }
                const ES_CONTINUOUS: u32 = 0x80000000;
                const ES_SYSTEM_REQUIRED: u32 = 0x00000001;
                SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
            }
            self.active = true;
        }
    }

    /// 恢复系统默认休眠策略
    pub fn allow_sleep(&mut self) {
        if self.active {
            unsafe {
                unsafe extern "system" {
                    fn SetThreadExecutionState(esFlags: u32) -> u32;
                }
                const ES_CONTINUOUS: u32 = 0x80000000;
                SetThreadExecutionState(ES_CONTINUOUS);
            }
            self.active = false;
        }
    }
}

#[cfg(windows)]
impl Drop for SleepInhibitor {
    fn drop(&mut self) {
        self.allow_sleep();
    }
}

#[cfg(not(windows))]
pub struct SleepInhibitor;

#[cfg(not(windows))]
impl SleepInhibitor {
    pub fn new() -> Self {
        Self
    }
    pub fn prevent_sleep(&mut self) {}
    pub fn allow_sleep(&mut self) {}
}

impl Default for SleepInhibitor {
    fn default() -> Self {
        Self::new()
    }
}
