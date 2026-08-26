//! Narrow PCM realtime ABI for native Omniphony hosts.
//!
//! Identity remains mode 0 as the deterministic transport oracle. Mode 1 runs
//! the retained stereo Current model on a dedicated worker thread. The host
//! callback only copies PCM into/out of preallocated SPSC rings; the existing
//! allocating renderer never runs on the audio callback thread.

mod height_preference;
mod listening_finish;
mod module_lifetime;
mod native_bed;
mod native_bed_ffi;
mod noire_x_profile;
mod windows_objects_ffi;
mod windows_objects_scene;
mod windows_static_ffi;
pub mod windows_spatial_contract;

use height_preference::HeightPreference;
use listening_finish::ListeningFinish;
use noire_x_profile::NoireXPersonalEq;
use orender_engine::current_music_support::CurrentMusicSupportRenderer;
use renderer::music_field::{MUSIC_FIELD_CHANNELS, MusicFieldProcessor};
use renderer::music_foundation::MusicFoundationProcessor;
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::env;
use std::fs;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(crate) fn ffi_guard<T>(fallback: T, body: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(body)).unwrap_or(fallback)
}

const ABI_MAJOR: u32 = 0;
const ABI_MINOR: u32 = 7;
const MODE_IDENTITY: u32 = 0;
const MODE_CURRENT: u32 = 1;
const PROCESS_BLOCK_MS: usize = 20;
const CURRENT_HOST_LATENCY_MS: usize = 40;
const RING_SECONDS: usize = 2;
const FIELD_SUPPORT_GAIN: f32 = 1.0;
const LINEAR_OUTPUT_GAIN: f32 = 0.90;
const OUTPUT_MAKEUP_GAIN: f32 = 1.380_384_3;
const OUTPUT_CEILING: f32 = 0.891_250_9;
const OUTPUT_LOOKAHEAD_MS: usize = 5;
const OUTPUT_RELEASE_MS: f32 = 160.0;
const CURRENT_ENABLED_FILE_NAME: &str = "current-enabled.txt";

fn enabled_setting_text(text: &str) -> bool {
    !matches!(
        text.trim().to_ascii_lowercase().as_str(),
        "0" | "off" | "false" | "disabled" | "none"
    )
}

/// Resolve the user-facing stereo Current switch once at the mode/control
/// boundary. The realtime callback never touches the filesystem. Missing state
/// preserves historical behavior: Current is enabled by default.
fn stereo_current_enabled() -> bool {
    let root = env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
        .join("Omniphony")
        .join(CURRENT_ENABLED_FILE_NAME);
    fs::read_to_string(root)
        .map(|text| enabled_setting_text(&text))
        .unwrap_or(true)
}

#[repr(C)]
pub struct OmniphonyRealtimeConfig {
    pub sample_rate_hz: u32,
    pub channels: u32,
}

struct AudioRing {
    cells: Box<[UnsafeCell<f32>]>,
    capacity: usize,
    read: AtomicUsize,
    write: AtomicUsize,
}

unsafe impl Send for AudioRing {}
unsafe impl Sync for AudioRing {}

impl AudioRing {
    fn new(capacity: usize) -> Self {
        let capacity = capacity.max(2);
        let cells = (0..capacity)
            .map(|_| UnsafeCell::new(0.0f32))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            cells,
            capacity,
            read: AtomicUsize::new(0),
            write: AtomicUsize::new(0),
        }
    }

    fn available(&self) -> usize {
        self.write
            .load(Ordering::Acquire)
            .wrapping_sub(self.read.load(Ordering::Acquire))
    }

    fn free(&self) -> usize {
        self.capacity.saturating_sub(self.available().min(self.capacity))
    }

    unsafe fn push_ptr(&self, input: *const f32, count: usize) -> bool {
        if count > self.free() {
            return false;
        }
        let write = self.write.load(Ordering::Relaxed);
        for i in 0..count {
            let slot = (write.wrapping_add(i)) % self.capacity;
            unsafe { *self.cells[slot].get() = *input.add(i) };
        }
        self.write.store(write.wrapping_add(count), Ordering::Release);
        true
    }

    fn push_slice(&self, input: &[f32]) -> bool {
        unsafe { self.push_ptr(input.as_ptr(), input.len()) }
    }

    unsafe fn pop_ptr(&self, output: *mut f32, count: usize) -> usize {
        let read = self.read.load(Ordering::Relaxed);
        let available = self
            .write
            .load(Ordering::Acquire)
            .wrapping_sub(read)
            .min(self.capacity);
        let take = count.min(available);
        for i in 0..take {
            let slot = (read.wrapping_add(i)) % self.capacity;
            unsafe { *output.add(i) = *self.cells[slot].get() };
        }
        self.read.store(read.wrapping_add(take), Ordering::Release);
        take
    }

    fn pop_slice(&self, output: &mut [f32]) -> usize {
        unsafe { self.pop_ptr(output.as_mut_ptr(), output.len()) }
    }

    fn discard(&self, count: usize) -> usize {
        let read = self.read.load(Ordering::Relaxed);
        let available = self
            .write
            .load(Ordering::Acquire)
            .wrapping_sub(read)
            .min(self.capacity);
        let take = count.min(available);
        self.read.store(read.wrapping_add(take), Ordering::Release);
        take
    }
}

