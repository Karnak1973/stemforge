//! Track model, demo-track synthesis and audio-file analysis.
//!
//! Demo tracks are generated as four real component buses (vocals, drums,
//! bass, instruments) so that muting a stem is sample-accurate. Loaded files
//! go through a spectral four-band split; the buses always sum back to the
//! original signal.

use std::f32::consts::TAU;
use std::path::Path;

use symphonia::core::audio::{SampleBuffer, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const SR: f32 = 44_100.0;
pub const STEM_COUNT: usize = 4;
pub const STEM_VOCALS: usize = 0;
pub const STEM_DRUMS: usize = 1;
pub const STEM_BASS: usize = 2;
pub const STEM_OTHER: usize = 3;
pub const PEAK_BUCKETS: usize = 1600;

#[derive(Clone)]
pub struct TrackData {
    pub name: String,
    pub artist: String,
    pub bpm: f32,
    pub key: String,
    pub source: String,
    pub stems: [Vec<f32>; STEM_COUNT],
    pub peaks: Vec<[f32; STEM_COUNT]>,
    pub duration: f32,
    pub is_demo: bool,
}

impl TrackData {
    pub fn frames(&self) -> usize {
        self.stems[0].len()
    }

    pub fn duration_secs(&self) -> f32 {
        self.duration
    }

    /// Peak values around a playback position, for the scrolling waveform.
    pub fn window_peaks(
        &self,
        center_sec: f64,
        half_window_sec: f64,
        columns: usize,
    ) -> Vec<[f32; STEM_COUNT]> {
        let buckets = self.peaks.len();
        let mut out = Vec::with_capacity(columns);
        if buckets == 0 || columns == 0 {
            return out;
        }
        let dur = self.duration.max(0.001) as f64;
        for x in 0..columns {
            let frac = (x as f64 + 0.5) / columns as f64;
            let t = center_sec - half_window_sec + frac * half_window_sec * 2.0;
            let idx = ((t / dur) * buckets as f64) as isize;
            let idx = idx.clamp(0, buckets as isize - 1) as usize;
            out.push(self.peaks[idx]);
        }
        out
    }
}

#[derive(Clone, Copy)]
pub struct DemoMeta {
    pub name: &'static str,
    pub artist: &'static str,
    pub bpm: f32,
    pub key: &'static str,
    pub accent: u8,
}

pub fn demo_catalog() -> &'static [DemoMeta] {
    &[
        DemoMeta {
            name: "Neon Drive",
            artist: "Metro Static",
            bpm: 124.0,
            key: "8A · Am",
            accent: 0,
        },
        DemoMeta {
            name: "Midnight Bloom",
            artist: "Vela Grey",
            bpm: 122.0,
            key: "7A · Dm",
            accent: 1,
        },
        DemoMeta {
            name: "Glass Horizon",
            artist: "North Arcade",
            bpm: 126.0,
            key: "11A · F#m",
            accent: 2,
        },
        DemoMeta {
            name: "Velvet Circuit",
            artist: "Low Atlas",
            bpm: 120.0,
            key: "9A · Em",
            accent: 3,
        },
    ]
}

#[derive(Clone, Copy)]
struct SongSpec {
    name: &'static str,
    artist: &'static str,
    bpm: f32,
    key_name: &'static str,
    root_midi: i32,
    progression: [i32; 4],
    seed: u64,
    energy: f32,
}

fn spec(index: usize) -> SongSpec {
    match index {
        1 => SongSpec {
            name: "Midnight Bloom",
            artist: "Vela Grey",
            bpm: 122.0,
            key_name: "7A · Dm",
            root_midi: 38,
            progression: [0, -4, 3, -2],
            seed: 0x51A9_22C4,
            energy: 0.9,
        },
        2 => SongSpec {
            name: "Glass Horizon",
            artist: "North Arcade",
            bpm: 126.0,
            key_name: "11A · F#m",
            root_midi: 30,
            progression: [0, 3, -2, -4],
            seed: 0x99C3_71E2,
            energy: 1.0,
        },
        3 => SongSpec {
            name: "Velvet Circuit",
            artist: "Low Atlas",
            bpm: 120.0,
            key_name: "9A · Em",
            root_midi: 40,
            progression: [0, -4, -2, 3],
            seed: 0x2D77_0B18,
            energy: 0.85,
        },
        _ => SongSpec {
            name: "Neon Drive",
            artist: "Metro Static",
            bpm: 124.0,
            key_name: "8A · Am",
            root_midi: 33,
            progression: [0, -4, 3, -2],
            seed: 0x7E11_00AF,
            energy: 1.0,
        },
    }
}

