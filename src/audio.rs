//! Low-latency mixer engine: cpal output, per-deck transport, 3-band EQ,
//! bipolar DJ filter, four stem gains, equal-power crossfader and recording.

use std::f32::consts::PI;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample};

use crate::track::{Coeffs, Biquad, TrackData, SR, STEM_COUNT};

#[derive(Clone)]
pub struct DeckParams {
    pub playing: bool,
    pub position: f64,
    pub rate: f64,
    pub gain: f32,
    pub trim: f32,
    pub eq_db: [f32; 3],
    pub kill: [bool; 3],
    pub filter: f32,
    pub stem_gain: [f32; STEM_COUNT],
    pub stem_mute: [bool; STEM_COUNT],
    pub stem_solo: [bool; STEM_COUNT],
    pub loop_active: bool,
    pub loop_in: f64,
    pub loop_out: f64,
    pub cue: f64,
}

impl Default for DeckParams {
    fn default() -> Self {
        Self {
            playing: false,
            position: 0.0,
            rate: 1.0,
            gain: 0.82,
            trim: 1.0,
            eq_db: [0.0; 3],
            kill: [false; 3],
            filter: 0.0,
            stem_gain: [1.0; STEM_COUNT],
            stem_mute: [false; STEM_COUNT],
            stem_solo: [false; STEM_COUNT],
            loop_active: false,
            loop_in: 0.0,
            loop_out: 0.0,
            cue: 0.0,
        }
    }
}

struct DeckDsp {
    low: Biquad,
    mid: Biquad,
    high: Biquad,
    filter: Biquad,
    last_filter_sign: f32,
    sm_eq: [f32; 3],
    sm_filter: f32,
    sm_gain: f32,
    sm_xfade: f32,
    sm_stem: [f32; STEM_COUNT],
}

impl Default for DeckDsp {
    fn default() -> Self {
        Self {
            low: Biquad::new(),
            mid: Biquad::new(),
            high: Biquad::new(),
            filter: Biquad::new(),
            last_filter_sign: 0.0,
            sm_eq: [0.0; 3],
            sm_filter: 0.0,
            sm_gain: 0.82,
            sm_xfade: 0.707,
            sm_stem: [1.0; STEM_COUNT],
        }
    }
}

pub struct Engine {
    pub decks: [Option<Arc<TrackData>>; 2],
    pub p: [DeckParams; 2],
    pub crossfader: f32,
    pub master: f32,
    pub levels: [f32; 2],
    pub master_level: [f32; 2],
    pub recording: Option<Vec<f32>>,
    pub sample_rate: f32,
    pub deck_sr: f32,
    dsp: [DeckDsp; 2],
    scratch: Vec<f32>,
    record_cap: usize,
}

impl Engine {
    pub fn new() -> Self {
        Self {
            decks: [None, None],
            p: [DeckParams::default(), DeckParams::default()],
            crossfader: 0.5,
            master: 0.72,
            levels: [0.0; 2],
            master_level: [0.0; 2],
            recording: None,
            sample_rate: 48_000.0,
            deck_sr: SR,
            dsp: [DeckDsp::default(), DeckDsp::default()],
            scratch: Vec::new(),
            record_cap: 6 * 60 * 48_000 * 2,
        }
    }

    pub fn load(&mut self, deck: usize, track: Arc<TrackData>) {
        self.decks[deck] = Some(track);
        self.p[deck] = DeckParams::default();
        self.dsp[deck] = DeckDsp::default();
    }

    pub fn eject(&mut self, deck: usize) {
        self.decks[deck] = None;
        self.p[deck] = DeckParams::default();
        self.dsp[deck] = DeckDsp::default();
    }

    pub fn is_recording(&self) -> bool {
        self.recording.is_some()
    }

    pub fn start_recording(&mut self) {
        if self.recording.is_none() {
            self.recording = Some(Vec::with_capacity(48_000 * 2 * 30));
        }
    }

    pub fn stop_recording(&mut self) -> Option<Vec<f32>> {
        self.recording.take()
    }