struct StereoLookaheadPeakGuard {
    frames: VecDeque<[f32; 2]>,
    peaks: VecDeque<(u64, f32)>,
    next_frame_index: u64,
    gain: f32,
    release_coeff: f32,
    lookahead_frames: usize,
}

impl StereoLookaheadPeakGuard {
    fn new(sample_rate_hz: u32) -> Self {
        let release_seconds = OUTPUT_RELEASE_MS / 1000.0;
        let release_coeff = (-1.0 / (release_seconds * sample_rate_hz.max(1) as f32)).exp();
        let lookahead_frames = (sample_rate_hz as usize * OUTPUT_LOOKAHEAD_MS) / 1000;
        Self {
            frames: VecDeque::with_capacity(lookahead_frames + 2),
            peaks: VecDeque::with_capacity(lookahead_frames + 2),
            next_frame_index: 0,
            gain: 1.0,
            release_coeff,
            lookahead_frames,
        }
    }

    fn process_interleaved(&mut self, input: &[f32]) -> Vec<f32> {
        let mut out = Vec::with_capacity(input.len());
        for frame in input.chunks_exact(2) {
            let left = if frame[0].is_finite() { frame[0] } else { 0.0 };
            let right = if frame[1].is_finite() { frame[1] } else { 0.0 };
            let queued = [left * OUTPUT_MAKEUP_GAIN, right * OUTPUT_MAKEUP_GAIN];
            let frame_peak = queued[0].abs().max(queued[1].abs());
            let frame_index = self.next_frame_index;
            self.next_frame_index = self.next_frame_index.saturating_add(1);

            while let Some(&(_, back_peak)) = self.peaks.back() {
                if back_peak >= frame_peak {
                    break;
                }
                self.peaks.pop_back();
            }
            self.peaks.push_back((frame_index, frame_peak));
            self.frames.push_back(queued);

            if self.frames.len() <= self.lookahead_frames {
                continue;
            }

            let oldest_index = frame_index - self.lookahead_frames as u64;
            while self.peaks.front().is_some_and(|&(index, _)| index < oldest_index) {
                self.peaks.pop_front();
            }
            let (peak_frame_index, future_peak) = self.peaks.front().copied().unwrap();
            let peak_index = (peak_frame_index - oldest_index) as usize;
            let target_gain = if future_peak > OUTPUT_CEILING {
                OUTPUT_CEILING / future_peak
            } else {
                1.0
            };

            if target_gain < self.gain {
                if peak_index == 0 {
                    self.gain = target_gain;
                } else {
                    self.gain += (target_gain - self.gain) / peak_index as f32;
                }
            } else {
                self.gain = target_gain - (target_gain - self.gain) * self.release_coeff;
            }

            let current = self.frames.pop_front().unwrap();
            let current_peak = current[0].abs().max(current[1].abs());
            let immediate_safe_gain = if current_peak > OUTPUT_CEILING {
                OUTPUT_CEILING / current_peak
            } else {
                1.0
            };
            let applied_gain = self.gain.min(immediate_safe_gain).clamp(0.0, 1.0);
            self.gain = self.gain.min(applied_gain);
            out.push(current[0] * applied_gain);
            out.push(current[1] * applied_gain);
        }
        out
    }
}

