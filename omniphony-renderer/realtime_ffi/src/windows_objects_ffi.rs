//! Realtime C ABI for one Windows Spatial Audio stream containing fixed static
//! roles plus a bounded set of dynamic objects.
//!
//! The Windows callback owns no renderer allocation. It copies one fixed-size
//! packet containing dynamic metadata and planar PCM into a preallocated SPSC
//! ring. A dedicated worker reconstructs the authored object quantum and runs a
//! single `WindowsSpatialObjectPipeline` for both static and dynamic sources.

use crate::windows_objects_scene::WindowsSpatialObjectPipeline;
use crate::windows_spatial_contract::{
    WindowsDynamicObject, WindowsSpatialPosition, WindowsStaticObject, WindowsStaticObjectRole,
};
use crate::{AudioRing, OUTPUT_CEILING, RING_SECONDS, StereoDelay};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc::sync_channel};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const DYNAMIC_METADATA_FLOATS: usize = 5;
const MAX_DYNAMIC_OBJECTS_LIMIT: usize = 32;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OmniphonySpatialObjectStaticDescriptor {
    pub role: u32,
    pub x_right_m: f32,
    pub y_up_m: f32,
    pub z_back_m: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OmniphonySpatialDynamicObjectDescriptor {
    pub stable_id: u64,
    pub x_right_m: f32,
    pub y_up_m: f32,
    pub z_back_m: f32,
}

#[repr(C)]
pub struct OmniphonySpatialObjectConfig {
    pub sample_rate_hz: u32,
    pub frames_per_quantum: u32,
    pub static_object_count: u32,
    pub static_objects: *const OmniphonySpatialObjectStaticDescriptor,
    pub max_dynamic_objects: u32,
}

#[derive(Clone, Copy, Debug)]
struct StaticDescriptor {
    role: WindowsStaticObjectRole,
    position: Option<WindowsSpatialPosition>,
}

#[derive(Clone, Copy, Debug)]
struct PacketLayout {
    metadata_floats: usize,
    static_pcm_offset: usize,
    dynamic_pcm_offset: usize,
    packet_floats: usize,
}

impl PacketLayout {
    fn new(
        frames_per_quantum: usize,
        static_count: usize,
        max_dynamic: usize,
    ) -> Option<Self> {
        let metadata_floats = 1usize.checked_add(max_dynamic.checked_mul(DYNAMIC_METADATA_FLOATS)?)?;
        let static_samples = static_count.checked_mul(frames_per_quantum)?;
        let dynamic_samples = max_dynamic.checked_mul(frames_per_quantum)?;
        let static_pcm_offset = metadata_floats;
        let dynamic_pcm_offset = static_pcm_offset.checked_add(static_samples)?;
        let packet_floats = dynamic_pcm_offset.checked_add(dynamic_samples)?;
        Some(Self {
            metadata_floats,
            static_pcm_offset,
            dynamic_pcm_offset,
            packet_floats,
        })
    }
}

fn copy_static_descriptors(
    input: &[OmniphonySpatialObjectStaticDescriptor],
) -> Result<Vec<StaticDescriptor>, String> {
    if input.len() > 17 {
        return Err("static object count exceeds 17".to_string());
    }

    let mut seen = [false; 17];
    let mut out = Vec::with_capacity(input.len());
    for descriptor in input {
        let role = WindowsStaticObjectRole::from_canonical_scene_index(descriptor.role)
            .ok_or_else(|| format!("invalid static object role {}", descriptor.role))?;
        let index = role.canonical_scene_index();
        if seen[index] {
            return Err(format!("duplicate static object role {role:?}"));
        }
        seen[index] = true;

        let position = if role == WindowsStaticObjectRole::LowFrequency {
            None
        } else {
            if !descriptor.x_right_m.is_finite()
                || !descriptor.y_up_m.is_finite()
                || !descriptor.z_back_m.is_finite()
            {
                return Err(format!("non-finite static object position for {role:?}"));
            }
            Some(WindowsSpatialPosition::new(
                descriptor.x_right_m,
                descriptor.y_up_m,
                descriptor.z_back_m,
            ))
        };
        out.push(StaticDescriptor { role, position });
    }
    Ok(out)
}

fn dynamic_position_pan(position: WindowsSpatialPosition) -> [f32; 2] {
    let normalized = position.x_right_m / (1.0 + position.x_right_m.abs());
    let left = ((1.0 - normalized) * 0.5).max(0.0).sqrt();
    let right = ((1.0 + normalized) * 0.5).max(0.0).sqrt();
    [left, right]
}

unsafe fn safety_downmix_frame(
    static_descriptors: &[StaticDescriptor],
    static_planar: *const f32,
    dynamic_descriptors: *const OmniphonySpatialDynamicObjectDescriptor,
    dynamic_planar: *const f32,
    dynamic_count: usize,
    frames: usize,
    frame_index: usize,
) -> [f32; 2] {
    let mut left = 0.0f32;
    let mut right = 0.0f32;

    for (object_index, descriptor) in static_descriptors.iter().enumerate() {
        let sample = unsafe { *static_planar.add(object_index * frames + frame_index) };
        let sample = if sample.is_finite() { sample } else { 0.0 };
        match descriptor.role {
            WindowsStaticObjectRole::FrontLeft => left += sample,
            WindowsStaticObjectRole::FrontRight => right += sample,
            WindowsStaticObjectRole::FrontCenter => {
                left += sample * 0.707_106_77;
                right += sample * 0.707_106_77;
            }
            WindowsStaticObjectRole::LowFrequency => {
                left += sample * 0.5;
                right += sample * 0.5;
            }
            WindowsStaticObjectRole::SideLeft | WindowsStaticObjectRole::BackLeft => {
                left += sample * 0.707_106_77;
            }
            WindowsStaticObjectRole::SideRight | WindowsStaticObjectRole::BackRight => {
                right += sample * 0.707_106_77;
            }
            WindowsStaticObjectRole::BackCenter => {
                left += sample * 0.5;
                right += sample * 0.5;
            }
            WindowsStaticObjectRole::TopFrontLeft
            | WindowsStaticObjectRole::TopBackLeft
            | WindowsStaticObjectRole::BottomFrontLeft
            | WindowsStaticObjectRole::BottomBackLeft => left += sample * 0.5,
            WindowsStaticObjectRole::TopFrontRight
            | WindowsStaticObjectRole::TopBackRight
            | WindowsStaticObjectRole::BottomFrontRight
            | WindowsStaticObjectRole::BottomBackRight => right += sample * 0.5,
        }
    }

    for object_index in 0..dynamic_count {
        let descriptor = unsafe { *dynamic_descriptors.add(object_index) };
        let sample = unsafe { *dynamic_planar.add(object_index * frames + frame_index) };
        let sample = if sample.is_finite() { sample } else { 0.0 };
        let pan = dynamic_position_pan(WindowsSpatialPosition::new(
            descriptor.x_right_m,
            descriptor.y_up_m,
            descriptor.z_back_m,
        ));
        left += sample * pan[0];
        right += sample * pan[1];
    }

    let peak = left.abs().max(right.abs());
    if peak > 1.0 {
        [left / peak, right / peak]
    } else {
        [left, right]
    }
}

fn encode_dynamic_metadata(
    packet: &mut [f32],
    index: usize,
    descriptor: OmniphonySpatialDynamicObjectDescriptor,
) {
    let base = 1 + index * DYNAMIC_METADATA_FLOATS;
    packet[base] = f32::from_bits(descriptor.stable_id as u32);
    packet[base + 1] = f32::from_bits((descriptor.stable_id >> 32) as u32);
    packet[base + 2] = descriptor.x_right_m;
    packet[base + 3] = descriptor.y_up_m;
    packet[base + 4] = descriptor.z_back_m;
}

fn decode_dynamic_metadata(
    packet: &[f32],
    index: usize,
) -> OmniphonySpatialDynamicObjectDescriptor {
    let base = 1 + index * DYNAMIC_METADATA_FLOATS;
    let low = packet[base].to_bits() as u64;
    let high = packet[base + 1].to_bits() as u64;
    OmniphonySpatialDynamicObjectDescriptor {
        stable_id: low | (high << 32),
        x_right_m: packet[base + 2],
        y_up_m: packet[base + 3],
        z_back_m: packet[base + 4],
    }
}

struct AsyncSpatialObjects {
    input: Arc<AudioRing>,
    output: Arc<AudioRing>,
    stop: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
    processed_blocks: Arc<AtomicU64>,
    worker: Option<JoinHandle<()>>,
    fallback_delay: StereoDelay,
    missed_rendered_frames: usize,
    static_descriptors: Vec<StaticDescriptor>,
    frames_per_quantum: usize,
    max_dynamic_objects: usize,
    layout: PacketLayout,
    packet: Vec<f32>,
}

impl AsyncSpatialObjects {
    fn new(
        sample_rate_hz: u32,
        frames_per_quantum: usize,
        static_descriptors: Vec<StaticDescriptor>,
        max_dynamic_objects: usize,
    ) -> Result<Self, String> {
        if sample_rate_hz != 48_000 {
            return Err("Windows spatial object ABI currently requires 48 kHz".to_string());
        }
        if frames_per_quantum == 0 || frames_per_quantum > 4096 {
            return Err("invalid spatial object quantum size".to_string());
        }
        if max_dynamic_objects > MAX_DYNAMIC_OBJECTS_LIMIT {
            return Err(format!(
                "max dynamic object count exceeds {MAX_DYNAMIC_OBJECTS_LIMIT}"
            ));
        }
        if static_descriptors.is_empty() && max_dynamic_objects == 0 {
            return Err("spatial object stream has no static or dynamic capacity".to_string());
        }

        let layout = PacketLayout::new(
            frames_per_quantum,
            static_descriptors.len(),
            max_dynamic_objects,
        )
        .ok_or_else(|| "spatial object packet size overflow".to_string())?;
        let input_capacity = layout
            .packet_floats
            .checked_mul((RING_SECONDS * sample_rate_hz as usize / frames_per_quantum).max(2))
            .ok_or_else(|| "spatial object input ring size overflow".to_string())?;
        let output_capacity = (sample_rate_hz as usize)
            .saturating_mul(2)
            .saturating_mul(RING_SECONDS);

        let input = Arc::new(AudioRing::new(input_capacity.max(layout.packet_floats * 2)));
        let output = Arc::new(AudioRing::new(output_capacity));
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let processed_blocks = Arc::new(AtomicU64::new(0));
        let (init_tx, init_rx) = sync_channel::<Result<(), String>>(1);

        let input_worker = Arc::clone(&input);
        let output_worker = Arc::clone(&output);
        let stop_worker = Arc::clone(&stop);
        let failed_worker = Arc::clone(&failed);
        let blocks_worker = Arc::clone(&processed_blocks);
        let worker_static = static_descriptors.clone();

        let worker = thread::Builder::new()
            .name("omniphony-spatial-objects".to_string())
            .spawn(move || {
                let mut pipeline =
                    match WindowsSpatialObjectPipeline::new(sample_rate_hz, max_dynamic_objects) {
                    Ok(pipeline) => {
                        let _ = init_tx.send(Ok(()));
                        pipeline
                    }
                    Err(error) => {
                        let _ = init_tx.send(Err(error));
                        failed_worker.store(true, Ordering::Release);
                        return;
                    }
                };

                let mut packet = vec![0.0f32; layout.packet_floats];
                while !stop_worker.load(Ordering::Acquire) {
                    if input_worker.available() < layout.packet_floats {
                        thread::sleep(Duration::from_micros(250));
                        continue;
                    }
                    if input_worker.pop_slice(&mut packet) != layout.packet_floats {
                        continue;
                    }

                    let dynamic_count = packet[0].to_bits() as usize;
                    if dynamic_count > max_dynamic_objects {
                        failed_worker.store(true, Ordering::Release);
                        return;
                    }

                    let rendered = {
                        let mut static_objects = Vec::with_capacity(worker_static.len());
                        for (index, descriptor) in worker_static.iter().enumerate() {
                            let start = layout.static_pcm_offset + index * frames_per_quantum;
                            let end = start + frames_per_quantum;
                            static_objects.push(WindowsStaticObject {
                                role: descriptor.role,
                                windows_position: descriptor.position,
                                mono_pcm: &packet[start..end],
                            });
                        }

                        let mut dynamic_objects = Vec::with_capacity(dynamic_count);
                        for index in 0..dynamic_count {
                            let descriptor = decode_dynamic_metadata(&packet, index);
                            let start = layout.dynamic_pcm_offset + index * frames_per_quantum;
                            let end = start + frames_per_quantum;
                            dynamic_objects.push(WindowsDynamicObject {
                                stable_id: descriptor.stable_id,
                                windows_position: WindowsSpatialPosition::new(
                                    descriptor.x_right_m,
                                    descriptor.y_up_m,
                                    descriptor.z_back_m,
                                ),
                                mono_pcm: &packet[start..end],
                            });
                        }

                        let result = if static_objects.is_empty() && dynamic_objects.is_empty() {
                            pipeline.process_silence(frames_per_quantum)
                        } else {
                            pipeline.process(&static_objects, &dynamic_objects)
                        };
                        match result {
                            Ok(rendered) => rendered,
                            Err(_) => {
                                failed_worker.store(true, Ordering::Release);
                                return;
                            }
                        }
                    };
                    blocks_worker.fetch_add(1, Ordering::Relaxed);

                    while !stop_worker.load(Ordering::Acquire) {
                        if output_worker.free() >= rendered.len() && output_worker.push_slice(&rendered) {
                            break;
                        }
                        thread::sleep(Duration::from_micros(250));
                    }
                }
            })
            .map_err(|error| error.to_string())?;

        match init_rx.recv_timeout(Duration::from_secs(30)) {
            Ok(Ok(())) => Ok(Self {
                input,
                output,
                stop,
                failed,
                processed_blocks,
                worker: Some(worker),
                fallback_delay: StereoDelay::new(sample_rate_hz),
                missed_rendered_frames: 0,
                static_descriptors,
                frames_per_quantum,
                max_dynamic_objects,
                layout,
                packet: vec![0.0f32; layout.packet_floats],
            }),
            Ok(Err(error)) => {
                stop.store(true, Ordering::Release);
                let _ = worker.join();
                Err(error)
            }
            Err(error) => {
                stop.store(true, Ordering::Release);
                let _ = worker.join();
                Err(format!("spatial object renderer initialization timed out: {error}"))
            }
        }
    }

    fn latency_frames(&self) -> usize {
        self.fallback_delay.delay_frames()
    }

    unsafe fn process_raw(
        &mut self,
        static_input_planar: *const f32,
        dynamic_descriptors: *const OmniphonySpatialDynamicObjectDescriptor,
        dynamic_count: usize,
        dynamic_input_planar: *const f32,
        output_stereo: *mut f32,
        frames: usize,
    ) -> i32 {
        if frames != self.frames_per_quantum {
            return -10;
        }
        if dynamic_count > self.max_dynamic_objects {
            return -11;
        }
        if !self.static_descriptors.is_empty() && static_input_planar.is_null() {
            return -12;
        }
        if dynamic_count > 0 && (dynamic_descriptors.is_null() || dynamic_input_planar.is_null()) {
            return -13;
        }

        for index in 0..dynamic_count {
            let descriptor = unsafe { *dynamic_descriptors.add(index) };
            if !descriptor.x_right_m.is_finite()
                || !descriptor.y_up_m.is_finite()
                || !descriptor.z_back_m.is_finite()
            {
                return -14;
            }
            if descriptor.stable_id == 0 {
                return -16;
            }
            for previous in 0..index {
                if unsafe { (*dynamic_descriptors.add(previous)).stable_id } == descriptor.stable_id {
                    return -15;
                }
            }
        }

        self.packet.fill(0.0);
        self.packet[0] = f32::from_bits(dynamic_count as u32);
        for index in 0..dynamic_count {
            encode_dynamic_metadata(
                &mut self.packet,
                index,
                unsafe { *dynamic_descriptors.add(index) },
            );
        }

        let static_samples = self.static_descriptors.len() * frames;
        if static_samples > 0 {
            let source = unsafe { std::slice::from_raw_parts(static_input_planar, static_samples) };
            self.packet[self.layout.static_pcm_offset..self.layout.static_pcm_offset + static_samples]
                .copy_from_slice(source);
        }
        let dynamic_samples = dynamic_count * frames;
        if dynamic_samples > 0 {
            let source = unsafe { std::slice::from_raw_parts(dynamic_input_planar, dynamic_samples) };
            self.packet[self.layout.dynamic_pcm_offset..self.layout.dynamic_pcm_offset + dynamic_samples]
                .copy_from_slice(source);
        }

        let mut use_rendered = !self.failed.load(Ordering::Acquire);
        if use_rendered && !self.input.push_slice(&self.packet) {
            self.failed.store(true, Ordering::Release);
            use_rendered = false;
        }

        for frame_index in 0..frames {
            let fallback = unsafe {
                safety_downmix_frame(
                    &self.static_descriptors,
                    static_input_planar,
                    dynamic_descriptors,
                    dynamic_input_planar,
                    dynamic_count,
                    frames,
                    frame_index,
                )
            };
            let (delayed, primed) = self.fallback_delay.push(fallback);
            let mut rendered = if primed { delayed } else { [0.0, 0.0] };

            if primed && use_rendered && !self.failed.load(Ordering::Acquire) {
                while self.missed_rendered_frames > 0 && self.output.available() >= 2 {
                    if self.output.discard(2) != 2 {
                        break;
                    }
                    self.missed_rendered_frames -= 1;
                }

                if self.missed_rendered_frames == 0 && self.output.available() >= 2 {
                    let mut object_render = [0.0f32; 2];
                    if self.output.pop_slice(&mut object_render) == 2 {
                        rendered = object_render;
                    } else {
                        self.missed_rendered_frames = self.missed_rendered_frames.saturating_add(1);
                    }
                } else {
                    self.missed_rendered_frames = self.missed_rendered_frames.saturating_add(1);
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
                *output_stereo.add(frame_index * 2) = rendered[0];
                *output_stereo.add(frame_index * 2 + 1) = rendered[1];
            }
        }
        0
    }
}

impl Drop for AsyncSpatialObjects {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub struct OmniphonySpatialObjectProcessor {
    sample_rate_hz: u32,
    frames_per_quantum: u32,
    static_object_count: u32,
    max_dynamic_objects: u32,
    inner: AsyncSpatialObjects,
}

impl OmniphonySpatialObjectProcessor {
    pub(crate) fn new_static_only(
        sample_rate_hz: u32,
        frames_per_quantum: u32,
        static_objects: &[OmniphonySpatialObjectStaticDescriptor],
    ) -> Result<Self, String> {
        if sample_rate_hz == 0
            || frames_per_quantum == 0
            || static_objects.is_empty()
            || static_objects.len() > 17
        {
            return Err("invalid static spatial object configuration".to_string());
        }
        let static_descriptors = copy_static_descriptors(static_objects)?;
        let inner = AsyncSpatialObjects::new(
            sample_rate_hz,
            frames_per_quantum as usize,
            static_descriptors,
            0,
        )?;
        Ok(Self {
            sample_rate_hz,
            frames_per_quantum,
            static_object_count: static_objects.len() as u32,
            max_dynamic_objects: 0,
            inner,
        })
    }

    pub(crate) fn latency_frames(&self) -> usize {
        self.inner.latency_frames()
    }

    pub(crate) fn processed_blocks(&self) -> u64 {
        self.inner.processed_blocks.load(Ordering::Relaxed)
    }

    pub(crate) fn reset_stream(&mut self) -> Result<(), String> {
        let replacement = AsyncSpatialObjects::new(
            self.sample_rate_hz,
            self.frames_per_quantum as usize,
            self.inner.static_descriptors.clone(),
            self.max_dynamic_objects as usize,
        )?;
        self.inner = replacement;
        Ok(())
    }

    pub(crate) unsafe fn process_static_only(
        &mut self,
        static_input_planar: *const f32,
        output_stereo: *mut f32,
        frames: usize,
    ) -> i32 {
        unsafe {
            self.inner.process_raw(
                static_input_planar,
                ptr::null(),
                0,
                ptr::null(),
                output_stereo,
                frames,
            )
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_create(
    config: *const OmniphonySpatialObjectConfig,
) -> *mut OmniphonySpatialObjectProcessor {
    crate::ffi_guard(ptr::null_mut(), || {
        if config.is_null() {
            return ptr::null_mut();
        }
        let config = unsafe { &*config };
        if config.sample_rate_hz == 0
            || config.frames_per_quantum == 0
            || config.static_object_count > 17
            || config.max_dynamic_objects as usize > MAX_DYNAMIC_OBJECTS_LIMIT
            || (config.static_object_count == 0 && config.max_dynamic_objects == 0)
            || (config.static_object_count > 0 && config.static_objects.is_null())
        {
            return ptr::null_mut();
        }

        let static_descriptors = if config.static_object_count == 0 {
            Vec::new()
        } else {
            let input = unsafe {
                std::slice::from_raw_parts(
                    config.static_objects,
                    config.static_object_count as usize,
                )
            };
            let Ok(descriptors) = copy_static_descriptors(input) else {
                return ptr::null_mut();
            };
            descriptors
        };

        let Ok(inner) = AsyncSpatialObjects::new(
            config.sample_rate_hz,
            config.frames_per_quantum as usize,
            static_descriptors,
            config.max_dynamic_objects as usize,
        ) else {
            return ptr::null_mut();
        };

        Box::into_raw(Box::new(OmniphonySpatialObjectProcessor {
            sample_rate_hz: config.sample_rate_hz,
            frames_per_quantum: config.frames_per_quantum,
            static_object_count: config.static_object_count,
            max_dynamic_objects: config.max_dynamic_objects,
            inner,
        }))
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_destroy(
    processor: *mut OmniphonySpatialObjectProcessor,
) {
    crate::ffi_guard((), || {
        if !processor.is_null() {
            unsafe { drop(Box::from_raw(processor)) };
        }
    });
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_latency_frames(
    processor: *const OmniphonySpatialObjectProcessor,
) -> usize {
    if processor.is_null() {
        0
    } else {
        unsafe { (*processor).latency_frames() }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_processed_blocks(
    processor: *const OmniphonySpatialObjectProcessor,
) -> u64 {
    if processor.is_null() {
        0
    } else {
        unsafe { (*processor).processed_blocks() }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_process_f32(
    processor: *mut OmniphonySpatialObjectProcessor,
    static_input_planar: *const f32,
    dynamic_objects: *const OmniphonySpatialDynamicObjectDescriptor,
    dynamic_object_count: u32,
    dynamic_input_planar: *const f32,
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
        if output_stereo.is_null() {
            return -2;
        }
        let processor = unsafe { &mut *processor };
        unsafe {
            processor.inner.process_raw(
                static_input_planar,
                dynamic_objects,
                dynamic_object_count as usize,
                dynamic_input_planar,
                output_stereo,
                frames,
            )
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_reset(
    processor: *mut OmniphonySpatialObjectProcessor,
) -> i32 {
    crate::ffi_guard(-127, || {
        if processor.is_null() {
            return -1;
        }
        let processor = unsafe { &mut *processor };
        match processor.reset_stream() {
            Ok(()) => 0,
            Err(_) => -2,
        }
    })
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_sample_rate_hz(
    processor: *const OmniphonySpatialObjectProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).sample_rate_hz } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_frames_per_quantum(
    processor: *const OmniphonySpatialObjectProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).frames_per_quantum } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_static_object_count(
    processor: *const OmniphonySpatialObjectProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).static_object_count } }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn omniphony_spatial_objects_max_dynamic_objects(
    processor: *const OmniphonySpatialObjectProcessor,
) -> u32 {
    if processor.is_null() { 0 } else { unsafe { (*processor).max_dynamic_objects } }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_id_packet_encoding_is_exact_for_full_u64_domain() {
        let descriptor = OmniphonySpatialDynamicObjectDescriptor {
            stable_id: u64::MAX,
            x_right_m: -0.25,
            y_up_m: 0.75,
            z_back_m: -1.5,
        };
        let mut packet = vec![0.0f32; 1 + DYNAMIC_METADATA_FLOATS];
        encode_dynamic_metadata(&mut packet, 0, descriptor);
        let decoded = decode_dynamic_metadata(&packet, 0);
        assert_eq!(decoded.stable_id, u64::MAX);
        assert_eq!(decoded.x_right_m, descriptor.x_right_m);
        assert_eq!(decoded.y_up_m, descriptor.y_up_m);
        assert_eq!(decoded.z_back_m, descriptor.z_back_m);
    }

    #[test]
    fn packet_layout_is_fixed_for_stream_capacity_not_active_count() {
        let layout = PacketLayout::new(480, 2, 16).unwrap();
        assert_eq!(layout.metadata_floats, 81);
        assert_eq!(layout.packet_floats, 81 + 2 * 480 + 16 * 480);
    }

    #[test]
    fn static_descriptor_validation_matches_static_abi() {
        let duplicate = OmniphonySpatialObjectStaticDescriptor {
            role: 0,
            x_right_m: -0.7,
            y_up_m: 0.0,
            z_back_m: -0.7,
        };
        assert!(copy_static_descriptors(&[duplicate, duplicate]).is_err());
    }

    #[test]
    fn spatial_object_reset_restarts_worker_state_without_changing_stream_shape() {
        let descriptors = [OmniphonySpatialObjectStaticDescriptor {
            role: 0,
            x_right_m: -0.7,
            y_up_m: 0.0,
            z_back_m: -0.7,
        }];
        let processor = unsafe {
            omniphony_spatial_objects_create(&OmniphonySpatialObjectConfig {
                sample_rate_hz: 48_000,
                frames_per_quantum: 480,
                static_object_count: 1,
                static_objects: descriptors.as_ptr(),
                max_dynamic_objects: 2,
            })
        };
        assert!(!processor.is_null());

        unsafe {
            assert_eq!(omniphony_spatial_objects_reset(processor), 0);
            assert_eq!(omniphony_spatial_objects_sample_rate_hz(processor), 48_000);
            assert_eq!(omniphony_spatial_objects_frames_per_quantum(processor), 480);
            assert_eq!(omniphony_spatial_objects_static_object_count(processor), 1);
            assert_eq!(omniphony_spatial_objects_max_dynamic_objects(processor), 2);
            assert_eq!(omniphony_spatial_objects_processed_blocks(processor), 0);
            omniphony_spatial_objects_destroy(processor);
        }
    }

    #[test]
    fn dynamic_safety_pan_tracks_continuous_x_position() {
        let left = dynamic_position_pan(WindowsSpatialPosition::new(-1.0, 0.0, 0.0));
        let right = dynamic_position_pan(WindowsSpatialPosition::new(1.0, 0.0, 0.0));
        assert!(left[0] > left[1]);
        assert!(right[1] > right[0]);
    }
}