fn midi_hz(m: f32) -> f32 {
    440.0 * 2.0_f32.powf((m - 69.0) / 12.0)
}

/// Builds one demo track, fully separated into four stems.
pub fn synth_demo(index: usize) -> TrackData {
    let s = spec(index);
    let beat = 60.0 / s.bpm as f64;
    let bars = 24usize;
    let total = (bars as f64 * 4.0 * beat * SR as f64) as usize;
    let mut stems: [Vec<f32>; STEM_COUNT] = std::array::from_fn(|_| vec![0.0; total]);
    let mut noise = Lcg::new(s.seed);

    // ---------------- drums ----------------
    {
        let drums = &mut stems[STEM_DRUMS];
        for bar in 0..bars {
            for beat_i in 0..4 {
                let t = (bar * 4 + beat_i) as f64 * beat;
                match beat_i {
                    0 => {
                        kick(drums, t, 0.62 * s.energy);
                        if bar % 4 == 3 {
                            kick(drums, t + beat * 0.5, 0.34 * s.energy);
                        }
                    }
                    2 => kick(drums, t, 0.48 * s.energy),
                    1 | 3 => snare(drums, t, 0.30 * s.energy, &mut noise),
                    _ => {}
                }
                hat(drums, t, 0.10 * s.energy, &mut noise, false);
                hat(drums, t + beat * 0.5, 0.075 * s.energy, &mut noise, beat_i == 3);
            }
            // small fill at the end of every eighth bar
            if bar % 8 == 7 {
                for k in 0..4 {
                    snare(
                        drums,
                        (bar * 4) as f64 * beat + beat * (3.25 + k as f64 * 0.125),
                        0.12 * s.energy,
                        &mut noise,
                    );
                }
            }
        }
    }

    // ---------------- bass ----------------
    {
        let bass = &mut stems[STEM_BASS];
        let pattern = [0, 0, 12, 0, 7, 0, 12, 10];
        for bar in 0..bars {
            let root = s.root_midi + s.progression[bar % 4];
            for step in 0..8 {
                let t = (bar * 4) as f64 * beat + step as f64 * beat * 0.5;
                let note = root + pattern[step];
                let dur = beat * 0.42;
                let f = midi_hz(note as f32);
                tone(
                    bass,
                    t,
                    dur,
                    0.30 * s.energy,
                    f,
                    &[1.0, 0.32, 0.16, 0.08],
                    0.004,
                    0.35,
                    0.0,
                    false,
                );
                tone(
                    bass,
                    t,
                    dur * 0.9,
                    0.16 * s.energy,
                    f * 0.5,
                    &[1.0],
                    0.005,
                    0.35,
                    0.0,
                    false,
                );
            }
        }
    }

    // ---------------- chords / melodic bus ----------------
    {
        let other = &mut stems[STEM_OTHER];
        for bar in 0..bars {
            let root = s.root_midi + s.progression[bar % 4];
            let chord_start = (bar * 4) as f64 * beat;
            let chord_len = beat * 4.0;
            // warm minor pad
            for (i, interval) in [12, 15, 19, 24].iter().enumerate() {
                let f = midi_hz((root + interval) as f32);
                tone(
                    other,
                    chord_start,
                    chord_len * 1.02,
                    0.042 * s.energy,
                    f * if i == 3 { 0.5 } else { 1.0 },
                    &[1.0, 0.42, 0.22, 0.1],
                    0.35,
                    0.55,
                    0.0,
                    false,
                );
            }
            // off-beat arpeggio pluck
            let arp = [0, 7, 12, 3, 7, 12, 15, 7];
            for step in 0..8 {
                let t = chord_start + step as f64 * beat * 0.5;
                let note = root + 24 + arp[(step + bar) % arp.len()];
                tone(
                    other,
                    t,
                    beat * 0.30,
                    0.055 * s.energy,
                    midi_hz(note as f32),
                    &[1.0, 0.25, 0.4, 0.12],
                    0.003,
                    0.20,
                    0.0,
                    false,
                );
            }
        }
    }

    // ---------------- lead / vocal bus ----------------
    {
        let vocals = &mut stems[STEM_VOCALS];
        let scale = [0, 3, 5, 7, 10, 12, 15, 7, 5, 3];
        let phrase_starts = [4usize, 12, 20];
        for (pi, start_bar) in phrase_starts.iter().enumerate() {
            for k in 0..16 {
                if k % 4 == 3 {
                    continue;
                }
                let bar = start_bar + k / 4;
                let root = s.root_midi + s.progression[bar % 4];
                let deg = scale[(k + pi * 3) % scale.len()];
                let note = root + 36 + deg;
                let t = (bar * 4) as f64 * beat + (k % 4) as f64 * beat;
                let dur = beat * if k % 2 == 0 { 0.85 } else { 0.55 };
                let f = midi_hz(note as f32);
                tone(
                    vocals,
                    t,
                    dur,
                    0.16 * s.energy,
                    f,
                    &[1.0, 0.30, 0.16, 0.07],
                    0.05,
                    0.30,
                    0.006,
                    true,
                );
                // harmonic shadow, gives the bus a vocal-ish formant colour
                tone(
                    vocals,
                    t,
                    dur,
                    0.055 * s.energy,
                    f * 2.7,
                    &[1.0, 0.4],
                    0.06,
                    0.28,
                    0.006,
                    true,
                );
            }
        }
    }

    let lower = (total as f64 * 0.02) as usize;
    for stem in stems.iter_mut() {
        fade_edges(stem, lower);
    }

    let peaks = compute_peaks(&stems);
    TrackData {
        name: s.name.to_string(),
        artist: s.artist.to_string(),
        bpm: s.bpm,
        key: s.key_name.to_string(),
        source: "Stem-native demo".to_string(),
        stems,
        peaks,
        duration: total as f32 / SR,
        is_demo: true,
    }
}