struct CurrentPipeline {
    field: MusicFieldProcessor,
    height: HeightPreference,
    foundation: MusicFoundationProcessor,
    support: CurrentMusicSupportRenderer,
    dry_fifo: VecDeque<f32>,
    foundation_fifo: VecDeque<f32>,
    headphone_eq: NoireXPersonalEq,
    listening_finish: ListeningFinish,
    peak_guard: StereoLookaheadPeakGuard,
}

impl CurrentPipeline {
    fn new(sample_rate_hz: u32) -> Result<Self, String> {
        Ok(Self {
            field: MusicFieldProcessor::new(sample_rate_hz),
            height: HeightPreference::new(),
            foundation: MusicFoundationProcessor::new(sample_rate_hz),
            support: CurrentMusicSupportRenderer::new(sample_rate_hz)
                .map_err(|error| error.to_string())?,
            dry_fifo: VecDeque::new(),
            foundation_fifo: VecDeque::new(),
            headphone_eq: NoireXPersonalEq::new(sample_rate_hz),
            listening_finish: ListeningFinish::new(sample_rate_hz),
            peak_guard: StereoLookaheadPeakGuard::new(sample_rate_hz),
        })
    }

    fn process(&mut self, input: &[f32]) -> Result<Vec<f32>, String> {
        if input.is_empty() || input.len() % 2 != 0 {
            return Err("Current model requires interleaved stereo".to_string());
        }

        self.dry_fifo.extend(input.iter().copied());
        let foundation = self.foundation.process_interleaved_delta(input);
        if foundation.len() != input.len() {
            return Err("foundation width mismatch".to_string());
        }
        self.foundation_fifo.extend(foundation);

        let mut field = self.field.process_interleaved_stereo(input);
        if field.len() != (input.len() / 2) * MUSIC_FIELD_CHANNELS {
            return Err("field width mismatch".to_string());
        }
        self.height.apply(&mut field);

        let rendered = self.support.process(&field).map_err(|error| error.to_string())?;
        let mut out = Vec::new();
        for block in rendered {
            if block.n_channels != 2 {
                return Err("support renderer changed output width".to_string());
            }
            if block.samples.is_empty() {
                continue;
            }
            if self.dry_fifo.len() < block.samples.len()
                || self.foundation_fifo.len() < block.samples.len()
            {
                return Err("support renderer outran aligned master".to_string());
            }

            let mut mixed = Vec::with_capacity(block.samples.len());
            for &support in &block.samples {
                let base = self.dry_fifo.pop_front().unwrap();
                let body = self.foundation_fifo.pop_front().unwrap();
                let base = if base.is_finite() { base } else { 0.0 };
                let body = if body.is_finite() { body } else { 0.0 };
                let support = if support.is_finite() { support } else { 0.0 };
                mixed.push((base + body + support * FIELD_SUPPORT_GAIN) * LINEAR_OUTPUT_GAIN);
            }

            self.headphone_eq.process_interleaved(&mut mixed);
            self.listening_finish.process_interleaved(&mut mixed);
            out.extend(self.peak_guard.process_interleaved(&mixed));
        }
        Ok(out)
    }
}

struct StereoDelay {
    frames: Box<[[f32; 2]]>,
    offset: usize,
    filled: usize,
}

impl StereoDelay {
    fn new(sample_rate_hz: u32) -> Self {
        let delay_frames = ((sample_rate_hz as usize) * CURRENT_HOST_LATENCY_MS / 1000).max(1);
        Self {
            frames: vec![[0.0, 0.0]; delay_frames].into_boxed_slice(),
            offset: 0,
            filled: 0,
        }
    }

    fn delay_frames(&self) -> usize {
        self.frames.len()
    }

    fn reset(&mut self) {
        self.frames.fill([0.0, 0.0]);
        self.offset = 0;
        self.filled = 0;
    }

    fn push(&mut self, input: [f32; 2]) -> ([f32; 2], bool) {
        let delayed = self.frames[self.offset];
        self.frames[self.offset] = input;
        self.offset += 1;
        if self.offset == self.frames.len() {
            self.offset = 0;
        }
        let primed = self.filled >= self.frames.len();
        if self.filled < self.frames.len() {
            self.filled += 1;
        }
        (delayed, primed)
    }
}