    pub fn render(&mut self, frames: usize, out_sr: f32) {
        self.sample_rate = out_sr;
        self.scratch.clear();
        self.scratch.resize(frames * 2, 0.0);
        self.levels = [0.0; 2];
        self.master_level = [0.0; 2];
        self.prepare_block(out_sr);
        let mut peak_l = 0.0f32;
        let mut peak_r = 0.0f32;
        for f in 0..frames {
            let (l, r) = self.next_frame(out_sr);
            peak_l = peak_l.max(l.abs());
            peak_r = peak_r.max(r.abs());
            self.scratch[f * 2] = l;
            self.scratch[f * 2 + 1] = r;
            if let Some(rec) = &mut self.recording {
                if rec.len() < self.record_cap {
                    rec.push(l);
                    rec.push(r);
                }
            }
        }
        self.master_level = [peak_l, peak_r];
    }

    pub fn scratch(&self) -> &[f32] {
        &self.scratch
    }

    fn prepare_block(&mut self, sr: f32) {
        // loudness-safe smoothing constants aim for ~6 ms response
        for d in 0..2 {
            let p = &self.p[d];
            let dsp = &mut self.dsp[d];
            for i in 0..3 {
                let target = if p.kill[i] { -42.0 } else { p.eq_db[i] };
                dsp.sm_eq[i] += (target - dsp.sm_eq[i]) * 0.35;
            }
            dsp.low.set_coeffs(Coeffs::low_shelf(sr, 130.0, dsp.sm_eq[0]));
            dsp.mid.set_coeffs(Coeffs::peaking(sr, 1_100.0, 0.75, dsp.sm_eq[1]));
            dsp.high.set_coeffs(Coeffs::high_shelf(sr, 6_200.0, dsp.sm_eq[2]));

            dsp.sm_filter += (p.filter - dsp.sm_filter) * 0.25;
            let sign = dsp.sm_filter.signum();
            if sign != dsp.last_filter_sign {
                dsp.filter.reset();
                dsp.last_filter_sign = sign;
            }
            let v = dsp.sm_filter;
            let coeffs = if v < -0.015 {
                let t = -v;
                Coeffs::lowpass(sr, 18_000.0 * (170.0f32 / 18_000.0).powf(t), 0.75 + 0.9 * t)
            } else if v > 0.015 {
                Coeffs::highpass(sr, 28.0 * (7_500.0f32 / 28.0).powf(v), 0.75 + 0.9 * v)
            } else {
                Coeffs::identity()
            };
            dsp.filter.set_coeffs(coeffs);

            let xf = self.crossfader.clamp(0.0, 1.0);
            let xf_target = if d == 0 {
                (xf * PI * 0.5).cos()
            } else {
                (xf * PI * 0.5).sin()
            };
            dsp.sm_xfade += (xf_target - dsp.sm_xfade) * 0.4;
            dsp.sm_gain += (p.gain * p.trim - dsp.sm_gain) * 0.4;
            let any_solo = p.stem_solo.iter().any(|&s| s);
            for s in 0..STEM_COUNT {
                let audible = if any_solo {
                    p.stem_solo[s]
                } else {
                    !p.stem_mute[s]
                };
                let target = if audible { p.stem_gain[s] } else { 0.0 };
                dsp.sm_stem[s] += (target - dsp.sm_stem[s]) * 0.30;
            }
        }
    }

    fn next_frame(&mut self, sr: f32) -> (f32, f32) {
        let mut mix = 0.0f32;
        for d in 0..2 {
            let (sample, ended) = {
                let p = &mut self.p[d];
                let Some(track) = &self.decks[d] else {
                    continue;
                };
                let total = track.frames();
                if !p.playing || total < 2 {
                    self.levels[d] = 0.0;
                    continue;
                }
                let pos = p.position;
                if pos < 0.0 || pos >= total as f64 {
                    p.playing = false;
                    p.position = 0.0;
                    self.levels[d] = 0.0;
                    continue;
                }
                let i = pos.floor() as usize;
                let frac = (pos - i as f64) as f32;
                let i1 = (i + 1).min(total - 1);
                let mut stems = [0.0f32; STEM_COUNT];
                for s in 0..STEM_COUNT {
                    let a = track.stems[s][i];
                    let b = track.stems[s][i1];
                    stems[s] = a + (b - a) * frac;
                }
                (stems, false)
            };
            let _ = ended;
            let dsp = &mut self.dsp[d];
            let mut s = 0.0f32;
            for k in 0..STEM_COUNT {
                s += sample[k] * dsp.sm_stem[k];
            }
            s = dsp.low.process(s);
            s = dsp.mid.process(s);
            s = dsp.high.process(s);
            s = dsp.filter.process(s);
            s *= dsp.sm_gain * dsp.sm_xfade;
            self.levels[d] = self.levels[d].max(s.abs());
            mix += s;

            // advance transport
            let p = &mut self.p[d];
            let total = self.decks[d].as_ref().map(|t| t.frames()).unwrap_or(0);
            if total > 0 {
                p.position += p.rate * SR as f64 / sr as f64;
                if p.loop_active && p.loop_out > p.loop_in + 32.0 {
                    let len = p.loop_out - p.loop_in;
                    if p.position >= p.loop_out {
                        p.position = p.loop_in + (p.position - p.loop_in) % len;
                    }
                }
                if p.position >= total as f64 {
                    p.position = 0.0;
                    p.playing = false;
                }
            }
        }
        let mut master = mix * self.master;
        // gentle saturation keeps the sum clean at high stem density
        master = master.tanh();
        (master, master)
    }

