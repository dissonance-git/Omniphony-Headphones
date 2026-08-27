#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "windows")]
#[path = "../supervisor.rs"]
mod supervisor;

#[cfg(target_os = "windows")]
#[path = "../music_engine.rs"]
mod music_engine;

#[cfg(target_os = "windows")]
pub(crate) mod realtime_priority {
    use std::ffi::c_void;

    #[link(name = "Avrt")]
    unsafe extern "system" {
        fn AvSetMmThreadCharacteristicsW(
            task_name: *const u16,
            task_index: *mut u32,
        ) -> *mut c_void;
        fn AvRevertMmThreadCharacteristics(handle: *mut c_void) -> i32;
    }

    /// Store the opaque AVRT handle as an integer rather than a raw pointer so
    /// the guard can safely move into CPAL's Send playback callback closure.
    pub struct MmcssGuard(usize);

    impl Drop for MmcssGuard {
        fn drop(&mut self) {
            if self.0 != 0 {
                unsafe {
                    let _ = AvRevertMmThreadCharacteristics(self.0 as *mut c_void);
                }
            }
        }
    }

    fn claim(task: &str) -> Option<MmcssGuard> {
        let mut wide: Vec<u16> = task.encode_utf16().collect();
        wide.push(0);
        let mut task_index = 0u32;
        let handle = unsafe { AvSetMmThreadCharacteristicsW(wide.as_ptr(), &mut task_index) };
        (!handle.is_null()).then_some(MmcssGuard(handle as usize))
    }

    pub fn claim_realtime_audio() -> Option<MmcssGuard> {
        claim("Pro Audio").or_else(|| claim("Audio"))
    }
}

#[cfg(target_os = "windows")]
fn run_audio_engine() -> anyhow::Result<()> {
    use anyhow::Context;

    wasapi::initialize_mta()
        .ok()
        .context("failed to initialize COM MTA before Windows audio startup")?;

    let _mmcss = realtime_priority::claim_realtime_audio();
    music_engine::run()
}

fn main() -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        // Spatial is the private Windows-product shell. The renderer internals
        // retain their Omniphony identity while the transport/product layer is
        // replaced. A second copy of this executable is used only as the
        // crash-isolated internal audio-engine child.
        if std::env::var_os("OMNIPHONY_INTERNAL_ENGINE").is_some() {
            return run_audio_engine();
        }
        return supervisor::run();
    }

    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("Spatial is only available on Windows");
    }
}