struct AsyncCurrent {
    input: Arc<AudioRing>,
    output: Arc<AudioRing>,
    stop: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    processed_blocks: Arc<AtomicU64>,
    rendered_frames: AtomicU64,
    worker: Option<JoinHandle<()>>,
    dry_delay: StereoDelay,
    missed_current_frames: usize,
    worker_ready_observed: bool,
}

impl AsyncCurrent {
    fn new(sample_rate_hz: u32) -> Result<Self, String> {
        if !module_lifetime::pin_for_process_lifetime() {
            return Err(
                "could not pin realtime module for detached Current worker lifetime".to_string(),
            );
        }

        let capacity_samples = (sample_rate_hz as usize)
            .saturating_mul(2)
            .saturating_mul(RING_SECONDS);
        let input = Arc::new(AudioRing::new(capacity_samples));
        let output = Arc::new(AudioRing::new(capacity_samples));
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let processed_blocks = Arc::new(AtomicU64::new(0));

        let input_worker = Arc::clone(&input);
        let output_worker = Arc::clone(&output);
        let stop_worker = Arc::clone(&stop);
        let failed_worker = Arc::clone(&failed);
        let ready_worker = Arc::clone(&ready);
        let blocks_worker = Arc::clone(&processed_blocks);
        let process_frames = ((sample_rate_hz as usize) * PROCESS_BLOCK_MS / 1000).max(64);
        let process_samples = process_frames * 2;

        let worker = thread::Builder::new()
            .name("omniphony-current-model".to_string())
            .spawn(move || {
                let mut pipeline = match CurrentPipeline::new(sample_rate_hz) {
                    Ok(pipeline) => {
                        ready_worker.store(true, Ordering::Release);
                        pipeline
                    }
                    Err(_) => {
                        failed_worker.store(true, Ordering::Release);
                        return;
                    }
                };
                let mut block = vec![0.0f32; process_samples];

                while !stop_worker.load(Ordering::Acquire) {
                    if input_worker.available() < process_samples {
                        thread::sleep(Duration::from_micros(250));
                        continue;
                    }
                    let got = input_worker.pop_slice(&mut block);
                    if got != process_samples {
                        continue;
                    }

                    let rendered = match pipeline.process(&block) {
                        Ok(rendered) => rendered,
                        Err(_) => {
                            failed_worker.store(true, Ordering::Release);
                            return;
                        }
                    };
                    blocks_worker.fetch_add(1, Ordering::Relaxed);

                    if rendered.is_empty() {
                        continue;
                    }
                    while !stop_worker.load(Ordering::Acquire) {
                        if output_worker.free() >= rendered.len() {
                            if output_worker.push_slice(&rendered) {
                                break;
                            }
                        }
                        thread::sleep(Duration::from_micros(250));
                    }
                }
            })
            .map_err(|error| error.to_string())?;

        Ok(Self {
            input,
            output,
            stop,
            failed,
            ready,
            processed_blocks,
            rendered_frames: AtomicU64::new(0),
            worker: Some(worker),
            dry_delay: StereoDelay::new(sample_rate_hz),
            missed_current_frames: 0,
            worker_ready_observed: false,
        })
    }

    fn latency_frames(&self) -> usize {
        self.dry_delay.delay_frames()
    }

