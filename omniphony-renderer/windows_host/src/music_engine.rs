use anyhow::{Context, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use renderer::music_field::{MUSIC_FIELD_CHANNELS, MusicFieldProcessor, MusicFieldSnapshot};
use renderer::music_foundation::MusicFoundationProcessor;
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc::{Receiver, TryRecvError, sync_channel},
};
use std::time::{Duration, Instant};
use wasapi::{
    AudioCaptureClient, AudioClient, Direction, SampleType, StreamMode, WaveFormat,
    make_channelmasks,
};

use orender_engine::current_music_support::CurrentMusicSupportRenderer;

const SAMPLE_RATE_HZ: u32 = 48_000;
/// Emergency burst reservoir between the capture/DSP producer and WASAPI.
/// This is capacity, not desired latency. Playback inventory is separately
/// regulated below so a cold-start producer burst cannot park hundreds of
/// milliseconds of history in this queue.
const PLAYBACK_QUEUE_BLOCKS: usize = 32;
/// Two nominal 20 ms process-loopback packets. This is the steady transport
/// inventory target, not the renderer's total end-to-end latency.
const PLAYBACK_TARGET_LATENCY_MS: usize = 40;
/// Re-enter refill when callback-visible inventory collapses this far below the
/// target. The existing continuity ramp carries the audible edge while the
/// queue rebuilds.
const PLAYBACK_LOW_RECOVER_LATENCY_MS: usize = 10;
/// If scheduler/boot catch-up places substantially more history in front of the
/// DAC, discard oldest buffered source time back to the target rather than
/// preserving a permanent A/V-like offset until manual engine restart.
const PLAYBACK_HIGH_RECOVER_LATENCY_MS: usize = 100;
/// If the playback producer is briefly late, glide the last sample toward zero
/// instead of creating an instantaneous waveform-to-zero discontinuity. Two
/// milliseconds is short enough not to sound like a fade, while removing the
/// sharp edge that turns a rare queue starvation into a click/crackle.
const PLAYBACK_CONCEAL_FRAMES: usize = 96;
const FIELD_SUPPORT_GAIN: f32 = 1.00;
/// Fixed linear output headroom shared by ON and OFF.
const LINEAR_OUTPUT_GAIN: f32 = 0.90;
/// Fixed listening-level reclaim downstream of every spatial mechanism.
/// Physical listening found the tonal/power balance right but the total level a
/// little high, so reduce only this final fixed gain instead of touching bass,
/// foundation, support balance, or the peak-safety law.
const OUTPUT_MAKEUP_DB: f32 = 2.8;
const OUTPUT_MAKEUP_GAIN: f32 = 1.380_384_3;
/// Conservative sample ceiling leaves margin for inter-sample reconstruction.
const OUTPUT_CEILING_DBFS: f32 = -1.0;
const OUTPUT_CEILING: f32 = 0.891_250_9;
const OUTPUT_LOOKAHEAD_FRAMES: usize = 240; // 5 ms at 48 kHz.
const OUTPUT_RELEASE_MS: f32 = 160.0;
const METER_INTERVAL_SECS: u64 = 5;

fn playback_frames_for_ms(ms: usize) -> usize {
    (SAMPLE_RATE_HZ as usize).saturating_mul(ms) / 1000
}

#[derive(Default)]
struct Args {
    output: Option<String>,
    start_off: bool,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut parsed = Args::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => {
                parsed.output = Some(
                    args.next()
                        .context("--output requires a device-name substring")?,
                );
            }
            "--start-off" => parsed.start_off = true,
            "-h" | "--help" => {
                println!(
                    "Omniphony protected-master full-sphere stereo renderer\n\n\
                     One normal listening path: Current model.\n"
                );
                std::process::exit(0);
            }
            other => bail!("unknown argument: {other}"),
        }
    }
    Ok(parsed)
}

/// Final-bus safety only. This is not a loudness leveller or spatial AGC.
///
/// The guard adds fixed makeup gain, delays both channels equally, and applies
/// one stereo-linked attenuation envelope only when a future peak would cross
/// the endpoint ceiling. Relative L/R amplitude and upstream spatial relations
/// are preserved.
struct StereoLookaheadPeakGuard {
    frames: VecDeque<[f32; 2]>,
    peaks: VecDeque<(u64, f32)>,
    next_frame_index: u64,
    gain: f32,
    release_coeff: f32,
    min_gain_since_report: f32,
}

impl StereoLookaheadPeakGuard {
    fn new(sample_rate_hz: u32) -> Self {
        let release_seconds = OUTPUT_RELEASE_MS / 1000.0;
        let release_coeff = (-1.0 / (release_seconds * sample_rate_hz.max(1) as f32)).exp();
        Self {
            frames: VecDeque::with_capacity(OUTPUT_LOOKAHEAD_FRAMES + 2),
            peaks: VecDeque::with_capacity(OUTPUT_LOOKAHEAD_FRAMES + 2),
            next_frame_index: 0,
            gain: 1.0,
            release_coeff,
            min_gain_since_report: 1.0,
        }
    }

