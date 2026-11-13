//! STEMFORGE - a two-deck live stem mixer.
//!
//! The interface is painted with egui's painter so the layout stays exact at
//! any window size. Audio runs on a cpal callback; the UI only copies control
//! values in and out of the shared engine.

mod audio;
mod deck_ui;
mod library;
mod mixer;
mod track;
mod widgets;

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Rect, Stroke, StrokeKind, Ui,
};

use audio::{start_audio, write_wav, AudioOutput, Engine};
use deck_ui::DeckUi;
use track::{analyze_file, demo_catalog, synth_demo, TrackData, SR};
use widgets::*;

// ---------------------------------------------------------------------------
// background file analysis
// ---------------------------------------------------------------------------

enum LoadMsg {
    Progress(f32, String),
    Ready(Box<TrackData>),
    Err(String),
}

struct Loader {
    deck: usize,
    label: String,
    progress: f32,
    status: String,
    rx: Receiver<LoadMsg>,
}

// ---------------------------------------------------------------------------
// screenshot mode
// ---------------------------------------------------------------------------

struct Shot {
    path: PathBuf,
    requested: bool,
    saved: bool,
}

// ---------------------------------------------------------------------------
// application state
// ---------------------------------------------------------------------------

pub struct StemForgeApp {
    engine: Arc<Mutex<Engine>>,
    audio: Option<AudioOutput>,
    audio_error: Option<String>,
    demo_cache: Vec<Option<Arc<TrackData>>>,
    deck: [DeckUi; 2],
    search: String,
    loader: Option<Loader>,
    record_start: Option<Instant>,
    toast: Option<(String, Instant)>,
    shot: Option<Shot>,
    frames: u64,
    last_update: Instant,
    started: Instant,
}