    /// Advances clocks when no audio device is available, so the interface
    /// still behaves like a player (used by the screenshot check too).
    pub fn advance_visual(&mut self, dt: f64) {
        for d in 0..2 {
            let Some(track) = &self.decks[d] else { continue };
            let total = track.frames();
            let p = &mut self.p[d];
            if p.playing && total > 2 {
                p.position += p.rate * SR as f64 * dt;
                if p.loop_active && p.loop_out > p.loop_in + 32.0 {
                    let len = p.loop_out - p.loop_in;
                    if p.position >= p.loop_out {
                        p.position = p.loop_in + (p.position - p.loop_in) % len;
                    }
                }
                if p.position >= total as f64 {
                    p.position = 0.0;
                    p.playing = false;
                }
            }
            let i = (p.position as usize).min(total.saturating_sub(1));
            let solo = p.stem_solo.iter().any(|&s| s);
            let mut sum = 0.0f32;
            let mut count = 0usize;
            for k in 0..STEM_COUNT {
                let on = if solo { p.stem_solo[k] } else { !p.stem_mute[k] };
                let g = if on { p.stem_gain[k] } else { 0.0 };
                if g <= 0.0001 {
                    continue;
                }
                let stem = &track.stems[k];
                let end = (i + 1024).min(stem.len());
                let mut j = i;
                while j < end {
                    let s = stem[j] * g;
                    sum += s * s;
                    count += 1;
                    j += 2;
                }
            }
            let rms = if count > 0 {
                (sum / count as f32).sqrt()
            } else {
                0.0
            };
            let target = (rms * 2.4 * p.gain).clamp(0.0, 1.0);
            let cur = self.levels[d];
            self.levels[d] = if target > cur {
                cur + (target - cur) * 0.45
            } else {
                cur + (target - cur) * 0.18
            };
        }
        let xf = self.crossfader.clamp(0.0, 1.0);
        let a = (xf * PI * 0.5).cos();
        let b = (xf * PI * 0.5).sin();
        let l = (self.levels[0] * a + self.levels[1] * b) * self.master * 1.35;
        self.master_level = [l.min(1.0), l.min(1.0)];
    }
}

fn write_output<T>(engine: &Arc<Mutex<Engine>>, data: &mut [T], channels: usize, sr: f32)
where
    T: SizedSample + FromSample<f32>,
{
    let frames = data.len() / channels;
    let mut e = match engine.lock() {
        Ok(e) => e,
        Err(p) => p.into_inner(),
    };
    e.render(frames, sr);
    let block = e.scratch();
    for (i, frame) in data.chunks_mut(channels).enumerate() {
        let l = block.get(i * 2).copied().unwrap_or(0.0);
        let r = block.get(i * 2 + 1).copied().unwrap_or(l);
        for (c, sample) in frame.iter_mut().enumerate() {
            let v = match c {
                0 => l,
                1 => r,
                _ => (l + r) * 0.5,
            };
            *sample = T::from_sample(v.clamp(-1.0, 1.0));
        }
    }
}

pub struct AudioOutput {
    pub stream: cpal::Stream,
    pub device_name: String,
    pub sample_rate: u32,
}