    fn process_interleaved(&mut self, input: &[f32]) -> anyhow::Result<Vec<f32>> {
        if input.len() % 2 != 0 {
            bail!("output peak guard requires interleaved stereo samples");
        }
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

            if self.frames.len() <= OUTPUT_LOOKAHEAD_FRAMES {
                continue;
            }

            let oldest_index = frame_index - OUTPUT_LOOKAHEAD_FRAMES as u64;
            while self
                .peaks
                .front()
                .is_some_and(|&(index, _)| index < oldest_index)
            {
                self.peaks.pop_front();
            }
            let (peak_frame_index, future_peak) = self
                .peaks
                .front()
                .copied()
                .expect("look-ahead peak queue is non-empty");
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

            let current = self
                .frames
                .pop_front()
                .expect("lookahead queue is non-empty");
            let current_peak = current[0].abs().max(current[1].abs());
            let immediate_safe_gain = if current_peak > OUTPUT_CEILING {
                OUTPUT_CEILING / current_peak
            } else {
                1.0
            };
            let applied_gain = self.gain.min(immediate_safe_gain).clamp(0.0, 1.0);
            self.gain = self.gain.min(applied_gain);
            self.min_gain_since_report = self.min_gain_since_report.min(applied_gain);
            out.push(current[0] * applied_gain);
            out.push(current[1] * applied_gain);
        }
        Ok(out)
    }

    fn take_max_reduction_db(&mut self) -> f32 {
        let reduction = if self.min_gain_since_report < 1.0 {
            -20.0 * self.min_gain_since_report.max(1.0e-6).log10()
        } else {
            0.0
        };
        self.min_gain_since_report = 1.0;
        reduction
    }
}

fn report_output_peak_guard(guard: &mut StereoLookaheadPeakGuard) {
    let reduction_db = guard.take_max_reduction_db();
    println!(
        "  output: +{OUTPUT_MAKEUP_DB:.1} dB makeup, ceiling={OUTPUT_CEILING_DBFS:.1} dBFS, max peak reduction={reduction_db:.2} dB"
    );
}

#[derive(Default)]
struct SignalMeter {
    sum_squares: f64,
    peak: f32,
    samples: u64,
}

impl SignalMeter {
    fn observe(&mut self, samples: &[f32]) {
        for &sample in samples {
            if !sample.is_finite() {
                continue;
            }
            self.sum_squares += f64::from(sample) * f64::from(sample);
            self.peak = self.peak.max(sample.abs());
            self.samples = self.samples.saturating_add(1);
        }
    }

    fn observe_delta(&mut self, mixed: &[f32], dry: &[f32]) {
        for (&wet, &base) in mixed.iter().zip(dry.iter()) {
            let delta = wet - base;
            if !delta.is_finite() {
                continue;
            }
            self.sum_squares += f64::from(delta) * f64::from(delta);
            self.peak = self.peak.max(delta.abs());
            self.samples = self.samples.saturating_add(1);
        }
    }

    fn rms_dbfs(&self) -> f32 {
        if self.samples == 0 {
            return -120.0;
        }
        to_dbfs((self.sum_squares / self.samples as f64).sqrt() as f32)
    }