impl StemForgeApp {
    fn new(cc: &eframe::CreationContext<'_>, shot: Option<PathBuf>) -> Self {
        install_style(&cc.egui_ctx);

        let mut engine = Engine::new();
        let a = Arc::new(synth_demo(0));
        let b = Arc::new(synth_demo(1));
        engine.load(0, a.clone());
        engine.load(1, b.clone());
        let shot_mode = shot.is_some();
        engine.p[0].position = 11.0 * SR as f64;
        engine.p[0].playing = shot_mode;
        engine.p[1].position = 22.0 * SR as f64;
        engine.p[1].playing = shot_mode;
        engine.p[1].rate = 124.0 / 122.0;
        engine.crossfader = 0.5;
        engine.master = 0.72;

        if shot_mode && std::env::var_os("STEMFORGE_DEMO_STATE").is_some() {
            engine.p[0].position = 12.5 * SR as f64;
            engine.p[0].stem_mute[0] = true;
            engine.p[0].stem_gain[1] = 0.55;
            engine.p[0].kill[2] = true;
            engine.p[0].filter = -0.45;
            engine.p[0].eq_db[1] = -3.0;
            engine.p[0].loop_active = true;
            engine.p[0].loop_in = 12.0 * SR as f64;
            engine.p[0].loop_out = 12.0 * SR as f64 + 4.0 * (60.0 / 124.0) * SR as f64;
            engine.p[1].stem_solo[0] = true;
            engine.p[1].filter = 0.35;
            engine.p[1].eq_db[0] = 2.0;
            engine.crossfader = 0.38;
        }

        let mut decks = [DeckUi::new(), DeckUi::new()];
        if shot_mode && std::env::var_os("STEMFORGE_DEMO_STATE").is_some() {
            decks[0].hotcues = [Some(9.5), Some(15.0), None, None];
            decks[0].loop_beats = 4.0;
        }

        let engine = Arc::new(Mutex::new(engine));
        let (audio, audio_error) = if shot.is_some() {
            (None, None)
        } else {
            match start_audio(engine.clone()) {
                Ok(out) => (Some(out), None),
                Err(e) => (None, Some(e)),
            }
        };

        let mut demo_cache: Vec<Option<Arc<TrackData>>> = vec![None, None, None, None];
        demo_cache[0] = Some(a);
        demo_cache[1] = Some(b);

        Self {
            engine,
            audio,
            audio_error,
            demo_cache,
            deck: decks,
            search: String::new(),
            loader: None,
            record_start: None,
            toast: None,
            shot: shot.map(|path| Shot {
                path,
                requested: false,
                saved: false,
            }),
            frames: 0,
            last_update: Instant::now(),
            started: Instant::now(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Engine> {
        match self.engine.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn notify(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), Instant::now()));
    }

    fn load_track(&mut self, deck: usize, track: Arc<TrackData>) {
        self.lock().load(deck, track.clone());
        self.deck[deck] = DeckUi::new();
        self.notify(format!(
            "{} loaded on deck {}",
            track.name,
            if deck == 0 { "A" } else { "B" }
        ));
    }

    fn load_demo(&mut self, deck: usize, index: usize) {
        let track = if let Some(t) = self.demo_cache.get(index).and_then(|t| t.clone()) {
            t
        } else {
            let t = Arc::new(synth_demo(index));
            if index < self.demo_cache.len() {
                self.demo_cache[index] = Some(t.clone());
            }
            t
        };
        self.load_track(deck, track);
    }

    fn pick_file(&mut self, deck: usize) {
        if self.loader.is_some() {
            self.notify("Wait for the current analysis to finish");
            return;
        }
        let picked = rfd::FileDialog::new()
            .set_title("Load an audio track")
            .add_filter("Audio", &["mp3", "wav", "flac", "m4a", "aac", "ogg"])
            .pick_file();
        let Some(path) = picked else { return };

        let label = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Audio file")
            .to_string();
        let (tx, rx) = channel();
        let thread_path = path.clone();
        std::thread::spawn(move || {
            let progress_tx = tx.clone();
            let result = analyze_file(&thread_path, &mut |p, s| {
                let _ = progress_tx.send(LoadMsg::Progress(p, s.to_string()));
            });
            match result {
                Ok(t) => {
                    let _ = tx.send(LoadMsg::Ready(Box::new(t)));
                }
                Err(e) => {
                    let _ = tx.send(LoadMsg::Err(e));
                }
            }
        });
        self.loader = Some(Loader {
            deck,
            label,
            progress: 0.0,
            status: "Reading file".to_string(),
            rx,
        });
    }

    fn poll_loader(&mut self) {
        let mut outcome: Option<Result<Box<TrackData>, String>> = None;
        if let Some(loader) = &mut self.loader {
            loop {
                match loader.rx.try_recv() {
                    Ok(LoadMsg::Progress(p, s)) => {
                        loader.progress = p.clamp(0.0, 1.0);
                        loader.status = s;
                    }
                    Ok(LoadMsg::Ready(t)) => {
                        outcome = Some(Ok(t));
                        break;
                    }
                    Ok(LoadMsg::Err(e)) => {
                        outcome = Some(Err(e));
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        outcome = Some(Err("Analysis stopped unexpectedly".to_string()));
                        break;
                    }
                }
            }
        }
        if let Some(result) = outcome {
            let Some(loader) = self.loader.take() else {
                return;
            };
            match result {
                Ok(t) => self.load_track(loader.deck, Arc::new(*t)),
                Err(e) => self.notify(format!("Could not load audio: {e}")),
            }
        }
    }

    fn toggle_recording(&mut self) {
        let (is_rec, sr) = {
            let e = self.lock();
            (e.is_recording(), e.sample_rate)
        };
        if is_rec {
            let buf = {
                let mut e = self.lock();
                e.stop_recording()
            };
            self.record_start = None;
            let Some(samples) = buf else { return };
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let dir = std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("recordings");
            let path = dir.join(format!("stemforge_session_{stamp}.wav"));
            match write_wav(&path, &samples, sr.max(8_000.0) as u32) {
                Ok(()) => {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("recording.wav")
                        .to_string();
                    self.notify(format!("Session saved as {name}"));
                }
                Err(e) => self.notify(format!("Could not save session: {e}")),
            }
        } else {
            self.lock().start_recording();
            self.record_start = Some(Instant::now());
            self.notify("Recording master output");
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            let mut e = self.lock();
            e.p[0].playing = !e.p[0].playing;
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        if self.shot.is_none() {
            return;
        }
        self.frames += 1;
        ctx.request_repaint_after(Duration::from_millis(16));
        let requested = self.shot.as_ref().map(|s| s.requested).unwrap_or(true);
        if self.frames >= 125 && !requested {
            if let Some(shot) = &mut self.shot {
                shot.requested = true;
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        let mut captured: Option<([usize; 2], Vec<u8>)> = None;
        ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    captured = Some((image.size, image.as_raw().to_vec()));
                }
            }
        });
        if let Some((size, raw)) = captured {
            let path = self.shot.as_ref().map(|s| s.path.clone());
            if let Some(path) = path {
                if size[0] > 1 && size[1] > 1 {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let saved = image::save_buffer(
                        &path,
                        &raw,
                        size[0] as u32,
                        size[1] as u32,
                        image::ColorType::Rgba8,
                    );
                    match saved {
                        Ok(()) => eprintln!("screenshot saved: {}", path.display()),
                        Err(e) => eprintln!("screenshot failed: {e}"),
                    }
                }
            }
            if let Some(shot) = &mut self.shot {
                shot.saved = true;
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn master_bpm(&self) -> f32 {
        let e = self.lock();
        if let Some(t) = &e.decks[0] {
            t.bpm * e.p[0].rate as f32
        } else if let Some(t) = &e.decks[1] {
            t.bpm * e.p[1].rate as f32
        } else {
            0.0
        }
    }

    // -----------------------------------------------------------------------
    // chrome
    // -----------------------------------------------------------------------

    fn draw_header(&mut self, ui: &mut Ui, rect: Rect) {
        let p = ui.painter().with_clip_rect(rect);
        p.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(11, 14, 20));
        p.line_segment(
            [pos2(rect.left(), rect.bottom()), pos2(rect.right(), rect.bottom())],
            Stroke::new(1.0, LINE),
        );

        // logo: four stem bars inside a rounded plate
        let logo = Rect::from_min_size(rect.min + vec2(16.0, 13.0), vec2(32.0, 32.0));
        p.rect_filled(logo, CornerRadius::same(9), PANEL3);
        p.rect_stroke(
            logo,
            CornerRadius::same(9),
            Stroke::new(1.0, alpha(DECK_A, 90)),
            StrokeKind::Inside,
        );
        for i in 0..4 {
            let h = [15.0, 21.0, 11.0, 18.0][i];
            let x = logo.left() + 7.5 + i as f32 * 5.0;
            p.rect_filled(
                Rect::from_min_max(
                    pos2(x, logo.bottom() - 7.0 - h),
                    pos2(x + 2.6, logo.bottom() - 7.0),
                ),
                CornerRadius::same(1),
                STEM_COLORS[i],
            );
        }
        txt(
            &p,
            pos2(rect.left() + 58.0, rect.top() + 12.0),
            "STEMFORGE",
            16.0,
            TEXT,
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(rect.left() + 59.0, rect.top() + 33.0),
            "LIVE STEM MIXER",
            8.0,
            DIM,
            Align2::LEFT_TOP,
        );

        // master tempo block
        let mx = rect.left() + 216.0;
        p.line_segment(
            [pos2(mx - 22.0, rect.top() + 15.0), pos2(mx - 22.0, rect.bottom() - 15.0)],
            Stroke::new(1.0, LINE),
        );
        label(&p, pos2(mx, rect.top() + 12.0), "MASTER TEMPO");
        let bpm = self.master_bpm();
        if bpm > 0.0 {
            mono(
                &p,
                pos2(mx, rect.top() + 24.0),
                format!("{bpm:.1}"),
                20.0,
                TEXT,
                Align2::LEFT_TOP,
            );
            txt(
                &p,
                pos2(mx + 58.0, rect.top() + 31.0),
                "BPM",
                8.0,
                MUTED,
                Align2::LEFT_TOP,
            );
        } else {
            mono(
                &p,
                pos2(mx, rect.top() + 26.0),
                "--.-",
                20.0,
                DIM,
                Align2::LEFT_TOP,
            );
        }

        // active beat dots
        let (beat_phase, deck_key) = {
            let e = self.lock();
            let phase = e
                .decks[0]
                .as_ref()
                .map(|t| {
                    let beat = 60.0 / (t.bpm as f64).max(1.0);
                    ((e.p[0].position / SR as f64) / beat).fract()
                })
                .unwrap_or(0.0);
            let key = e
                .decks[0]
                .as_ref()
                .map(|t| t.key.clone())
                .or_else(|| e.decks[1].as_ref().map(|t| t.key.clone()))
                .unwrap_or_else(|| "--".to_string());
            (phase, key)
        };
        let bx = mx + 100.0;
        label(&p, pos2(bx, rect.top() + 12.0), "KEY");
        txt(
            &p,
            pos2(bx, rect.top() + 25.0),
            deck_key,
            13.0,
            STEM_COLORS[2],
            Align2::LEFT_TOP,
        );
        for i in 0..4 {
            let active = (beat_phase * 4.0) as usize == i;
            p.circle_filled(
                pos2(bx + 8.0 + i as f32 * 11.0, rect.top() + 46.0),
                2.5,
                if active { DECK_A } else { alpha(DIM, 90) },
            );
        }

        // right hand status
        let right = rect.right() - 16.0;
        let recording = self.lock().is_recording();
        let rec_rect = Rect::from_min_size(pos2(right - 64.0, rect.top() + 17.0), vec2(64.0, 22.0));
        p.rect_filled(
            rec_rect,
            CornerRadius::same(7),
            if recording { alpha(RED, 40) } else { PANEL2 },
        );
        p.rect_stroke(
            rec_rect,
            CornerRadius::same(7),
            Stroke::new(1.0, if recording { alpha(RED, 160) } else { LINE }),
            StrokeKind::Inside,
        );
        let blink = if recording {
            (ui.input(|i| i.time) * 2.4).sin() as f32 * 0.35 + 0.65
        } else {
            0.0
        };
        p.circle_filled(
            pos2(rec_rect.left() + 11.0, rec_rect.center().y),
            3.2,
            if recording { alpha(RED, (blink * 255.0) as u8) } else { DIM },
        );
        txt(
            &p,
            pos2(rec_rect.left() + 20.0, rec_rect.center().y),
            if recording { "RECORDING" } else { "RECORD" },
            8.0,
            if recording { RED } else { MUTED },
            Align2::LEFT_CENTER,
        );

        let status_color = if self.audio.is_some() || self.shot.is_some() {
            GREEN
        } else {
            RED
        };
        let status_text = if self.audio.is_some() {
            "AUDIO ONLINE"
        } else if self.shot.is_some() {
            "RENDER READY"
        } else {
            "NO OUTPUT"
        };
        let sr = self
            .audio
            .as_ref()
            .map(|a| a.sample_rate)
            .unwrap_or(48_000);
        let status_rect =
            Rect::from_min_size(pos2(rec_rect.left() - 140.0, rect.top() + 17.0), vec2(130.0, 22.0));
        small_dot(&p, pos2(status_rect.left() + 7.0, status_rect.center().y), 3.0, status_color);
        txt(
            &p,
            pos2(status_rect.left() + 15.0, status_rect.center().y),
            status_text,
            8.2,
            MUTED,
            Align2::LEFT_CENTER,
        );
        mono(
            &p,
            pos2(status_rect.right(), status_rect.center().y),
            format!("{:.1} kHz", sr as f32 / 1000.0),
            8.2,
            DIM,
            Align2::RIGHT_CENTER,
        );

        let engine_rect =
            Rect::from_min_size(pos2(status_rect.left() - 92.0, rect.top() + 17.0), vec2(82.0, 22.0));
        p.rect_filled(engine_rect, CornerRadius::same(7), PANEL2);
        p.rect_stroke(
            engine_rect,
            CornerRadius::same(7),
            Stroke::new(1.0, LINE),
            StrokeKind::Inside,
        );
        txt(
            &p,
            engine_rect.center(),
            "4 STEMS",
            8.0,
            alpha(STEM_COLORS[3], 220),
            Align2::CENTER_CENTER,
        );
    }

    fn draw_status(&mut self, ui: &mut Ui, rect: Rect) {
        let p = ui.painter().with_clip_rect(rect);
        p.rect_filled(rect, CornerRadius::ZERO, Color32::from_rgb(10, 12, 18));
        p.line_segment(
            [pos2(rect.left(), rect.top()), pos2(rect.right(), rect.top())],
            Stroke::new(1.0, LINE_SOFT),
        );
        let (a_pos, a_len, a_name, b_pos, b_len, b_name) = {
            let e = self.lock();
            let info = |d: usize| {
                if let Some(t) = &e.decks[d] {
                    (
                        e.p[d].position / SR as f64,
                        t.duration as f64,
                        t.name.clone(),
                    )
                } else {
                    (0.0, 0.0, "empty".to_string())
                }
            };
            let (ap, al, an) = info(0);
            let (bp, bl, bn) = info(1);
            (ap, al, an, bp, bl, bn)
        };
        mono(
            &p,
            pos2(rect.left() + 16.0, rect.center().y),
            format!("A  {} / {}  {}", fmt_time(a_pos), fmt_time(a_len), a_name),
            8.5,
            MUTED,
            Align2::LEFT_CENTER,
        );
        mono(
            &p,
            pos2(rect.right() - 16.0, rect.center().y),
            format!("B  {} / {}  {}", fmt_time(b_pos), fmt_time(b_len), b_name),
            8.5,
            MUTED,
            Align2::RIGHT_CENTER,
        );
        let hint = if self.audio_error.is_some() {
            "No audio device - interface running in silent preview"
        } else {
            "SPACE  play/pause A     drag a knob vertically     double-click a knob to reset"
        };
        txt(
            &p,
            pos2(rect.center().x, rect.center().y),
            hint,
            8.5,
            if self.audio_error.is_some() { RED } else { DIM },
            Align2::CENTER_CENTER,
        );
    }

    fn draw_toast(&mut self, ui: &mut Ui, area: Rect) {
        let Some((text, at)) = &self.toast else { return };
        let age = at.elapsed().as_secs_f32();
        if age > 5.0 {
            self.toast = None;
            return;
        }
        let fade = if age > 4.3 {
            ((5.0 - age) / 0.7).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let p = ui.painter();
        let galley = p.layout_no_wrap(
            text.clone(),
            egui::FontId::proportional(11.0),
            alpha(TEXT, (fade * 255.0) as u8),
        );
        let w = galley.size().x + 32.0;
        let r = Rect::from_center_size(
            pos2(area.center().x, area.bottom() - 26.0),
            vec2(w.max(180.0), 30.0),
        );
        p.rect_filled(r, CornerRadius::same(9), alpha(PANEL3, (fade * 245.0) as u8));
        p.rect_stroke(
            r,
            CornerRadius::same(9),
            Stroke::new(1.0, alpha(LINE, (fade * 255.0) as u8)),
            StrokeKind::Inside,
        );
        p.galley(r.center() - galley.size() * 0.5, galley, TEXT);
    }

    fn draw_loader(&mut self, ui: &mut Ui, area: Rect) {
        let Some(loader) = &self.loader else { return };
        let p = ui.painter();
        let r = Rect::from_center_size(
            pos2(area.center().x, area.bottom() - 60.0),
            vec2(380.0, 52.0),
        );
        p.rect_filled(r, CornerRadius::same(10), PANEL3);
        p.rect_stroke(
            r,
            CornerRadius::same(10),
            Stroke::new(1.0, alpha(DECK_B, 110)),
            StrokeKind::Inside,
        );
        txt(
            &p,
            pos2(r.left() + 14.0, r.top() + 10.0),
            format!("ANALYSING  {}", fit(&loader.label, 34)),
            10.0,
            TEXT,
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(r.right() - 14.0, r.top() + 10.0),
            format!("{}%", (loader.progress * 100.0).round() as i32),
            10.0,
            DECK_B,
            Align2::RIGHT_TOP,
        );
        let bar = Rect::from_min_size(
            pos2(r.left() + 14.0, r.bottom() - 20.0),
            vec2(r.width() - 28.0, 4.0),
        );
        p.rect_filled(bar, CornerRadius::same(2), LINE_SOFT);
        p.rect_filled(
            Rect::from_min_size(bar.min, vec2(bar.width() * loader.progress, 4.0)),
            CornerRadius::same(2),
            DECK_B,
        );
        txt(
            &p,
            pos2(r.left() + 14.0, r.bottom() - 13.0),
            format!("{} - deck {}", loader.status, if loader.deck == 0 { "A" } else { "B" }),
            8.0,
            DIM,
            Align2::LEFT_TOP,
        );
    }
}

impl eframe::App for StemForgeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let now = Instant::now();
        let dt = (now - self.last_update).as_secs_f64().clamp(0.0, 0.1);
        self.last_update = now;

        self.poll_loader();
        self.handle_shortcuts(&ctx);
        self.handle_screenshot(&ctx);

        if self.audio.is_none() {
            self.lock().advance_visual(dt);
        }

        let recording = self.lock().is_recording();
        if !recording {
            self.record_start = None;
        }

        let full = ui.max_rect();
        let header = Rect::from_min_size(full.min, vec2(full.width(), 58.0));
        let status = Rect::from_min_max(
            pos2(full.left(), full.bottom() - 24.0),
            pos2(full.right(), full.bottom()),
        );
        self.draw_header(ui, header);
        self.draw_status(ui, status);

        let body = Rect::from_min_max(
            pos2(full.left(), header.bottom()),
            pos2(full.right(), status.top()),
        );
        draw_background(ui.painter(), body);
        let content = body.shrink(12.0);
        let gap = 10.0;
        let lib_w = 202.0;
        let mix_w = 232.0;
        let deck_w = ((content.width() - lib_w - mix_w - gap * 3.0) / 2.0).max(310.0);
        let y = content.top();
        let h = content.height();
        let lib = Rect::from_min_size(pos2(content.left(), y), vec2(lib_w, h));
        let a = Rect::from_min_size(pos2(lib.right() + gap, y), vec2(deck_w, h));
        let mix = Rect::from_min_size(pos2(a.right() + gap, y), vec2(mix_w, h));
        let b = Rect::from_min_size(pos2(mix.right() + gap, y), vec2(deck_w, h));

        self.draw_library(ui, lib);
        self.draw_deck(ui, a, 0);
        self.draw_mixer(ui, mix);
        self.draw_deck(ui, b, 1);

        self.draw_loader(ui, body);
        self.draw_toast(ui, body);

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

fn draw_background(p: &egui::Painter, rect: Rect) {
    p.rect_filled(rect, CornerRadius::ZERO, BG);
    // soft stage lights behind each deck
    for (cx, color) in [
        (rect.left() + rect.width() * 0.34, DECK_A),
        (rect.left() + rect.width() * 0.76, DECK_B),
    ] {
        for i in 0..5 {
            let r = 300.0 - i as f32 * 40.0;
            p.circle_filled(
                pos2(cx, rect.top() + rect.height() * 0.22),
                r,
                alpha(color, 3),
            );
        }
    }
}

fn install_style(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = PANEL;
        style.visuals.extreme_bg_color = PANEL2;
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = alpha(DECK_A, 80);
        style.visuals.selection.stroke = Stroke::new(1.0, DECK_A);
        style.visuals.widgets.inactive.bg_fill = PANEL2;
        style.visuals.widgets.inactive.weak_bg_fill = PANEL2;
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, MUTED);
        style.visuals.widgets.hovered.bg_fill = PANEL3;
        style.visuals.widgets.hovered.weak_bg_fill = PANEL3;
        style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
        style.visuals.widgets.active.bg_fill = PANEL3;
        style.visuals.widgets.active.weak_bg_fill = PANEL3;
        style.spacing.item_spacing = vec2(8.0, 6.0);
        style.spacing.button_padding = vec2(10.0, 5.0);
    });
}

fn main() -> eframe::Result {
    let mut shot_path: Option<PathBuf> = None;
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--screenshot" {
            let Some(value) = args.get(i + 1) else {
                eprintln!("--screenshot needs a file path");
                std::process::exit(2);
            };
            shot_path = Some(PathBuf::from(value));
            i += 2;
        } else if let Some(value) = args[i].strip_prefix("--screenshot=") {
            shot_path = Some(PathBuf::from(value));
            i += 1;
        } else {
            i += 1;
        }
    }
    if let Some(path) = &shot_path {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
    }

    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1180.0, 760.0])
        .with_title("STEMFORGE - Live Stem Mixer")
        .with_resizable(true);
    if shot_path.is_some() {
        viewport = viewport.with_active(false);
        #[cfg(target_os = "windows")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::AlwaysOnBottom);
        }
        #[cfg(target_os = "macos")]
        {
            viewport = viewport.with_window_level(egui::WindowLevel::Normal);
        }
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "stemforge",
        options,
        Box::new(move |cc| Ok(Box::new(StemForgeApp::new(cc, shot_path)))),
    )
}