fn fade_edges(buf: &mut [f32], edge: usize) {
    let n = buf.len();
    let edge = edge.min(n / 2);
    for i in 0..edge {
        let g = i as f32 / edge as f32;
        buf[i] *= g;
        buf[n - 1 - i] *= g;
    }
}

fn compute_peaks(stems: &[Vec<f32>; STEM_COUNT]) -> Vec<[f32; STEM_COUNT]> {
    let len = stems[0].len().max(1);
    let mut peaks = vec![[0.0f32; STEM_COUNT]; PEAK_BUCKETS];
    for (b, slot) in peaks.iter_mut().enumerate() {
        let start = b * len / PEAK_BUCKETS;
        let end = ((b + 1) * len / PEAK_BUCKETS).max(start + 1).min(len);
        for (si, stem) in stems.iter().enumerate() {
            let mut peak = 0.0f32;
            for &v in &stem[start..end] {
                let a = v.abs();
                if a > peak {
                    peak = a;
                }
            }
            slot[si] = peak.min(1.0);
        }
    }
    peaks
}

// ---------------------------------------------------------------------------
// synthesis primitives
// ---------------------------------------------------------------------------

struct Lcg(u64);

impl Lcg {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }
    fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32 / (1u64 << 31) as f32) - 1.0
    }
}

fn kick(buf: &mut [f32], start: f64, amp: f32) {
    let s0 = (start * SR as f64) as usize;
    let n = (0.42 * SR) as usize;
    let mut phase = 0.0f32;
    for i in 0..n {
        let idx = s0 + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / SR;
        let f = 44.0 + 118.0 * (-t * 34.0).exp();
        phase += TAU * f / SR;
        let env = (-t * 8.5).exp() * (t * 400.0).min(1.0);
        buf[idx] += phase.sin() * amp * env;
    }
}

fn snare(buf: &mut [f32], start: f64, amp: f32, rng: &mut Lcg) {
    let s0 = (start * SR as f64) as usize;
    let n = (0.24 * SR) as usize;
    for i in 0..n {
        let idx = s0 + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / SR;
        let env = (-t * 18.0).exp();
        let body = (TAU * 186.0 * t).sin() * 0.35;
        buf[idx] += (rng.next_f32() * 0.65 + body) * amp * env;
    }
}