    fn peak_dbfs(&self) -> f32 {
        to_dbfs(self.peak)
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

fn to_dbfs(value: f32) -> f32 {
    if !value.is_finite() || value <= 1.0e-6 {
        -120.0
    } else {
        20.0 * value.log10()
    }
}

fn print_meters(
    direct: &mut SignalMeter,
    evidence: &mut SignalMeter,
    rendered: &mut SignalMeter,
    added: &mut SignalMeter,
    scene: MusicFieldSnapshot,
    bypass: bool,
) {
    if bypass {
        println!(
            "  meter OFF: direct rms={:.1} dBFS peak={:.1} dBFS",
            direct.rms_dbfs(),
            direct.peak_dbfs()
        );
    } else {
        let direct_rms = direct.rms_dbfs();
        let added_rms = added.rms_dbfs();
        println!(
            "  meter ON: direct={direct_rms:.1}/{:.1} dBFS | field={:.1}/{:.1} | rendered={:.1}/{:.1} | added={added_rms:.1}/{:.1} | added/direct={:+.1} dB",
            direct.peak_dbfs(),
            evidence.rms_dbfs(),
            evidence.peak_dbfs(),
            rendered.rms_dbfs(),
            rendered.peak_dbfs(),
            added.peak_dbfs(),
            added_rms - direct_rms,
        );
        println!(
            "  evidence: anchor={:.2} broad={:.2} lateral={:.2} diffuse={:.2} height={:.2} pan={:+.2} side={:.2}",
            scene.anchor,
            scene.broad,
            scene.lateral,
            scene.diffuse,
            scene.height,
            scene.lateral_pan,
            scene.side_fraction,
        );
    }
    direct.clear();
    evidence.clear();
    rendered.clear();
    added.clear();
}

#[derive(Clone, Default)]
struct PlaybackTelemetry {
    underrun_frames: Arc<AtomicU64>,
    buffered_frames: Arc<AtomicU64>,
    trimmed_frames: Arc<AtomicU64>,
}

fn report_playback_transport(telemetry: &PlaybackTelemetry) {
    let underrun_frames = telemetry.underrun_frames.swap(0, Ordering::Relaxed);
    let buffered_frames = telemetry.buffered_frames.load(Ordering::Relaxed);
    let trimmed_frames = telemetry.trimmed_frames.swap(0, Ordering::Relaxed);
    let buffered_ms = buffered_frames as f64 * 1000.0 / SAMPLE_RATE_HZ as f64;
    let trimmed_ms = trimmed_frames as f64 * 1000.0 / SAMPLE_RATE_HZ as f64;
    println!(
        "  transport: queue={buffered_ms:.1} ms target={PLAYBACK_TARGET_LATENCY_MS} ms, trimmed={trimmed_ms:.1} ms"
    );
    if underrun_frames != 0 {
        let duration_ms = underrun_frames as f64 * 1000.0 / SAMPLE_RATE_HZ as f64;
        eprintln!(
            "  realtime warning: WASAPI playback queue starved for {underrun_frames} frame(s) (~{duration_ms:.2} ms) in the last meter interval; short gaps were continuity-concealed"
        );
    }
}

pub fn run() -> anyhow::Result<()> {
    let args = parse_args()?;
    let host = cpal::default_host();
    let output_device = choose_output_device(&host, args.output.as_deref())?;
    let output_name = output_device
        .name()
        .unwrap_or_else(|_| "<unavailable output name>".to_string());
    let (output_format, output_config) = choose_output_config(&output_device, SAMPLE_RATE_HZ)?;
    let mut loopback = LoopbackCapture::open_stereo(SAMPLE_RATE_HZ)?;
    let mut support_renderer = CurrentMusicSupportRenderer::new(SAMPLE_RATE_HZ)?;

    println!("Omniphony for Headphones - protected-master full-sphere renderer");
    println!("  profile: current");
    println!("  capture: {SAMPLE_RATE_HZ} Hz / stereo / f32 process loopback");
    println!("  output:  {output_name}");
    println!("  direct:  captured stereo master remains authoritative");
    println!("  analysis: FFT magnitude + phase -> portable stereo/scene inference");
    println!("  field:   below 320 Hz protected; 320+ Hz uses 12 evidence lanes");
    println!("  height:  vertical extent from already-spatial evidence + coherent transfer");
    println!("  foundation: coherent pressure/body delta, no LFE/compression/saturation");
    println!("  support route: single native Omniphony spatial path");
    println!(
        "  support: {:.0}% derived-field mix, linear master+foundation+support summing",
        FIELD_SUPPORT_GAIN * 100.0
    );
    println!(
        "  output: {:.1} dB base trim + {OUTPUT_MAKEUP_DB:.1} dB makeup; {OUTPUT_CEILING_DBFS:.1} dBFS stereo-linked look-ahead safety ceiling",
        20.0 * LINEAR_OUTPUT_GAIN.log10()
    );
    println!(
        "  realtime: producer + playback callback claim MMCSS; transport targets {PLAYBACK_TARGET_LATENCY_MS} ms and hard-recovers above {PLAYBACK_HIGH_RECOVER_LATENCY_MS} ms; {}-frame continuity concealment is active",
        PLAYBACK_CONCEAL_FRAMES
    );

    let quit = Arc::new(AtomicBool::new(false));
    let playback_telemetry = PlaybackTelemetry::default();
    let playback_failed = Arc::new(AtomicBool::new(false));
    let (play_tx, play_rx) = sync_channel::<Vec<f32>>(PLAYBACK_QUEUE_BLOCKS);
    let playback_stream = build_playback_stream(
        &output_device,
        &output_config,
        output_format,
        play_rx,
        playback_telemetry.clone(),
        Arc::clone(&playback_failed),
    )?;
    spawn_quit_control(Arc::clone(&quit));
    playback_stream
        .play()
        .context("failed to start WASAPI playback stream")?;
    loopback.start()?;

    let mut field = MusicFieldProcessor::new(SAMPLE_RATE_HZ);
    let mut foundation = MusicFoundationProcessor::new(SAMPLE_RATE_HZ);
    let mut output_peak_guard = StereoLookaheadPeakGuard::new(SAMPLE_RATE_HZ);
    let mut dry_fifo = VecDeque::<f32>::new();
    let mut foundation_fifo = VecDeque::<f32>::new();
    let mut direct_meter = SignalMeter::default();
    let mut evidence_meter = SignalMeter::default();
    let mut rendered_meter = SignalMeter::default();
    let mut added_meter = SignalMeter::default();
    let mut last_meter_report = Instant::now();

    println!();
    println!(
        "LIVE. Omniphony is {}.",
        if args.start_off { "OFF" } else { "ON" }
    );

    while !quit.load(Ordering::Relaxed) {
        if playback_failed.load(Ordering::Acquire) {
            bail!("WASAPI playback stream failed; supervisor will restart the audio engine");
        }
        let Some(input) = loopback.next_block()? else {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        };
        if input.is_empty() || input.len() % 2 != 0 {
            continue;
        }

        let output_reference = apply_output_headroom(&input);
        direct_meter.observe(&output_reference);

        if args.start_off {
            let output_reference = output_peak_guard.process_interleaved(&output_reference)?;
            queue_block(&play_tx, output_reference)?;
            if last_meter_report.elapsed() >= Duration::from_secs(METER_INTERVAL_SECS) {
                print_meters(
                    &mut direct_meter,
                    &mut evidence_meter,
                    &mut rendered_meter,
                    &mut added_meter,
                    field.snapshot(),
                    true,
                );
                report_playback_transport(&playback_telemetry);
                report_output_peak_guard(&mut output_peak_guard);
                last_meter_report = Instant::now();
            }
            continue;
        }

        dry_fifo.extend(input.iter().copied());
        let foundation_delta = foundation.process_interleaved_delta(&input);
        if foundation_delta.len() != input.len() {
            bail!(
                "music foundation width mismatch: stereo samples={} foundation samples={}",
                input.len(),
                foundation_delta.len()
            );
        }
        foundation_fifo.extend(foundation_delta);

        let field_input = field.process_interleaved_stereo(&input);
        if field_input.len() != (input.len() / 2) * MUSIC_FIELD_CHANNELS {
            bail!(
                "music field width mismatch: stereo samples={} field samples={}",
                input.len(),
                field_input.len()
            );
        }
        evidence_meter.observe(&field_input);

        let rendered = support_renderer
            .process(&field_input)
            .context("live Omniphony support render failed")?;
        for block in rendered {
            if block.n_channels != 2 {
                bail!(
                    "music support renderer changed output width to {}",
                    block.n_channels
                );
            }
            if block.samples.is_empty() {
                continue;
            }
            if dry_fifo.len() < block.samples.len() || foundation_fifo.len() < block.samples.len() {
                bail!(
                    "music support produced {} samples with dry/foundation buffered at {}/{}",
                    block.samples.len(),
                    dry_fifo.len(),
                    foundation_fifo.len()
                );
            }
            rendered_meter.observe(&block.samples);
            let mut dry = Vec::with_capacity(block.samples.len());
            let mut foundation_delta = Vec::with_capacity(block.samples.len());
            for _ in 0..block.samples.len() {
                dry.push(dry_fifo.pop_front().expect("dry FIFO length checked above"));
                foundation_delta.push(
                    foundation_fifo
                        .pop_front()
                        .expect("foundation FIFO length checked above"),
                );
            }
            let mixed = mix_preserved_master_with_support(
                &dry,
                &foundation_delta,
                &block.samples,
                FIELD_SUPPORT_GAIN,
            )?;
            let dry_reference = apply_output_headroom(&dry);
            added_meter.observe_delta(&mixed, &dry_reference);
            let mixed = output_peak_guard.process_interleaved(&mixed)?;
            queue_block(&play_tx, mixed)?;
        }

        if last_meter_report.elapsed() >= Duration::from_secs(METER_INTERVAL_SECS) {
            print_meters(
                &mut direct_meter,
                &mut evidence_meter,
                &mut rendered_meter,
                &mut added_meter,
                field.snapshot(),
                false,
            );
            report_playback_transport(&playback_telemetry);
            report_output_peak_guard(&mut output_peak_guard);
            last_meter_report = Instant::now();
        }
    }

    let _ = loopback.stop();
    drop(playback_stream);
    println!("Omniphony frequency-evidence renderer stopped");
    Ok(())
}

fn apply_output_headroom(samples: &[f32]) -> Vec<f32> {
    samples
        .iter()
        .map(|&sample| {
            if sample.is_finite() {
                sample * LINEAR_OUTPUT_GAIN
            } else {
                0.0
            }
        })
        .collect()
}

fn mix_preserved_master_with_support(
    dry: &[f32],
    foundation: &[f32],
    support: &[f32],
    support_gain: f32,
) -> anyhow::Result<Vec<f32>> {
    if dry.len() != support.len() || dry.len() != foundation.len() {
        bail!(
            "support-field length mismatch: dry={} foundation={} field={} samples",
            dry.len(),
            foundation.len(),
            support.len()
        );
    }
    let gain = support_gain.clamp(0.0, 1.0);
    let mut out = Vec::with_capacity(dry.len());
    for ((&base, &body), &field) in dry.iter().zip(foundation.iter()).zip(support.iter()) {
        let base = if base.is_finite() { base } else { 0.0 };
        let body = if body.is_finite() { body } else { 0.0 };
        let field = if field.is_finite() { field } else { 0.0 };
        out.push((base + body + field * gain) * LINEAR_OUTPUT_GAIN);
    }
    Ok(out)
}

fn queue_block(tx: &std::sync::mpsc::SyncSender<Vec<f32>>, block: Vec<f32>) -> anyhow::Result<()> {
    if block.is_empty() {
        return Ok(());
    }
    tx.send(block)
        .map_err(|_| anyhow::anyhow!("WASAPI playback stream disconnected"))
}

struct LoopbackCapture {
    client: AudioClient,
    capture: AudioCaptureClient,
    scratch: Vec<u8>,
}

impl LoopbackCapture {
    fn open_stereo(sample_rate_hz: u32) -> anyhow::Result<Self> {
        const BUFFER_DURATION_HNS: i64 = 200_000;
        let mode = StreamMode::PollingShared {
            autoconvert: true,
            buffer_duration_hns: BUFFER_DURATION_HNS,
        };
        let mask = make_channelmasks(2).into_iter().next().unwrap_or(0);
        let format = WaveFormat::new(
            32,
            32,
            &SampleType::Float,
            sample_rate_hz as usize,
            2,
            Some(mask),
        );
        let mut client = AudioClient::new_application_loopback_client(std::process::id(), false)
            .context("failed to activate self-excluding Windows process loopback")?;
        client
            .initialize_client(&format, &Direction::Capture, &mode)
            .context("Windows process loopback rejected required stereo 48 kHz float format")?;
        let capture = client
            .get_audiocaptureclient()
            .context("process loopback initialized but exposed no capture client")?;
        Ok(Self {
            client,
            capture,
            scratch: Vec::new(),
        })
    }

    fn start(&self) -> anyhow::Result<()> {
        self.client
            .start_stream()
            .context("failed to start self-excluding Windows process loopback")
    }

    fn stop(&self) -> anyhow::Result<()> {
        self.client
            .stop_stream()
            .context("failed to stop Windows process loopback")
    }

    fn next_block(&mut self) -> anyhow::Result<Option<Vec<f32>>> {
        let frames = self
            .capture
            .get_next_packet_size()
            .context("failed to query Windows process-loopback packet size")?
            .unwrap_or(0) as usize;
        if frames == 0 {
            return Ok(None);
        }
        let needed = frames.saturating_mul(2).saturating_mul(4);
        self.scratch.resize(needed, 0);
        let (read_frames, info) = self
            .capture
            .read_from_device(&mut self.scratch)
            .context("failed to read Windows process-loopback packet")?;
        let read_frames = read_frames as usize;
        if read_frames == 0 {
            return Ok(None);
        }
        let sample_count = read_frames.saturating_mul(2);
        if info.flags.silent {
            return Ok(Some(vec![0.0; sample_count]));
        }
        let byte_count = sample_count.saturating_mul(4);
        let mut samples = Vec::with_capacity(sample_count);
        for bytes in self.scratch[..byte_count].chunks_exact(4) {
            samples.push(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
        }
        Ok(Some(samples))
    }
}

fn name_contains(device: &cpal::Device, needle: &str) -> bool {
    device
        .name()
        .map(|name| {
            name.to_ascii_lowercase()
                .contains(&needle.to_ascii_lowercase())
        })
        .unwrap_or(false)
}

fn looks_like_virtual_cable(device: &cpal::Device) -> bool {
    device
        .name()
        .map(|name| {
            let lower = name.to_ascii_lowercase();
            lower.contains("vb-audio")
                || lower.contains("hi-fi cable")
                || lower.contains("hifi cable")
        })
        .unwrap_or(false)
}

fn choose_output_device(
    host: &cpal::Host,
    requested: Option<&str>,
) -> anyhow::Result<cpal::Device> {
    if let Some(needle) = requested {
        return host
            .output_devices()?
            .find(|device| name_contains(device, needle))
            .with_context(|| format!("no WASAPI output device contains '{needle}'"));
    }
    if let Some(device) = host
        .output_devices()?
        .find(|device| name_contains(device, "fiio"))
    {
        return Ok(device);
    }
    if let Some(device) = host.default_output_device() {
        if !looks_like_virtual_cable(&device) {
            return Ok(device);
        }
    }
    bail!("no physical output was auto-detected; expected FiiO or non-cable Windows default")
}

fn sample_format_rank(format: cpal::SampleFormat) -> u8 {
    match format {
        cpal::SampleFormat::F32 => 0,
        cpal::SampleFormat::I32 => 1,
        cpal::SampleFormat::I16 => 2,
        cpal::SampleFormat::F64 => 3,
        cpal::SampleFormat::U32 => 4,
        cpal::SampleFormat::U16 => 5,
        cpal::SampleFormat::I8 | cpal::SampleFormat::U8 => 6,
        cpal::SampleFormat::I64 | cpal::SampleFormat::U64 => 7,
        _ => 8,
    }
}

fn choose_output_config(
    device: &cpal::Device,
    sample_rate_hz: u32,
) -> anyhow::Result<(cpal::SampleFormat, cpal::StreamConfig)> {
    let best = device
        .supported_output_configs()
        .context("failed to enumerate WASAPI output formats")?
        .filter(|range| {
            range.channels() >= 2
                && range.min_sample_rate().0 <= sample_rate_hz
                && range.max_sample_rate().0 >= sample_rate_hz
        })
        .min_by_key(|range| (range.channels(), sample_format_rank(range.sample_format())))
        .with_context(|| format!("output device has no >=2ch {sample_rate_hz} Hz format"))?;
    let sample_format = best.sample_format();
    let config = best
        .with_sample_rate(cpal::SampleRate(sample_rate_hz))
        .config();
    Ok((sample_format, config))
}

fn build_playback_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: cpal::SampleFormat,
    rx: Receiver<Vec<f32>>,
    telemetry: PlaybackTelemetry,
    stream_failed: Arc<AtomicBool>,
) -> anyhow::Result<cpal::Stream> {
    match format {
        cpal::SampleFormat::I8 => build_typed_playback::<i8>(device, config, rx, telemetry, stream_failed),
        cpal::SampleFormat::I16 => build_typed_playback::<i16>(device, config, rx, telemetry, stream_failed),
        cpal::SampleFormat::I32 => build_typed_playback::<i32>(device, config, rx, telemetry, stream_failed),
        cpal::SampleFormat::I64 => build_typed_playback::<i64>(device, config, rx, telemetry, stream_failed),
        cpal::SampleFormat::U8 => build_typed_playback::<u8>(device, config, rx, telemetry, stream_failed),
        cpal::SampleFormat::U16 => build_typed_playback::<u16>(device, config, rx, telemetry, stream_failed),
        cpal::SampleFormat::U32 => build_typed_playback::<u32>(device, config, rx, telemetry, stream_failed),
        cpal::SampleFormat::U64 => build_typed_playback::<u64>(device, config, rx, telemetry, stream_failed),
        cpal::SampleFormat::F32 => build_typed_playback::<f32>(device, config, rx, telemetry, stream_failed),
        cpal::SampleFormat::F64 => build_typed_playback::<f64>(device, config, rx, telemetry, stream_failed),
        other => bail!("unsupported WASAPI output sample format: {other:?}"),
    }
}

#[derive(Debug, Clone, Copy)]
struct PlaybackPreparation {
    audible: bool,
    became_audible: bool,
    trimmed_frames: usize,
    buffered_frames: usize,
}

#[derive(Debug, Default)]
struct PlaybackLatencyGovernor {
    audible: bool,
}

impl PlaybackLatencyGovernor {
    fn prepare(
        &mut self,
        rx: &Receiver<Vec<f32>>,
        current: &mut Vec<f32>,
        cursor: &mut usize,
        pending: &mut VecDeque<Vec<f32>>,
    ) -> PlaybackPreparation {
        while let Ok(block) = rx.try_recv() {
            if !block.is_empty() {
                pending.push_back(block);
            }
        }

        let target_frames = playback_frames_for_ms(PLAYBACK_TARGET_LATENCY_MS);
        let low_frames = playback_frames_for_ms(PLAYBACK_LOW_RECOVER_LATENCY_MS);
        let high_frames = playback_frames_for_ms(PLAYBACK_HIGH_RECOVER_LATENCY_MS);
        let mut buffered = buffered_playback_frames(current, *cursor, pending);
        let was_audible = self.audible;

        if self.audible && buffered < low_frames {
            self.audible = false;
        }

        let mut trimmed_frames = 0usize;
        if !self.audible {
            if buffered >= target_frames {
                if buffered > target_frames {
                    trimmed_frames = discard_oldest_playback_frames(
                        current,
                        cursor,
                        pending,
                        buffered - target_frames,
                    );
                    buffered = buffered.saturating_sub(trimmed_frames);
                }
                self.audible = true;
            }
        } else if buffered > high_frames {
            trimmed_frames = discard_oldest_playback_frames(
                current,
                cursor,
                pending,
                buffered - target_frames,
            );
            buffered = buffered.saturating_sub(trimmed_frames);
        }

        PlaybackPreparation {
            audible: self.audible,
            became_audible: !was_audible && self.audible,
            trimmed_frames,
            buffered_frames: buffered,
        }
    }
}

fn buffered_playback_frames(
    current: &[f32],
    cursor: usize,
    pending: &VecDeque<Vec<f32>>,
) -> usize {
    let current_frames = current.len().saturating_sub(cursor) / 2;
    current_frames.saturating_add(
        pending
            .iter()
            .map(|block| block.len() / 2)
            .sum::<usize>(),
    )
}

fn discard_oldest_playback_frames(
    current: &mut Vec<f32>,
    cursor: &mut usize,
    pending: &mut VecDeque<Vec<f32>>,
    mut frames: usize,
) -> usize {
    let requested = frames;
    while frames != 0 {
        let available = current.len().saturating_sub(*cursor) / 2;
        if available != 0 {
            let discard = available.min(frames);
            *cursor += discard * 2;
            frames -= discard;
            continue;
        }
        let Some(block) = pending.pop_front() else {
            break;
        };
        *current = block;
        *cursor = 0;
    }
    requested - frames
}

#[derive(Debug)]
struct PlaybackContinuity {
    last_output: [f32; 2],
    conceal_anchor: [f32; 2],
    starvation_frames: usize,
    recovery_gain: f32,
    splice_anchor: [f32; 2],
    splice_frames_remaining: usize,
}

impl Default for PlaybackContinuity {
    fn default() -> Self {
        Self {
            last_output: [0.0, 0.0],
            conceal_anchor: [0.0, 0.0],
            starvation_frames: 0,
            recovery_gain: 1.0,
            splice_anchor: [0.0, 0.0],
            splice_frames_remaining: 0,
        }
    }
}

impl PlaybackContinuity {
    fn begin_splice_recovery(&mut self) {
        self.splice_anchor = self.last_output;
        self.splice_frames_remaining = PLAYBACK_CONCEAL_FRAMES;
        self.starvation_frames = 0;
        self.recovery_gain = 1.0;
    }

    fn render(&mut self, input: Option<(f32, f32)>) -> (f32, f32) {
        match input {
            Some((left, right)) => {
                if self.splice_frames_remaining != 0 {
                    let completed = PLAYBACK_CONCEAL_FRAMES - self.splice_frames_remaining;
                    let t = (completed + 1) as f32 / PLAYBACK_CONCEAL_FRAMES as f32;
                    let output = [
                        self.splice_anchor[0] + (left - self.splice_anchor[0]) * t,
                        self.splice_anchor[1] + (right - self.splice_anchor[1]) * t,
                    ];
                    self.splice_frames_remaining -= 1;
                    self.last_output = output;
                    return (output[0], output[1]);
                }

                if self.starvation_frames > 0 {
                    let missing = self.starvation_frames.min(PLAYBACK_CONCEAL_FRAMES);
                    self.recovery_gain =
                        (1.0 - missing as f32 / PLAYBACK_CONCEAL_FRAMES as f32).max(0.0);
                    self.starvation_frames = 0;
                }

                let gain = self.recovery_gain;
                let output = [left * gain, right * gain];
                self.recovery_gain =
                    (self.recovery_gain + 1.0 / PLAYBACK_CONCEAL_FRAMES as f32).min(1.0);
                self.last_output = output;
                (output[0], output[1])
            }
            None => {
                self.splice_frames_remaining = 0;
                if self.starvation_frames == 0 {
                    self.conceal_anchor = self.last_output;
                }
                self.starvation_frames = self.starvation_frames.saturating_add(1);
                let missing = self.starvation_frames.min(PLAYBACK_CONCEAL_FRAMES);
                let gain =
                    (1.0 - missing as f32 / PLAYBACK_CONCEAL_FRAMES as f32).max(0.0);
                let output = [self.conceal_anchor[0] * gain, self.conceal_anchor[1] * gain];
                self.last_output = output;
                (output[0], output[1])
            }
        }
    }
}

fn build_typed_playback<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    rx: Receiver<Vec<f32>>,
    telemetry: PlaybackTelemetry,
    stream_failed: Arc<AtomicBool>,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::Sample + cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut current = Vec::<f32>::new();
    let mut cursor = 0usize;
    let mut pending = VecDeque::<Vec<f32>>::new();
    let mut latency = PlaybackLatencyGovernor::default();
    let mut continuity = PlaybackContinuity::default();
    let mut callback_mmcss = None;
    let err_fn = move |err| {
        eprintln!("WASAPI playback stream error: {err}");
        stream_failed.store(true, Ordering::Release);
    };
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                if callback_mmcss.is_none() {
                    callback_mmcss = crate::realtime_priority::claim_realtime_audio();
                }

                let preparation = latency.prepare(&rx, &mut current, &mut cursor, &mut pending);
                telemetry
                    .buffered_frames
                    .store(preparation.buffered_frames as u64, Ordering::Relaxed);
                if preparation.trimmed_frames != 0 {
                    telemetry
                        .trimmed_frames
                        .fetch_add(preparation.trimmed_frames as u64, Ordering::Relaxed);
                    continuity.begin_splice_recovery();
                } else if preparation.became_audible {
                    continuity.begin_splice_recovery();
                }

                for frame in data.chunks_exact_mut(channels) {
                    let next = if preparation.audible {
                        try_next_stereo_frame(&rx, &mut current, &mut cursor, &mut pending)
                    } else {
                        None
                    };
                    if preparation.audible && next.is_none() {
                        telemetry.underrun_frames.fetch_add(1, Ordering::Relaxed);
                    }
                    let (left, right) = continuity.render(next);
                    frame[0] = T::from_sample(left);
                    frame[1] = T::from_sample(right);
                    for sample in &mut frame[2..] {
                        *sample = T::from_sample(0.0);
                    }
                }
            },
            err_fn,
            None,
        )
        .context("failed to create WASAPI playback stream")
}