pub fn start_audio(engine: Arc<Mutex<Engine>>) -> Result<AudioOutput, String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "No audio output device found".to_string())?;
    let device_name = device.name().unwrap_or_else(|_| "Default output".to_string());
    let supported = device
        .default_output_config()
        .map_err(|e| format!("Output device unavailable: {e}"))?;
    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let sr = config.sample_rate.0 as f32;
    let channels = config.channels as usize;
    let err_fn = |err| eprintln!("audio stream error: {err}");

    let stream = match sample_format {
        SampleFormat::F32 => {
            let e = engine.clone();
            device
                .build_output_stream(&config, move |data: &mut [f32], _| write_output(&e, data, channels, sr), err_fn, None)
                .map_err(|e| format!("Cannot open audio stream: {e}"))?
        }
        SampleFormat::I16 => {
            let e = engine.clone();
            device
                .build_output_stream(&config, move |data: &mut [i16], _| write_output(&e, data, channels, sr), err_fn, None)
                .map_err(|e| format!("Cannot open audio stream: {e}"))?
        }
        SampleFormat::U16 => {
            let e = engine.clone();
            device
                .build_output_stream(&config, move |data: &mut [u16], _| write_output(&e, data, channels, sr), err_fn, None)
                .map_err(|e| format!("Cannot open audio stream: {e}"))?
        }
        SampleFormat::I32 => {
            let e = engine.clone();
            device
                .build_output_stream(&config, move |data: &mut [i32], _| write_output(&e, data, channels, sr), err_fn, None)
                .map_err(|e| format!("Cannot open audio stream: {e}"))?
        }
        SampleFormat::F64 => {
            let e = engine.clone();
            device
                .build_output_stream(&config, move |data: &mut [f64], _| write_output(&e, data, channels, sr), err_fn, None)
                .map_err(|e| format!("Cannot open audio stream: {e}"))?
        }
        other => return Err(format!("Unsupported sample format: {other:?}")),
    };
    stream
        .play()
        .map_err(|e| format!("Cannot start audio stream: {e}"))?;
    if let Ok(mut e) = engine.lock() {
        e.sample_rate = sr;
    }
    Ok(AudioOutput {
        stream,
        device_name,
        sample_rate: config.sample_rate.0,
    })
}

/// Writes 16-bit stereo PCM.
pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create folder: {e}"))?;
    }
    let file = File::create(path).map_err(|e| format!("Cannot create recording: {e}"))?;
    let mut w = BufWriter::new(file);
    let channels: u16 = 2;
    let bits: u16 = 16;
    let byte_rate = sample_rate * channels as u32 * (bits as u32 / 8);
    let data_len = (samples.len() * 2) as u32;
    w.write_all(b"RIFF").map_err(|e| e.to_string())?;
    w.write_all(&(36 + data_len).to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(b"WAVEfmt ").map_err(|e| e.to_string())?;
    w.write_all(&16u32.to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(&1u16.to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(&channels.to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(&sample_rate.to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(&byte_rate.to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(&(channels * bits / 8).to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(&bits.to_le_bytes()).map_err(|e| e.to_string())?;
    w.write_all(b"data").map_err(|e| e.to_string())?;
    w.write_all(&data_len.to_le_bytes()).map_err(|e| e.to_string())?;
    for &s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32_767.0) as i16;
        w.write_all(&v.to_le_bytes()).map_err(|e| e.to_string())?;
    }
    w.flush().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::synth_demo;

    #[test]
    fn renders_demo_stems_without_panicking() {
        let mut e = Engine::new();
        e.load(0, std::sync::Arc::new(synth_demo(0)));
        e.load(1, std::sync::Arc::new(synth_demo(1)));
        e.p[0].position = 5.0 * SR as f64;
        e.p[0].playing = true;
        e.p[1].playing = true;
        e.start_recording();
        for _ in 0..8 {
            e.render(512, 48_000.0);
        }
        let peak = e.scratch().iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.01, "mix is silent");
        assert!(e.scratch().iter().all(|v| v.is_finite()));
        let frames = e.stop_recording().expect("recording buffer");
        assert!(frames.len() >= 512 * 2 * 7);
        assert!(e.master_level[0] > 0.0);
    }

    #[test]
    fn muting_every_stem_quiets_the_deck() {
        let mut e = Engine::new();
        e.load(0, std::sync::Arc::new(synth_demo(0)));
        e.p[0].playing = true;
        e.crossfader = 0.0;
        for _ in 0..4 {
            e.render(512, 44_100.0);
        }
        let loud = e.scratch().iter().fold(0.0f32, |m, v| m.max(v.abs()));
        for s in 0..STEM_COUNT {
            e.p[0].stem_mute[s] = true;
        }
        for _ in 0..12 {
            e.render(512, 44_100.0);
        }
        let quiet = e.scratch().iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(loud > 0.01);
        assert!(quiet < loud * 0.05, "mute left {quiet} of {loud}");
    }
}