fn hat(buf: &mut [f32], start: f64, amp: f32, rng: &mut Lcg, open: bool) {
    let s0 = (start * SR as f64) as usize;
    let n = ((if open { 0.20 } else { 0.055 }) * SR) as usize;
    let decay = if open { 20.0 } else { 68.0 };
    for i in 0..n {
        let idx = s0 + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / SR;
        let env = (-t * decay).exp();
        let noise = rng.next_f32();
        let edge = if i % 2 == 0 { 1.0 } else { -1.0 };
        buf[idx] += (noise * 0.6 + edge * 0.4) * amp * env;
    }
}

#[allow(clippy::too_many_arguments)]
fn tone(
    buf: &mut [f32],
    start: f64,
    dur: f64,
    amp: f32,
    freq: f32,
    harmonics: &[f32],
    attack: f32,
    release: f32,
    vibrato: f32,
    breath: bool,
) {
    let s0 = (start * SR as f64) as usize;
    let n = (dur * SR as f64) as usize;
    if n == 0 {
        return;
    }
    let mut noise = Lcg::new((freq * 1000.0) as u64 ^ 0x9E37_79B9);
    for i in 0..n {
        let idx = s0 + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / SR;
        let att = if attack > 0.0 { (t / attack).min(1.0) } else { 1.0 };
        let rel = if release > 0.0 {
            ((dur as f32 - t) / release).min(1.0).max(0.0)
        } else {
            1.0
        };
        let f = freq * (1.0 + vibrato * (TAU * 5.3 * t).sin());
        let phase = TAU * f * t;
        let mut v = 0.0;
        for (h, a) in harmonics.iter().enumerate() {
            v += a * (phase * (h + 1) as f32).sin();
        }
        if breath {
            v += noise.next_f32() * 0.05;
        }
        buf[idx] += v * amp * att * rel;
    }
}

// ---------------------------------------------------------------------------
// file loading
// ---------------------------------------------------------------------------

pub fn analyze_file(
    path: &Path,
    progress: &mut dyn FnMut(f32, &str),
) -> Result<TrackData, String> {
    progress(0.02, "Reading file");
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open file: {e}"))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions {
                enable_gapless: true,
                ..Default::default()
            },
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Unsupported audio file: {e}"))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .or_else(|| format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL))
        .ok_or_else(|| "No audio track found".to_string())?;
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let src_sr = codec_params.sample_rate.unwrap_or(44_100);
    let total_frames = codec_params.n_frames;
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| format!("No decoder for this file: {e}"))?;

    let max_seconds = 12.0 * 60.0;
    let max_samples = (max_seconds * src_sr as f64) as usize;
    let mut mono: Vec<f32> = Vec::new();
    let mut last_report = 0.0f32;

    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let ch = spec.channels.count().max(1);
                let mut sb = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                sb.copy_interleaved_ref(decoded);
                if ch == 1 {
                    mono.extend_from_slice(sb.samples());
                } else {
                    for frame in sb.samples().chunks(ch) {
                        mono.push(frame.iter().sum::<f32>() / ch as f32);
                    }
                }
                if mono.len() >= max_samples {
                    mono.truncate(max_samples);
                    break;
                }
                let done = total_frames
                    .map(|t| mono.len() as f32 / t.max(1) as f32)
                    .unwrap_or(0.0);
                if done - last_report > 0.04 {
                    last_report = done;
                    progress(0.04 + done.min(1.0) * 0.5, "Decoding audio");
                }
            }
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }

    if mono.len() < 2000 {
        return Err("That file has no usable audio".to_string());
    }
    progress(0.56, "Resampling");
    let mono = resample_linear(&mono, src_sr, SR as u32);
    progress(0.62, "Splitting stems");

    let mut stems = split_stems(&mono);
    // Bring the whole track back to a safe headroom without changing the sum.
    let peak = stems
        .iter()
        .flat_map(|s| s.iter())
        .fold(0.0f32, |m, v| m.max(v.abs()));
    if peak > 0.98 {
        let g = 0.98 / peak;
        for stem in stems.iter_mut() {
            for v in stem.iter_mut() {
                *v *= g;
            }
        }
    }
    progress(0.94, "Drawing waveform");
    let peaks = compute_peaks(&stems);
    let duration = mono.len() as f32 / SR;
    let bpm = estimate_bpm(&mono);
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled track")
        .to_string();
    let (artist, source) = file_origin(path);
    progress(1.0, "Ready");
    Ok(TrackData {
        name,
        artist,
        bpm,
        key: "—".to_string(),
        source,
        stems,
        peaks,
        duration,
        is_demo: false,
    })
}

