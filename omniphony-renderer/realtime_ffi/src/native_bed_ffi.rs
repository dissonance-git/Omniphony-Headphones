//! Realtime C ABI for authored multichannel speaker beds.
//!
//! This is intentionally separate from `omniphony_realtime_*`, whose stereo
//! Current contract is already proven in the Windows endpoint path. The stream
//! APO selects this ABI only when Windows supplies an actual multichannel
//! WAVEFORMATEXTENSIBLE bed. Rendering stays off the AudioDG callback thread.

use crate::native_bed::{NativeBedLayout, NativeBedPipeline};
use crate::{AudioRing, OUTPUT_CEILING, PROCESS_BLOCK_MS, RING_SECONDS, StereoDelay};
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[repr(C)]
pub struct OmniphonyNativeBedConfig {
    pub sample_rate_hz: u32,
    pub channels: u32,
    pub channel_mask: u32,
}

struct AsyncNativeBed {
    input: Arc<AudioRing>,
    output: Arc<AudioRing>,
    stop: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    ready: Arc<AtomicBool>,
    processed_blocks: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
    fallback_layout: NativeBedLayout,
    fallback_delay: StereoDelay,
    missed_native_frames: usize,
    channels: usize,
}

impl AsyncNativeBed {
    fn new(sample_rate_hz: u32, channels: usize, channel_mask: u32) -> Result<Self, String> {
        if !crate::module_lifetime::pin_for_process_lifetime() {
            return Err(
                "could not pin realtime module for detached native-bed worker lifetime"
                    .to_string(),
            );
        }

        let fallback_layout = NativeBedLayout::new(channels, channel_mask)?;
        let input_capacity = (sample_rate_hz as usize)
            .saturating_mul(channels)
            .saturating_mul(RING_SECONDS);
        let output_capacity = (sample_rate_hz as usize)
            .saturating_mul(2)
            .saturating_mul(RING_SECONDS);
        let input = Arc::new(AudioRing::new(input_capacity));
        let output = Arc::new(AudioRing::new(output_capacity));
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
        let process_samples = process_frames.saturating_mul(channels);

        let worker = thread::Builder::new()
            .name("omniphony-native-bed".to_string())
            .spawn(move || {
                let mut pipeline = match NativeBedPipeline::new(sample_rate_hz, channels, channel_mask) {
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
                        if output_worker.free() >= rendered.len() && output_worker.push_slice(&rendered) {
                            break;
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
            worker: Some(worker),
            fallback_layout,
            fallback_delay: StereoDelay::new(sample_rate_hz),
            missed_native_frames: 0,
            channels,
        })
    }

    fn latency_frames(&self) -> usize {
        self.fallback_delay.delay_frames()
    }

    unsafe fn process_raw(&mut self, input: *const f32, output: *mut f32, frames: usize) -> i32 {
        let Some(input_samples) = frames.checked_mul(self.channels) else {
            return -10;
        };

        let mut use_native = self.ready.load(Ordering::Acquire)
            && !self.failed.load(Ordering::Acquire);
        if use_native && !unsafe { self.input.push_ptr(input, input_samples) } {
            self.failed.store(true, Ordering::Release);
            use_native = false;
        }

        for frame_index in 0..frames {
            let input_base = frame_index * self.channels;
            let input_frame = unsafe {
                std::slice::from_raw_parts(input.add(input_base), self.channels)
            };
            let fallback = self.fallback_layout.safety_downmix_frame(input_frame);
            let (delayed, primed) = self.fallback_delay.push(fallback);

            let mut rendered = if primed { delayed } else { [0.0, 0.0] };
            if primed && use_native && !self.failed.load(Ordering::Acquire) {
                while self.missed_native_frames > 0 && self.output.available() >= 2 {
                    if self.output.discard(2) != 2 {
                        break;
                    }
                    self.missed_native_frames -= 1;
                }

                if self.missed_native_frames == 0 && self.output.available() >= 2 {
                    let mut native = [0.0f32; 2];
                    if self.output.pop_slice(&mut native) == 2 {
                        rendered = native;
                    } else {
                        self.missed_native_frames = self.missed_native_frames.saturating_add(1);
                    }
                } else {
                    self.missed_native_frames = self.missed_native_frames.saturating_add(1);
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
                *output.add(frame_index * 2) = rendered[0];
                *output.add(frame_index * 2 + 1) = rendered[1];
            }
        }
        0
    }
}

impl Drop for AsyncNativeBed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Device invalidation must never make AudioDG graph teardown wait for a
        // renderer worker that may still be initializing or finishing a block.
        // Dropping JoinHandle detaches safely; the worker owns only Arc state
        // and observes `stop` once any in-flight initialization/work returns.
        let _ = self.worker.take();
    }
}

pub struct OmniphonyNativeBedProcessor {
    sample_rate_hz: u32,
    channels: u32,
    channel_mask: u32,
    inner: AsyncNativeBed,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_native_bed_create(
    config: *const OmniphonyNativeBedConfig,
) -> *mut OmniphonyNativeBedProcessor {
    crate::ffi_guard(ptr::null_mut(), || {
        if config.is_null() {
            return ptr::null_mut();
        }
        let config = unsafe { &*config };
        if config.sample_rate_hz == 0 || config.channels == 0 {
            return ptr::null_mut();
        }
        let Ok(inner) = AsyncNativeBed::new(
            config.sample_rate_hz,
            config.channels as usize,
            config.channel_mask,
        ) else {
            return ptr::null_mut();
        };
        Box::into_raw(Box::new(OmniphonyNativeBedProcessor {
            sample_rate_hz: config.sample_rate_hz,
            channels: config.channels,
            channel_mask: config.channel_mask,
            inner,
        }))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_native_bed_destroy(
    processor: *mut OmniphonyNativeBedProcessor,
) {
    crate::ffi_guard((), || {
        if !processor.is_null() {
            unsafe { drop(Box::from_raw(processor)) };
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_native_bed_latency_frames(
    processor: *const OmniphonyNativeBedProcessor,
) -> usize {
    if processor.is_null() {
        0
    } else {
        unsafe { (*processor).inner.latency_frames() }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_native_bed_processed_blocks(
    processor: *const OmniphonyNativeBedProcessor,
) -> u64 {
    if processor.is_null() {
        0
    } else {
        unsafe { (*processor).inner.processed_blocks.load(Ordering::Relaxed) }
    }
}

/// Process authored interleaved multichannel float32 PCM to binaural stereo.
/// Input and output must not alias because their channel counts differ.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_native_bed_process_f32(
    processor: *mut OmniphonyNativeBedProcessor,
    input: *const f32,
    output_stereo: *mut f32,
    frames: usize,
) -> i32 {
    crate::ffi_guard(-127, || {
        if processor.is_null() {
            return -1;
        }
        if frames == 0 {
            return 0;
        }
        if input.is_null() || output_stereo.is_null() {
            return -2;
        }
        let processor = unsafe { &mut *processor };
        unsafe { processor.inner.process_raw(input, output_stereo, frames) }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_native_bed_sample_rate_hz(
    processor: *const OmniphonyNativeBedProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).sample_rate_hz } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_native_bed_channels(
    processor: *const OmniphonyNativeBedProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).channels } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_native_bed_channel_mask(
    processor: *const OmniphonyNativeBedProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).channel_mask } }
}