fn try_next_stereo_frame(
    rx: &Receiver<Vec<f32>>,
    current: &mut Vec<f32>,
    cursor: &mut usize,
    pending: &mut VecDeque<Vec<f32>>,
) -> Option<(f32, f32)> {
    loop {
        if *cursor + 1 < current.len() {
            let pair = (current[*cursor], current[*cursor + 1]);
            *cursor += 2;
            return Some(pair);
        }
        if let Some(block) = pending.pop_front() {
            *current = block;
            *cursor = 0;
            continue;
        }
        match rx.try_recv() {
            Ok(block) => {
                *current = block;
                *cursor = 0;
            }
            Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => return None,
        }
    }
}

fn spawn_quit_control(quit: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        loop {
            let mut line = String::new();
            match stdin.read_line(&mut line) {
                Ok(0) | Err(_) => {
                    quit.store(true, Ordering::Relaxed);
                    break;
                }
                Ok(_) if line.trim().eq_ignore_ascii_case("q") => {
                    quit.store(true, Ordering::Relaxed);
                    break;
                }
                Ok(_) => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo_block(frames: usize, value: f32) -> Vec<f32> {
        let mut block = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            block.extend_from_slice(&[value, -value]);
        }
        block
    }

    #[test]
    fn lookahead_peak_guard_keeps_makeup_spike_below_ceiling() {
        let mut guard = StereoLookaheadPeakGuard::new(SAMPLE_RATE_HZ);
        let mut input = Vec::with_capacity(900 * 2);
        for frame in 0..900 {
            let sample = if frame == 430 { 1.0 } else { 0.10 };
            input.extend_from_slice(&[sample, -sample * 0.70]);
        }
        let output = guard.process_interleaved(&input).expect("peak guard");
        assert!(!output.is_empty());
        assert!(
            output
                .iter()
                .all(|sample| sample.abs() <= OUTPUT_CEILING + 1.0e-5)
        );
    }

    #[test]
    fn playback_starvation_is_concealed_instead_of_hard_zeroed() {
        let mut continuity = PlaybackContinuity::default();
        let live = continuity.render(Some((0.50, -0.25)));
        assert!((live.0 - 0.50).abs() < 1.0e-6);
        assert!((live.1 + 0.25).abs() < 1.0e-6);

        let first_missing = continuity.render(None);
        assert!(first_missing.0 > 0.0 && first_missing.0 < live.0);
        assert!(first_missing.1 < 0.0 && first_missing.1 > live.1);

        let mut fully_concealed = first_missing;
        for _ in 1..PLAYBACK_CONCEAL_FRAMES {
            fully_concealed = continuity.render(None);
        }
        assert!(fully_concealed.0.abs() < 1.0e-6);
        assert!(fully_concealed.1.abs() < 1.0e-6);

        let resume = continuity.render(Some((0.50, -0.25)));
        assert!(resume.0.abs() < 1.0e-6);
        let next = continuity.render(Some((0.50, -0.25)));
        assert!(next.0 > resume.0);
        assert!(next.0 < 0.50);
    }

    #[test]
    fn cold_start_overfill_is_trimmed_to_target_before_audio_opens() {
        let (tx, rx) = sync_channel::<Vec<f32>>(PLAYBACK_QUEUE_BLOCKS);
        let packet_frames = playback_frames_for_ms(20);
        for index in 0..25 {
            tx.try_send(stereo_block(packet_frames, index as f32 / 25.0))
                .expect("queue capacity covers synthetic cold-start burst");
        }

        let mut governor = PlaybackLatencyGovernor::default();
        let mut current = Vec::new();
        let mut cursor = 0usize;
        let mut pending = VecDeque::new();
        let preparation = governor.prepare(&rx, &mut current, &mut cursor, &mut pending);

        assert!(preparation.audible);
        assert_eq!(preparation.buffered_frames, playback_frames_for_ms(PLAYBACK_TARGET_LATENCY_MS));
        assert!(preparation.trimmed_frames >= playback_frames_for_ms(400));
    }

    #[test]
    fn startup_waits_for_target_then_opens_without_extra_history() {
        let (tx, rx) = sync_channel::<Vec<f32>>(PLAYBACK_QUEUE_BLOCKS);
        let packet_frames = playback_frames_for_ms(20);
        let mut governor = PlaybackLatencyGovernor::default();
        let mut current = Vec::new();
        let mut cursor = 0usize;
        let mut pending = VecDeque::new();

        tx.try_send(stereo_block(packet_frames, 0.1)).unwrap();
        let first = governor.prepare(&rx, &mut current, &mut cursor, &mut pending);
        assert!(!first.audible);
        assert_eq!(first.trimmed_frames, 0);

        tx.try_send(stereo_block(packet_frames, 0.2)).unwrap();
        let second = governor.prepare(&rx, &mut current, &mut cursor, &mut pending);
        assert!(second.audible);
        assert!(second.became_audible);
        assert_eq!(second.buffered_frames, playback_frames_for_ms(PLAYBACK_TARGET_LATENCY_MS));
        assert_eq!(second.trimmed_frames, 0);
    }

    #[test]
    fn high_backlog_recovery_discards_oldest_audio_back_to_target() {
        let (tx, rx) = sync_channel::<Vec<f32>>(PLAYBACK_QUEUE_BLOCKS);
        let packet_frames = playback_frames_for_ms(20);
        let mut governor = PlaybackLatencyGovernor::default();
        let mut current = Vec::new();
        let mut cursor = 0usize;
        let mut pending = VecDeque::new();

        for _ in 0..2 {
            tx.try_send(stereo_block(packet_frames, 0.1)).unwrap();
        }
        let start = governor.prepare(&rx, &mut current, &mut cursor, &mut pending);
        assert!(start.audible);

        for _ in 0..8 {
            tx.try_send(stereo_block(packet_frames, 0.2)).unwrap();
        }
        let recovered = governor.prepare(&rx, &mut current, &mut cursor, &mut pending);
        assert!(recovered.audible);
        assert!(recovered.trimmed_frames > 0);
        assert_eq!(recovered.buffered_frames, playback_frames_for_ms(PLAYBACK_TARGET_LATENCY_MS));
    }

    #[test]
    fn low_inventory_reenters_refill_before_resuming() {
        let (tx, rx) = sync_channel::<Vec<f32>>(PLAYBACK_QUEUE_BLOCKS);
        let packet_frames = playback_frames_for_ms(20);
        let mut governor = PlaybackLatencyGovernor::default();
        let mut current = Vec::new();
        let mut cursor = 0usize;
        let mut pending = VecDeque::new();

        for _ in 0..2 {
            tx.try_send(stereo_block(packet_frames, 0.1)).unwrap();
        }
        assert!(governor.prepare(&rx, &mut current, &mut cursor, &mut pending).audible);

        let available = buffered_playback_frames(&current, cursor, &pending);
        let dropped = discard_oldest_playback_frames(
            &mut current,
            &mut cursor,
            &mut pending,
            available,
        );
        assert_eq!(dropped, available);

        let empty = governor.prepare(&rx, &mut current, &mut cursor, &mut pending);
        assert!(!empty.audible);

        tx.try_send(stereo_block(packet_frames, 0.2)).unwrap();
        assert!(!governor.prepare(&rx, &mut current, &mut cursor, &mut pending).audible);
        tx.try_send(stereo_block(packet_frames, 0.3)).unwrap();
        assert!(governor.prepare(&rx, &mut current, &mut cursor, &mut pending).audible);
    }
}