fn file_origin(path: &Path) -> (String, String) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_uppercase())
        .unwrap_or_else(|| "AUDIO".to_string());
    ("Local file".to_string(), format!("{ext} · spectral stems"))
}

fn resample_linear(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if input.is_empty() || from == 0 {
        return input.to_vec();
    }
    if from == to {
        return input.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let out_len = (input.len() as f64 * ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let i0 = src.floor() as usize;
        let frac = (src - i0 as f64) as f32;
        let a = input.get(i0).copied().unwrap_or(0.0);
        let b = input.get(i0 + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

/// Spectral split used for imported files. `other` is the residual, so the
/// four buses reconstruct the source exactly.
fn split_stems(input: &[f32]) -> [Vec<f32>; STEM_COUNT] {
    let mut bass = filter_offline(input, BiquadKind::LowPass, 130.0, 0.72);
    let mut drums = filter_offline(input, BiquadKind::LowPass, 620.0, 0.8);
    drums = filter_offline(&drums, BiquadKind::HighPass, 110.0, 0.8);
    let mut vocals = filter_offline(input, BiquadKind::HighPass, 700.0, 0.7);
    vocals = filter_offline(&vocals, BiquadKind::LowPass, 4200.0, 0.7);
    let mut other = Vec::with_capacity(input.len());
    for i in 0..input.len() {
        let residual = input[i] - bass[i] - drums[i] - vocals[i];
        other.push(residual);
    }
    // remove DC / subsonic rumble from the bass bus only
    bass = filter_offline(&bass, BiquadKind::HighPass, 28.0, 0.7);
    [vocals, drums, bass, other]
}

#[derive(Clone, Copy)]
enum BiquadKind {
    LowPass,
    HighPass,
}

fn filter_offline(input: &[f32], kind: BiquadKind, cutoff: f32, q: f32) -> Vec<f32> {
    let mut bq = Biquad::new();
    bq.set(kind, cutoff, q, SR);
    input.iter().map(|&v| bq.process(v)).collect()
}

fn estimate_bpm(mono: &[f32]) -> f32 {
    let hop = 512usize;
    let env: Vec<f32> = mono
        .chunks(hop)
        .map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt())
        .collect();
    if env.len() < 64 {
        return 120.0;
    }
    let mean = env.iter().sum::<f32>() / env.len() as f32;
    let centered: Vec<f32> = env.iter().map(|v| v - mean).collect();
    let mut best_bpm = 120.0f32;
    let mut best = f32::NEG_INFINITY;
    let frame_rate = SR / hop as f32;
    for bpm in 70..=180 {
        let lag = (60.0 * frame_rate / bpm as f32).round() as usize;
        if lag < 2 || lag >= centered.len() {
            continue;
        }
        let mut acc = 0.0f32;
        for i in 0..centered.len() - lag {
            acc += centered[i] * centered[i + lag];
        }
        acc /= (centered.len() - lag) as f32;
        if acc > best {
            best = acc;
            best_bpm = bpm as f32;
        }
    }
    // prefer the common 2x octave when the raw estimate is very slow
    if best_bpm < 95.0 && best_bpm * 2.0 <= 180.0 {
        best_bpm *= 2.0;
    }
    best_bpm
}

// ---------------------------------------------------------------------------
// biquad (shared with the realtime engine)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
pub struct Coeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Coeffs {
    pub fn identity() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }

    pub fn lowpass(sr: f32, f0: f32, q: f32) -> Self {
        let (w0, cos, alpha) = w0_alpha(sr, f0, q);
        let b1 = 1.0 - cos;
        let b0 = b1 * 0.5;
        let a0 = 1.0 + alpha;
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b0 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    pub fn highpass(sr: f32, f0: f32, q: f32) -> Self {
        let (_, cos, alpha) = w0_alpha(sr, f0, q);
        let b0 = (1.0 + cos) * 0.5;
        let a0 = 1.0 + alpha;
        Self {
            b0: b0 / a0,
            b1: -(1.0 + cos) / a0,
            b2: b0 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
        }
    }

    pub fn peaking(sr: f32, f0: f32, q: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let (_, cos, alpha) = w0_alpha(sr, f0, q);
        let a0 = 1.0 + alpha / a;
        Self {
            b0: (1.0 + alpha * a) / a0,
            b1: (-2.0 * cos) / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: (-2.0 * cos) / a0,
            a2: (1.0 - alpha / a) / a0,
        }
    }

    pub fn low_shelf(sr: f32, f0: f32, gain_db: f32) -> Self {
        shelf(sr, f0, gain_db, true)
    }

    pub fn high_shelf(sr: f32, f0: f32, gain_db: f32) -> Self {
        shelf(sr, f0, gain_db, false)
    }
}

fn w0_alpha(sr: f32, f0: f32, q: f32) -> (f32, f32, f32) {
    let w0 = TAU * f0.clamp(10.0, sr * 0.45) / sr;
    (w0, w0.cos(), w0.sin() / (2.0 * q.max(0.05)))
}

fn shelf(sr: f32, f0: f32, gain_db: f32, low: bool) -> Coeffs {
    let a = 10f32.powf(gain_db / 40.0);
    let w0 = TAU * f0.clamp(10.0, sr * 0.45) / sr;
    let cos = w0.cos();
    let sin = w0.sin();
    let alpha = sin / 2.0 * (2.0f32).sqrt();
    let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
    let a0 = (a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha;
    if low {
        let b0 = a * ((a + 1.0) - (a - 1.0) * cos + two_sqrt_a_alpha);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cos);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha);
        Coeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: -2.0 * ((a - 1.0) + (a + 1.0) * cos) / a0,
            a2: ((a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha) / a0,
        }
    } else {
        let b0 = a * ((a + 1.0) + (a - 1.0) * cos + two_sqrt_a_alpha);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cos);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cos - two_sqrt_a_alpha);
        Coeffs {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: 2.0 * ((a - 1.0) - (a + 1.0) * cos) / a0,
            a2: ((a + 1.0) - (a - 1.0) * cos - two_sqrt_a_alpha) / a0,
        }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Biquad {
    c: Coeffs,
    z1: f32,
    z2: f32,
}

impl Biquad {
    pub fn new() -> Self {
        Self {
            c: Coeffs::identity(),
            z1: 0.0,
            z2: 0.0,
        }
    }

    pub fn set_coeffs(&mut self, c: Coeffs) {
        self.c = c;
    }

    pub fn set(&mut self, kind: BiquadKind, cutoff: f32, q: f32, sr: f32) {
        self.c = match kind {
            BiquadKind::LowPass => Coeffs::lowpass(sr, cutoff, q),
            BiquadKind::HighPass => Coeffs::highpass(sr, cutoff, q),
        };
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.c.b0 * x + self.z1;
        self.z1 = self.c.b1 * x - self.c.a1 * y + self.z2;
        self.z2 = self.c.b2 * x - self.c.a2 * y;
        y
    }

    pub fn reset(&mut self) {
        self.z1 = 0.0;
        self.z2 = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    #[test]
    fn imported_wav_splits_into_four_stems() {
        let dir = std::env::temp_dir().join("stemforge_track_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("tone.wav");
        let sr = 44_100usize;
        let mut samples = Vec::with_capacity(sr * 3 * 2);
        for i in 0..sr * 3 {
            let t = i as f32 / sr as f32;
            let l = (TAU * 110.0 * t).sin() * 0.25 + (TAU * 1_800.0 * t).sin() * 0.15;
            let r = (TAU * 220.0 * t).sin() * 0.2 + (TAU * 6_000.0 * t).sin() * 0.1;
            samples.push(l);
            samples.push(r);
        }
        crate::audio::write_wav(&path, &samples, sr as u32).unwrap();
        let mut last = 0.0f32;
        let track = analyze_file(&path, &mut |p, _| last = p).expect("wav decodes");
        assert!(last >= 0.9);
        assert!(track.duration > 2.5 && track.duration < 4.0);
        let frames = track.frames();
        assert!(frames > sr * 2);
        for s in 0..STEM_COUNT {
            assert_eq!(track.stems[s].len(), frames);
            let energy: f32 = track.stems[s].iter().map(|v| v * v).sum();
            assert!(energy > 0.0, "stem {s} has no energy");
        }
        let _ = std::fs::remove_file(&path);
    }
}