    unsafe fn process_raw(
        &mut self,
        input: *const f32,
        output: *mut f32,
        samples: usize,
    ) -> i32 {
        if samples % 2 != 0 {
            return -10;
        }

        let worker_ready = self.ready.load(Ordering::Acquire);
        if worker_ready && !self.worker_ready_observed {
            // Renderer construction is deliberately asynchronous. Audio may
            // have filled the delayed-dry lane before the worker became ready;
            // restart the alignment window at the first callback that can
            // actually submit Current PCM, otherwise every completed worker
            // block is forever classified as stale and discarded.
            self.worker_ready_observed = true;
            self.dry_delay.reset();
            self.missed_current_frames = 0;
            let queued = self.output.available();
            if queued > 0 {
                self.output.discard(queued);
            }
        }

        let mut use_current = worker_ready && !self.failed.load(Ordering::Acquire);
        if use_current {
            if !unsafe { self.input.push_ptr(input, samples) } {
                self.failed.store(true, Ordering::Release);
                use_current = false;
            }
        }

        let frame_count = samples / 2;
        for frame in 0..frame_count {
            let base = frame * 2;
            let left = unsafe { *input.add(base) };
            let right = unsafe { *input.add(base + 1) };
            let (delayed, primed) = self.dry_delay.push([
                if left.is_finite() { left } else { 0.0 },
                if right.is_finite() { right } else { 0.0 },
            ]);

            let mut rendered = delayed;
            if !primed {
                rendered = [0.0, 0.0];
            } else if use_current && !self.failed.load(Ordering::Acquire) {
                while self.missed_current_frames > 0 && self.output.available() >= 2 {
                    let discarded = self.output.discard(2);
                    if discarded != 2 {
                        break;
                    }
                    self.missed_current_frames -= 1;
                }

                if self.missed_current_frames == 0 && self.output.available() >= 2 {
                    let mut current = [0.0f32; 2];
                    let got = self.output.pop_slice(&mut current);
                    if got == 2 {
                        rendered = current;
                        self.rendered_frames.fetch_add(1, Ordering::Relaxed);
                    } else {
                        self.missed_current_frames = self.missed_current_frames.saturating_add(1);
                    }
                } else {
                    self.missed_current_frames = self.missed_current_frames.saturating_add(1);
                }
            }

            rendered[0] = if rendered[0].is_finite() { rendered[0] } else { 0.0 };
            rendered[1] = if rendered[1].is_finite() { rendered[1] } else { 0.0 };
            let peak = rendered[0].abs().max(rendered[1].abs());
            if peak > OUTPUT_CEILING {
                let gain = OUTPUT_CEILING / peak;
                rendered[0] *= gain;
                rendered[1] *= gain;
            }

            unsafe {
                *output.add(base) = rendered[0];
                *output.add(base + 1) = rendered[1];
            }
        }
        0
    }
}

impl Drop for AsyncCurrent {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // A device/graph teardown must never wait for expensive renderer
        // initialization or an in-flight worker block to finish. Dropping the
        // JoinHandle detaches safely; the worker owns only Arc state and exits
        // after observing `stop` when any in-flight work returns.
        let _ = self.worker.take();
    }
}

enum ProcessorMode {
    Identity,
    Current(AsyncCurrent),
}

pub struct OmniphonyRealtimeProcessor {
    sample_rate_hz: u32,
    channels: u32,
    mode: ProcessorMode,
}

#[unsafe(no_mangle)]
pub extern "C" fn omniphony_realtime_abi_major() -> u32 { ABI_MAJOR }

#[unsafe(no_mangle)]
pub extern "C" fn omniphony_realtime_abi_minor() -> u32 { ABI_MINOR }

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_create(
    config: *const OmniphonyRealtimeConfig,
) -> *mut OmniphonyRealtimeProcessor {
    ffi_guard(ptr::null_mut(), || {
        if config.is_null() {
            return ptr::null_mut();
        }
        let config = unsafe { &*config };
        if config.sample_rate_hz == 0 || config.channels == 0 {
            return ptr::null_mut();
        }
        Box::into_raw(Box::new(OmniphonyRealtimeProcessor {
            sample_rate_hz: config.sample_rate_hz,
            channels: config.channels,
            mode: ProcessorMode::Identity,
        }))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_destroy(processor: *mut OmniphonyRealtimeProcessor) {
    ffi_guard((), || {
        if !processor.is_null() {
            unsafe { drop(Box::from_raw(processor)) };
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_set_mode(
    processor: *mut OmniphonyRealtimeProcessor,
    mode: u32,
) -> i32 {
    ffi_guard(-127, || {
        if processor.is_null() {
            return -1;
        }
        let processor = unsafe { &mut *processor };
        match mode {
            MODE_IDENTITY => {
                processor.mode = ProcessorMode::Identity;
                0
            }
            MODE_CURRENT => {
                if processor.channels != 2 {
                    return -2;
                }
                // Both installed stereo APO placements converge here, while
                // authored multichannel uses the separate native-bed ABI. This
                // makes the tray's "Stereo Current" switch exact without ever
                // disabling authored surround/object source truth.
                if !stereo_current_enabled() {
                    processor.mode = ProcessorMode::Identity;
                    return 0;
                }
                match AsyncCurrent::new(processor.sample_rate_hz) {
                    Ok(current) => {
                        processor.mode = ProcessorMode::Current(current);
                        0
                    }
                    Err(_) => -3,
                }
            }
            _ => -4,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_mode(
    processor: *const OmniphonyRealtimeProcessor,
) -> u32 {
    if processor.is_null() {
        return u32::MAX;
    }
    match unsafe { &(*processor).mode } {
        ProcessorMode::Identity => MODE_IDENTITY,
        ProcessorMode::Current(_) => MODE_CURRENT,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_processed_blocks(
    processor: *const OmniphonyRealtimeProcessor,
) -> u64 {
    if processor.is_null() {
        return 0;
    }
    match unsafe { &(*processor).mode } {
        ProcessorMode::Identity => 0,
        ProcessorMode::Current(current) => current.processed_blocks.load(Ordering::Relaxed),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_latency_frames(
    processor: *const OmniphonyRealtimeProcessor,
) -> usize {
    if processor.is_null() {
        return 0;
    }
    match unsafe { &(*processor).mode } {
        ProcessorMode::Identity => 0,
        ProcessorMode::Current(current) => current.latency_frames(),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_reset(
    processor: *mut OmniphonyRealtimeProcessor,
) -> i32 {
    ffi_guard(-127, || {
        if processor.is_null() {
            return -1;
        }
        // Reset is a logical stream boundary. Replacing Current atomically at
        // this control boundary gives the new stream fresh rings, delayed-dry
        // alignment, renderer/EQ/limiter history, counters, and worker startup.
        // Dropping the old instance remains non-blocking: it signals its worker
        // and detaches the JoinHandle by design.
        let processor = unsafe { &mut *processor };
        if matches!(&processor.mode, ProcessorMode::Current(_)) {
            let replacement = match AsyncCurrent::new(processor.sample_rate_hz) {
                Ok(current) => current,
                Err(_) => return -2,
            };
            processor.mode = ProcessorMode::Current(replacement);
        }
        0
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_process_f32(
    processor: *mut OmniphonyRealtimeProcessor,
    input: *const f32,
    output: *mut f32,
    frames: usize,
) -> i32 {
    ffi_guard(-127, || {
        if processor.is_null() {
            return -1;
        }
        if frames == 0 {
            return 0;
        }
        if input.is_null() || output.is_null() {
            return -2;
        }
        let processor = unsafe { &mut *processor };
        let Some(samples) = frames.checked_mul(processor.channels as usize) else {
            return -3;
        };
        match &mut processor.mode {
            ProcessorMode::Identity => {
                unsafe { ptr::copy(input, output, samples) };
                0
            }
            ProcessorMode::Current(current) => {
                if processor.channels != 2 {
                    -4
                } else {
                    unsafe { current.process_raw(input, output, samples) }
                }
            }
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_rendered_frames(
    processor: *const OmniphonyRealtimeProcessor,
) -> u64 {
    if processor.is_null() {
        return 0;
    }
    match unsafe { &(*processor).mode } {
        ProcessorMode::Identity => 0,
        ProcessorMode::Current(current) => current.rendered_frames.load(Ordering::Relaxed),
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_sample_rate_hz(
    processor: *const OmniphonyRealtimeProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).sample_rate_hz } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_realtime_channels(
    processor: *const OmniphonyRealtimeProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).channels } }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> OmniphonyRealtimeConfig {
        OmniphonyRealtimeConfig { sample_rate_hz: 48_000, channels: 2 }
    }

    #[test]
    fn ffi_guard_contains_panics_at_the_host_boundary() {
        assert_eq!(ffi_guard(-127, || panic!("ffi boundary probe")), -127);
    }

    #[test]
    fn current_enabled_setting_parser_is_conservative() {
        for off in ["0", "off", "FALSE", " disabled ", "none"] {
            assert!(!enabled_setting_text(off), "{off:?} should disable stereo Current");
        }
        for on in ["1", "on", "true", "current", "enabled", ""] {
            assert!(enabled_setting_text(on), "{on:?} should preserve stereo Current");
        }
    }

    #[test]
    fn rejects_invalid_configuration() {
        let bad_rate = OmniphonyRealtimeConfig { sample_rate_hz: 0, channels: 2 };
        let bad_channels = OmniphonyRealtimeConfig { sample_rate_hz: 48_000, channels: 0 };
        unsafe {
            assert!(omniphony_realtime_create(std::ptr::null()).is_null());
            assert!(omniphony_realtime_create(&bad_rate).is_null());
            assert!(omniphony_realtime_create(&bad_channels).is_null());
        }
    }

    #[test]
    fn current_reset_replaces_stream_lifetime_state_without_changing_mode() {
        let cfg = config();
        unsafe {
            let processor = omniphony_realtime_create(&cfg);
            assert!(!processor.is_null());
            assert_eq!(omniphony_realtime_set_mode(processor, MODE_CURRENT), 0);
            assert_eq!(omniphony_realtime_mode(processor), MODE_CURRENT);
            assert!(omniphony_realtime_latency_frames(processor) > 0);

            assert_eq!(omniphony_realtime_reset(processor), 0);
            assert_eq!(omniphony_realtime_mode(processor), MODE_CURRENT);
            assert_eq!(omniphony_realtime_processed_blocks(processor), 0);
            assert_eq!(omniphony_realtime_rendered_frames(processor), 0);
            assert!(omniphony_realtime_latency_frames(processor) > 0);
            omniphony_realtime_destroy(processor);
        }
    }

    #[test]
    fn identity_is_bit_exact_out_of_place() {
        let input = [0.0f32, -0.25, 0.5, 1.0, -1.0, 0.125, -0.75, 0.875];
        let mut output = [f32::NAN; 8];
        let cfg = config();
        unsafe {
            let processor = omniphony_realtime_create(&cfg);
            assert!(!processor.is_null());
            assert_eq!(omniphony_realtime_process_f32(processor, input.as_ptr(), output.as_mut_ptr(), 4), 0);
            omniphony_realtime_destroy(processor);
        }
        for (before, after) in input.iter().zip(output.iter()) {
            assert_eq!(before.to_bits(), after.to_bits());
        }
    }

    #[test]
    fn identity_is_bit_exact_in_place() {
        let mut samples = [0.0f32, -0.25, 0.5, 1.0, -1.0, 0.125, -0.75, 0.875];
        let before = samples.map(f32::to_bits);
        let cfg = config();
        unsafe {
            let processor = omniphony_realtime_create(&cfg);
            assert!(!processor.is_null());
            assert_eq!(omniphony_realtime_process_f32(processor, samples.as_ptr(), samples.as_mut_ptr(), 4), 0);
            omniphony_realtime_destroy(processor);
        }
        assert_eq!(before, samples.map(f32::to_bits));
    }

    #[test]
    fn zero_frames_accepts_null_audio_buffers() {
        let cfg = config();
        unsafe {
            let processor = omniphony_realtime_create(&cfg);
            assert!(!processor.is_null());
            assert_eq!(omniphony_realtime_process_f32(processor, std::ptr::null(), std::ptr::null_mut(), 0), 0);
            omniphony_realtime_destroy(processor);
        }
    }

    #[test]
    fn current_reports_fixed_host_latency() {
        let cfg = config();
        unsafe {
            let processor = omniphony_realtime_create(&cfg);
            assert!(!processor.is_null());
            assert_eq!(omniphony_realtime_latency_frames(processor), 0);
            assert_eq!(omniphony_realtime_set_mode(processor, MODE_CURRENT), 0);
            assert_eq!(omniphony_realtime_latency_frames(processor), 1_920);
            omniphony_realtime_destroy(processor);
        }
    }

    #[test]
    fn aligned_dry_delay_primes_at_exact_frame_budget() {
        let mut delay = StereoDelay::new(48_000);
        for frame in 0..1_920 {
            let (_, primed) = delay.push([frame as f32, -(frame as f32)]);
            assert!(!primed);
        }
        let (delayed, primed) = delay.push([9_999.0, -9_999.0]);
        assert!(primed);
        assert_eq!(delayed, [0.0, -0.0]);
    }
}
