//! Deck panel: header, scrolling stem waveform, transport, stems, EQ and pads.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui,
};

use crate::audio::DeckParams;
use crate::widgets::*;
use crate::StemForgeApp;
use crate::track::{TrackData, SR, STEM_COUNT};

pub struct DeckUi {
    pub hotcues: [Option<f64>; 4],
    pub loop_beats: f32,
}

impl DeckUi {
    pub fn new() -> Self {
        Self {
            hotcues: [None; 4],
            loop_beats: 4.0,
        }
    }
}

impl Default for DeckUi {
    fn default() -> Self {
        Self::new()
    }
}

impl StemForgeApp {
    pub(crate) fn draw_deck(&mut self, ui: &mut Ui, rect: Rect, d: usize) {
        let accent = deck_color(d);
        let pad = 12.0;
        card(ui.painter(), rect, 13);
        let p = ui.painter().with_clip_rect(rect);
        // accent hairline at the top of the card
        p.rect_filled(
            Rect::from_min_size(
                pos2(rect.left() + 14.0, rect.top() + 1.0),
                vec2(rect.width() - 28.0, 2.0),
            ),
            CornerRadius::same(1),
            alpha(accent, 190),
        );

        let (track, p0, other_bpm) = {
            let e = self.lock();
            let other = &e.decks[1 - d];
            let other_bpm = other.as_ref().map(|t| t.bpm * e.p[1 - d].rate as f32);
            (e.decks[d].clone(), e.p[d].clone(), other_bpm)
        };

        let Some(track) = track else {
            self.draw_empty_deck(ui, rect, d, accent);
            return;
        };

        let mut params = p0.clone();
        let mut ui_state = std::mem::take(&mut self.deck[d]);
        let mut seek_sec: Option<f64> = None;
        let mut play_override: Option<bool> = None;
        let base_id = ("deck", d);

        let left = rect.left() + pad;
        let right = rect.right() - pad;
        let top = rect.top() + pad;
        let inner_w = right - left;

        // -------------------------------------------------------------------
        // header
        // -------------------------------------------------------------------
        let header_h = 64.0;
        let art = Rect::from_min_size(pos2(left, top), vec2(58.0, 58.0));
        draw_artwork(&p, art, &track, accent);
        txt(
            &p,
            pos2(art.left() + 5.0, art.bottom() - 10.0),
            if d == 0 { "A" } else { "B" },
            7.0,
            alpha(BG, 200),
            Align2::LEFT_TOP,
        );

        let text_x = art.right() + 13.0;
        let badge = Rect::from_min_size(pos2(text_x, top + 4.0), vec2(47.0, 15.0));
        p.rect_filled(badge, CornerRadius::same(4), alpha(accent, 34));
        txt(
            &p,
            badge.center(),
            if d == 0 { "DECK A" } else { "DECK B" },
            7.8,
            accent,
            Align2::CENTER_CENTER,
        );
        let source_x = badge.right() + 8.0;
        txt(
            &p,
            pos2(source_x, badge.center().y),
            fit(&track.source, 28).to_uppercase(),
            7.2,
            DIM,
            Align2::LEFT_CENTER,
        );

        let bpm_col = 74.0;
        let title_max = ((right - bpm_col - 10.0 - text_x) / 6.2).max(8.0) as usize;
        txt(
            &p,
            pos2(text_x, top + 22.0),
            fit(&track.name, title_max),
            16.5,
            TEXT,
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(text_x, top + 44.0),
            fit(&track.artist, 34),
            9.8,
            MUTED,
            Align2::LEFT_TOP,
        );

        let eff_bpm = track.bpm * params.rate as f32;
        mono(
            &p,
            pos2(right, top + 2.0),
            format!("{eff_bpm:.1}"),
            19.0,
            TEXT,
            Align2::RIGHT_TOP,
        );
        txt(
            &p,
            pos2(right - 1.0, top + 24.0),
            "BPM",
            7.0,
            DIM,
            Align2::RIGHT_TOP,
        );
        txt(
            &p,
            pos2(right, top + 33.0),
            fit(&track.key, 12),
            9.5,
            alpha(STEM_COLORS[2], 230),
            Align2::RIGHT_TOP,
        );

        let load_rect = Rect::from_min_size(pos2(right - 96.0, top + 48.0), vec2(44.0, 14.0));
        let ej_rect = Rect::from_min_size(pos2(right - 48.0, top + 48.0), vec2(48.0, 14.0));
        if text_button(ui, load_rect, "LOAD", accent, false, (base_id, "load")).clicked() {
            self.pick_file(d);
        }
        if text_button(ui, ej_rect, "EJECT", MUTED, false, (base_id, "eject")).clicked() {
            self.lock().eject(d);
            self.deck[d] = DeckUi::new();
            self.notify(format!("Deck {} ejected", if d == 0 { "A" } else { "B" }));
            return;
        }

        // -------------------------------------------------------------------
        // geometry for the rest of the deck
        // -------------------------------------------------------------------
        let wave_h = (rect.height() - 540.0).clamp(96.0, 280.0);
        let wave = Rect::from_min_size(
            pos2(left, top + header_h + 8.0),
            vec2(inner_w, wave_h),
        );
        let overview = Rect::from_min_size(
            pos2(left, wave.bottom() + 6.0),
            vec2(inner_w, 30.0),
        );
        let transport = Rect::from_min_size(
            pos2(left, overview.bottom() + 8.0),
            vec2(inner_w, 62.0),
        );
        let stems_top = transport.bottom() + 10.0;
        let eq_top = stems_top + 138.0 + 8.0;
        let pads_top = eq_top + 92.0 + 8.0;
        let loop_top = pads_top + 44.0 + 8.0;

        // -------------------------------------------------------------------
        // waveform
        // -------------------------------------------------------------------
        self.draw_waveform(
            ui,
            wave,
            &track,
            &params,
            accent,
            d,
            &mut seek_sec,
        );

        self.draw_overview(
            ui,
            overview,
            &track,
            &params,
            accent,
            &ui_state.hotcues,
            d,
            &mut seek_sec,
        );

        // -------------------------------------------------------------------
        // transport
        // -------------------------------------------------------------------
        let ty = transport.top();
        let cue_c = pos2(left + 20.0, ty + 31.0);
        let play_c = pos2(left + 64.0, ty + 31.0);
        let cue_resp = ui.interact(
            Rect::from_center_size(cue_c, vec2(38.0, 38.0)),
            ui.make_persistent_id((base_id, "cue")),
            Sense::click(),
        );
        p.circle_filled(cue_c, 17.0, PANEL2);
        p.circle_stroke(cue_c, 17.0, Stroke::new(1.0, alpha(accent, 110)));
        txt(
            &p,
            cue_c,
            "CUE",
            7.8,
            alpha(accent, 230),
            Align2::CENTER_CENTER,
        );
        if cue_resp.clicked() {
            let shift = ui.input(|i| i.modifiers.shift);
            if shift {
                params.cue = params.position;
                self.notify(format!(
                    "Cue set at {} on deck {}",
                    fmt_time(params.position / SR as f64),
                    if d == 0 { "A" } else { "B" }
                ));
            } else if params.playing {
                seek_sec = Some(params.cue / SR as f64);
                play_override = Some(false);
            } else {
                let at_cue = (params.position - params.cue).abs() < SR as f64 * 0.05;
                if at_cue {
                    play_override = Some(true);
                } else {
                    seek_sec = Some(params.cue / SR as f64);
                }
            }
        }

        let play_resp = ui.interact(
            Rect::from_center_size(play_c, vec2(48.0, 48.0)),
            ui.make_persistent_id((base_id, "play")),
            Sense::click(),
        );
        let pulse = if params.playing {
            1.0 + (ui.input(|i| i.time) * 3.0).sin() as f32 * 0.02
        } else {
            1.0
        };
        p.circle_filled(play_c, 23.0 * pulse, alpha(accent, 30));
        p.circle_filled(play_c, 21.0, if params.playing { accent } else { PANEL3 });
        p.circle_stroke(
            play_c,
            21.0,
            Stroke::new(1.0, if params.playing { alpha(TEXT, 80) } else { LINE }),
        );
        if params.playing {
            icon_pause(&p, play_c, 16.0, BG);
        } else {
            icon_play(&p, play_c, 17.0, TEXT);
        }
        if play_resp.clicked() {
            let total = track.frames() as f64;
            if !params.playing && params.position >= total - 8.0 {
                seek_sec = Some(0.0);
            }
            play_override = Some(!params.playing);
        }

        p.line_segment(
            [pos2(left + 94.0, ty + 8.0), pos2(left + 94.0, ty + 54.0)],
            Stroke::new(1.0, LINE_SOFT),
        );

        // tempo readout
        let bpm_x = left + 106.0;
        label(&p, pos2(bpm_x, ty + 4.0), "EFFECTIVE TEMPO");
        mono(
            &p,
            pos2(bpm_x, ty + 16.0),
            format!("{eff_bpm:.1}"),
            22.0,
            if params.playing { TEXT } else { MUTED },
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(bpm_x + 64.0, ty + 28.0),
            "BPM",
            7.5,
            DIM,
            Align2::LEFT_TOP,
        );
        let rate_pct = (params.rate - 1.0) * 100.0;
        mono(
            &p,
            pos2(bpm_x, ty + 44.0),
            format!("{rate_pct:+.1}%"),
            9.0,
            if rate_pct.abs() < 0.02 { DIM } else { accent },
            Align2::LEFT_TOP,
        );
        let sync_rect = Rect::from_min_size(pos2(bpm_x + 46.0, ty + 42.0), vec2(40.0, 17.0));
        if chip(ui, sync_rect, "SYNC", accent, false, (base_id, "sync")).clicked() {
            if let Some(ob) = other_bpm {
                let target = (ob / track.bpm.max(1.0)).clamp(0.6, 1.8) as f64;
                params.rate = target;
                self.notify(format!(
                    "Deck {} synced to {:.1} BPM",
                    if d == 0 { "A" } else { "B" },
                    track.bpm * target as f32
                ));
            } else {
                self.notify("Load a track on the other deck to sync");
            }
        }

        // pitch fader
        let pitch_x = left + 210.0;
        label(&p, pos2(pitch_x, ty + 4.0), "PITCH / TEMPO");
        mono(
            &p,
            pos2(right, ty + 4.0),
            format!("{rate_pct:+.1}%"),
            8.5,
            MUTED,
            Align2::RIGHT_TOP,
        );
        let pitch_rect = Rect::from_min_size(
            pos2(pitch_x, ty + 34.0),
            vec2((right - pitch_x).max(60.0), 16.0),
        );
        let mut rate = params.rate.clamp(0.92, 1.08) as f32;
        if hfader(
            ui,
            pitch_rect,
            &mut rate,
            0.92,
            1.08,
            accent,
            (base_id, "pitch"),
        )
        .changed()
        {
            params.rate = rate as f64;
        }
        txt(
            &p,
            pos2(pitch_x, ty + 52.0),
            "-8%",
            7.0,
            DIM,
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(right, ty + 52.0),
            "+8%",
            7.0,
            DIM,
            Align2::RIGHT_TOP,
        );

        // -------------------------------------------------------------------
        // stems
        // -------------------------------------------------------------------
        label(&p, pos2(left, stems_top), "STEMS");
        txt(
            &p,
            pos2(right, stems_top),
            "pre-analysed / instant mute",
            7.5,
            DIM,
            Align2::RIGHT_TOP,
        );
        let any_solo = params.stem_solo.iter().any(|&s| s);
        for s in 0..STEM_COUNT {
            let row_y = stems_top + 18.0 + s as f32 * 30.0;
            let color = STEM_COLORS[s];
            let audible = if any_solo {
                params.stem_solo[s]
            } else {
                !params.stem_mute[s]
            };
            let row = Rect::from_min_size(pos2(left, row_y), vec2(inner_w, 28.0));
            if row.contains(ui.ctx().pointer_hover_pos().unwrap_or(pos2(-99.0, -99.0))) {
                p.rect_filled(row, CornerRadius::same(5), alpha(PANEL3, 90));
            }
            small_dot(&p, pos2(left + 5.0, row.center().y), 3.2, if audible { color } else { alpha(color, 70) });
            txt(
                &p,
                pos2(left + 14.0, row.center().y),
                STEM_NAMES[s],
                10.0,
                if audible { TEXT } else { DIM },
                Align2::LEFT_CENTER,
            );
            let fader = Rect::from_min_size(
                pos2(left + 84.0, row.center().y - 8.0),
                vec2((inner_w - 180.0).max(60.0), 16.0),
            );
            hfader(
                ui,
                fader,
                &mut params.stem_gain[s],
                0.0,
                1.0,
                color,
                (base_id, "stem", s),
            );
            mono(
                &p,
                pos2(right - 92.0, row.center().y),
                format!("{:>3}%", (params.stem_gain[s] * 100.0).round() as i32),
                8.0,
                if audible { MUTED } else { DIM },
                Align2::RIGHT_CENTER,
            );
            let m = Rect::from_min_size(pos2(right - 68.0, row.center().y - 8.0), vec2(26.0, 16.0));
            let so = Rect::from_min_size(pos2(right - 38.0, row.center().y - 8.0), vec2(26.0, 16.0));
            if chip(ui, m, "M", RED, params.stem_mute[s], (base_id, "mute", s)).clicked() {
                params.stem_mute[s] = !params.stem_mute[s];
            }
            if chip(ui, so, "S", AMBER, params.stem_solo[s], (base_id, "solo", s)).clicked() {
                params.stem_solo[s] = !params.stem_solo[s];
            }
        }

        // -------------------------------------------------------------------
        // EQ + filter
        // -------------------------------------------------------------------
        label(&p, pos2(left, eq_top), "EQ / FILTER");
        txt(
            &p,
            pos2(right, eq_top),
            "drag vertically / double-click resets",
            7.2,
            DIM,
            Align2::RIGHT_TOP,
        );
        let cell = inner_w / 4.0;
        let knob_y = eq_top + 42.0;
        for i in 0..3 {
            let cx = left + cell * (i as f32 + 0.5);
            let names = ["LOW", "MID", "HIGH"];
            txt(
                &p,
                pos2(cx, eq_top + 14.0),
                names[i],
                9.0,
                MUTED,
                Align2::CENTER_CENTER,
            );
            knob(
                ui,
                pos2(cx, knob_y),
                17.0,
                &mut params.eq_db[i],
                -24.0,
                6.0,
                0.0,
                if params.kill[i] { RED } else { accent },
                (base_id, "eq", i),
            );
            mono(
                &p,
                pos2(cx, eq_top + 66.0),
                if params.kill[i] {
                    "KILL".to_string()
                } else {
                    db_text(params.eq_db[i])
                },
                8.2,
                if params.kill[i] { RED } else { MUTED },
                Align2::CENTER_CENTER,
            );
            let kill = Rect::from_center_size(pos2(cx, eq_top + 82.0), vec2(38.0, 15.0));
            if chip(ui, kill, "KILL", RED, params.kill[i], (base_id, "kill", i)).clicked() {
                params.kill[i] = !params.kill[i];
            }
        }
        // bipolar filter
        let cx = left + cell * 3.5;
        txt(
            &p,
            pos2(cx, eq_top + 14.0),
            "FILTER",
            9.0,
            MUTED,
            Align2::CENTER_CENTER,
        );
        knob(
            ui,
            pos2(cx, knob_y),
            17.0,
            &mut params.filter,
            -1.0,
            1.0,
            0.0,
            STEM_COLORS[3],
            (base_id, "filter"),
        );
        let ftext = if params.filter.abs() < 0.02 {
            "OFF".to_string()
        } else if params.filter < 0.0 {
            format!("LP {:>3}%", (-params.filter * 100.0).round() as i32)
        } else {
            format!("HP {:>3}%", (params.filter * 100.0).round() as i32)
        };
        mono(
            &p,
            pos2(cx, eq_top + 66.0),
            ftext,
            8.2,
            if params.filter.abs() < 0.02 { MUTED } else { STEM_COLORS[3] },
            Align2::CENTER_CENTER,
        );
        p.line_segment(
            [
                pos2(cx - 24.0, eq_top + 82.0),
                pos2(cx + 24.0, eq_top + 82.0),
            ],
            Stroke::new(1.0, alpha(STEM_COLORS[3], 90)),
        );
        p.circle_filled(pos2(cx, eq_top + 82.0), 2.0, alpha(STEM_COLORS[3], 140));
        txt(
            &p,
            pos2(cx - 26.0, eq_top + 82.0),
            "LP",
            7.0,
            DIM,
            Align2::RIGHT_CENTER,
        );
        txt(
            &p,
            pos2(cx + 26.0, eq_top + 82.0),
            "HP",
            7.0,
            DIM,
            Align2::LEFT_CENTER,
        );

        // -------------------------------------------------------------------
        // hot cues
        // -------------------------------------------------------------------
        label(&p, pos2(left, pads_top), "HOT CUES");
        txt(
            &p,
            pos2(right, pads_top),
            "shift+click sets / right-click clears",
            7.2,
            DIM,
            Align2::RIGHT_TOP,
        );
        let pgap = 6.0;
        let pw = (inner_w - pgap * 3.0) / 4.0;
        let play_sec = params.position / SR as f64;
        for i in 0..4 {
            let r = Rect::from_min_size(
                pos2(left + i as f32 * (pw + pgap), pads_top + 16.0),
                vec2(pw, 28.0),
            );
            let resp = ui.interact(
                r,
                ui.make_persistent_id((base_id, "pad", i)),
                Sense::click(),
            );
            let cue = ui_state.hotcues[i];
            let fill = if cue.is_some() {
                alpha(accent, 26)
            } else {
                PANEL2
            };
            p.rect_filled(r, CornerRadius::same(6), fill);
            p.rect_stroke(
                r,
                CornerRadius::same(6),
                Stroke::new(1.0, if cue.is_some() { alpha(accent, 150) } else { LINE }),
                StrokeKind::Inside,
            );
            mono(
                &p,
                pos2(r.left() + 9.0, r.center().y),
                format!("{}", i + 1),
                12.0,
                if cue.is_some() { accent } else { DIM },
                Align2::LEFT_CENTER,
            );
            if let Some(t) = cue {
                mono(
                    &p,
                    pos2(r.right() - 9.0, r.center().y),
                    fmt_time(t),
                    8.8,
                    MUTED,
                    Align2::RIGHT_CENTER,
                );
                p.circle_filled(pos2(r.right() - 9.0, r.top() + 5.0), 1.8, accent);
            } else {
                txt(
                    &p,
                    pos2(r.right() - 9.0, r.center().y),
                    "SET",
                    7.5,
                    DIM,
                    Align2::RIGHT_CENTER,
                );
            }
            if resp.secondary_clicked() {
                ui_state.hotcues[i] = None;
            } else if resp.clicked() {
                let shift = ui.input(|i| i.modifiers.shift);
                if shift {
                    ui_state.hotcues[i] = Some(play_sec);
                } else if let Some(t) = cue {
                    seek_sec = Some(t);
                } else {
                    ui_state.hotcues[i] = Some(play_sec);
                }
            }
        }

        // -------------------------------------------------------------------
        // loops
        // -------------------------------------------------------------------
        label(&p, pos2(left, loop_top + 4.0), "LOOP");
        let loop_x = left + 40.0;
        let beat = 60.0 / track.bpm.max(1.0) as f64;
        let beats = [0.25f32, 0.5, 1.0, 2.0, 4.0];
        let labels = ["1/4", "1/2", "1", "2", "4"];
        for i in 0..5 {
            let r = Rect::from_min_size(pos2(loop_x + i as f32 * 36.0, loop_top), vec2(32.0, 20.0));
            let is_current = params.loop_active && (ui_state.loop_beats - beats[i]).abs() < 0.01;
            if chip(ui, r, labels[i], accent, is_current, (base_id, "loop", i)).clicked() {
                if is_current {
                    params.loop_active = false;
                } else {
                    let beat_frames = beat * SR as f64;
                    let total = track.frames() as f64;
                    let wanted = beats[i] as f64 * beat_frames;
                    let mut start = (params.position / beat_frames).floor() * beat_frames;
                    if start + wanted > total {
                        start = (total - wanted).max(0.0);
                    }
                    params.loop_in = start;
                    params.loop_out = (start + wanted).min(total);
                    params.loop_active = true;
                    ui_state.loop_beats = beats[i];
                }
            }
        }
        let out_rect = Rect::from_min_size(pos2(loop_x + 5.0 * 36.0, loop_top), vec2(38.0, 20.0));
        if chip(ui, out_rect, "OUT", accent, false, (base_id, "loopout")).clicked() {
            if params.loop_active {
                let end = params.position;
                if end > params.loop_in + 64.0 {
                    params.loop_out = end;
                    ui_state.loop_beats =
                        ((params.loop_out - params.loop_in) / (beat * SR as f64)) as f32;
                }
            } else {
                self.notify("Start a loop first, then press OUT");
            }
        }
        let loop_text = if params.loop_active {
            format!(
                "{:.2} BEATS / {} - {}",
                (params.loop_out - params.loop_in) / (beat * SR as f64),
                fmt_time(params.loop_in / SR as f64),
                fmt_time(params.loop_out / SR as f64)
            )
        } else {
            "no loop armed".to_string()
        };
        txt(
            &p,
            pos2(right, loop_top + 10.0),
            loop_text,
            7.2,
            if params.loop_active { accent } else { DIM },
            Align2::RIGHT_CENTER,
        );

        // -------------------------------------------------------------------
        // write controls back
        // -------------------------------------------------------------------
        self.deck[d] = ui_state;
        let mut e = self.lock();
        let cur = &mut e.p[d];
        cur.rate = params.rate;
        cur.gain = params.gain;
        cur.trim = params.trim;
        cur.eq_db = params.eq_db;
        cur.kill = params.kill;
        cur.filter = params.filter;
        cur.stem_gain = params.stem_gain;
        cur.stem_mute = params.stem_mute;
        cur.stem_solo = params.stem_solo;
        cur.loop_active = params.loop_active;
        cur.loop_in = params.loop_in;
        cur.loop_out = params.loop_out;
        cur.cue = params.cue;
        if let Some(sec) = seek_sec {
            cur.position = (sec * SR as f64).clamp(0.0, track.frames().saturating_sub(1) as f64);
        }
        if let Some(playing) = play_override {
            cur.playing = playing;
        }
    }

    fn draw_empty_deck(&mut self, ui: &mut Ui, rect: Rect, d: usize, accent: Color32) {
        let p = ui.painter().with_clip_rect(rect);
        let left = rect.left() + 14.0;
        let top = rect.top() + 14.0;
        txt(
            &p,
            pos2(left, top),
            if d == 0 { "DECK A" } else { "DECK B" },
            9.0,
            accent,
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(left, top + 28.0),
            "No track loaded",
            20.0,
            TEXT,
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(left, top + 56.0),
            "Load a song to analyse its four stems.",
            10.0,
            MUTED,
            Align2::LEFT_TOP,
        );
        let button = Rect::from_min_size(pos2(left, top + 86.0), vec2(150.0, 32.0));
        if text_button(ui, button, "LOAD AUDIO", accent, true, ("empty", d)).clicked() {
            self.pick_file(d);
        }
        let hint = Rect::from_min_size(pos2(left, top + 140.0), vec2(rect.width() - 28.0, 100.0));
        p.rect_filled(hint, CornerRadius::same(8), PANEL2);
        txt(
            &p,
            pos2(hint.left() + 12.0, hint.top() + 12.0),
            "MP3 / WAV / FLAC / M4A / OGG",
            8.5,
            MUTED,
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(hint.left() + 12.0, hint.top() + 30.0),
            "Files are analysed once on load, then split into",
            8.5,
            DIM,
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(hint.left() + 12.0, hint.top() + 44.0),
            "vocals, drums, bass and instruments in memory.",
            8.5,
            DIM,
            Align2::LEFT_TOP,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_waveform(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        track: &TrackData,
        params: &DeckParams,
        accent: Color32,
        d: usize,
        seek_sec: &mut Option<f64>,
    ) {
        let p = ui.painter().with_clip_rect(rect);
        p.rect_filled(rect, CornerRadius::same(8), Color32::from_rgb(9, 11, 17));
        p.rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(1.0, LINE_SOFT),
            StrokeKind::Inside,
        );

        let center = params.position / SR as f64;
        let half = 7.0;
        let columns = (rect.width() as usize).clamp(80, 900);
        let peaks = track.window_peaks(center, half, columns);
        let any_solo = params.stem_solo.iter().any(|&s| s);
        let lane_h = rect.height() / STEM_COUNT as f32;

        for s in 0..STEM_COUNT {
            let lane_top = rect.top() + lane_h * s as f32;
            let mid = lane_top + lane_h * 0.5;
            let amp = (lane_h * 0.5 - 6.0).max(4.0);
            let audible = if any_solo {
                params.stem_solo[s]
            } else {
                !params.stem_mute[s]
            };
            let color = if audible {
                alpha(STEM_COLORS[s], 210)
            } else {
                alpha(STEM_COLORS[s], 38)
            };
            p.line_segment(
                [
                    pos2(rect.left() + 1.0, mid),
                    pos2(rect.right() - 1.0, mid),
                ],
                Stroke::new(1.0, alpha(STEM_COLORS[s], 28)),
            );
            for (i, peak) in peaks.iter().enumerate() {
                let x = rect.left() + (i as f32 + 0.5) * rect.width() / columns as f32;
                let v = (peak[s].sqrt() * 1.25).min(1.0) * amp;
                if v > 0.4 {
                    p.line_segment([pos2(x, mid - v), pos2(x, mid + v)], Stroke::new(1.0, color));
                }
            }
            // lane label
            let tag = Rect::from_min_size(pos2(rect.left() + 7.0, lane_top + 5.0), vec2(28.0, 11.0));
            p.rect_filled(tag, CornerRadius::same(3), Color32::from_black_alpha(110));
            mono(
                &p,
                tag.center(),
                STEM_SHORT[s],
                6.8,
                alpha(STEM_COLORS[s], if audible { 220 } else { 100 }),
                Align2::CENTER_CENTER,
            );
        }

        // beat grid
        let beat = 60.0 / track.bpm.max(1.0) as f64;
        let t0 = center - half;
        let t1 = center + half;
        let mut k = (t0 / beat).floor() as i64;
        while (k as f64) * beat <= t1 {
            let t = k as f64 * beat;
            if t >= t0 {
                let x = rect.left() + ((t - t0) / (half * 2.0)) as f32 * rect.width();
                let bar = k % 4 == 0;
                p.line_segment(
                    [pos2(x, rect.top()), pos2(x, rect.bottom())],
                    Stroke::new(1.0, alpha(if bar { TEXT } else { MUTED }, if bar { 22 } else { 11 })),
                );
                if bar && rect.width() > 330.0 {
                    mono(
                        &p,
                        pos2(x + 3.0, rect.bottom() - 4.0),
                        format!("{}", k / 4 + 1),
                        6.8,
                        alpha(MUTED, 90),
                        Align2::LEFT_BOTTOM,
                    );
                }
            }
            k += 1;
        }

        // centre playhead
        let cx = rect.center().x;
        p.rect_filled(
            Rect::from_center_size(pos2(cx, rect.center().y), vec2(6.0, rect.height())),
            CornerRadius::ZERO,
            alpha(accent, 18),
        );
        p.line_segment(
            [pos2(cx, rect.top() + 1.0), pos2(cx, rect.bottom() - 1.0)],
            Stroke::new(1.2, accent),
        );
        p.add(egui::Shape::convex_polygon(
            vec![
                pos2(cx - 5.0, rect.top() + 1.0),
                pos2(cx + 5.0, rect.top() + 1.0),
                pos2(cx, rect.top() + 8.0),
            ],
            accent,
            Stroke::NONE,
        ));

        // scrub interaction
        let resp = ui.interact(
            rect,
            ui.make_persistent_id(("wave", d)),
            Sense::click_and_drag(),
        );
        if resp.clicked() || resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let frac = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
                let t = center - half + frac * half * 2.0;
                *seek_sec = Some(t.clamp(0.0, track.duration as f64));
            }
        }
        if let Some(pos) = resp.hover_pos() {
            let frac = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0) as f64;
            let t = (center - half + frac * half * 2.0).clamp(0.0, track.duration as f64);
            p.line_segment(
                [pos2(pos.x, rect.top()), pos2(pos.x, rect.bottom())],
                Stroke::new(1.0, alpha(TEXT, 70)),
            );
            let tag = Rect::from_min_size(
                pos2((pos.x + 6.0).min(rect.right() - 54.0), rect.top() + 6.0),
                vec2(48.0, 15.0),
            );
            p.rect_filled(tag, CornerRadius::same(4), Color32::from_black_alpha(180));
            mono(
                &p,
                tag.center(),
                fmt_time(t),
                8.0,
                TEXT,
                Align2::CENTER_CENTER,
            );
        }
        // corner time labels
        mono(
            &p,
            pos2(rect.left() + 7.0, rect.bottom() - 4.0),
            fmt_time((center - half).max(0.0)),
            7.0,
            alpha(DIM, 200),
            Align2::LEFT_BOTTOM,
        );
        mono(
            &p,
            pos2(rect.right() - 7.0, rect.bottom() - 4.0),
            format!("+{}s", (half * 2.0) as i32),
            7.0,
            alpha(DIM, 200),
            Align2::RIGHT_BOTTOM,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_overview(
        &mut self,
        ui: &mut Ui,
        rect: Rect,
        track: &TrackData,
        params: &DeckParams,
        accent: Color32,
        hotcues: &[Option<f64>; 4],
        d: usize,
        seek_sec: &mut Option<f64>,
    ) {
        let p = ui.painter().with_clip_rect(rect);
        p.rect_filled(rect, CornerRadius::same(6), Color32::from_rgb(10, 12, 18));
        p.rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0, LINE_SOFT),
            StrokeKind::Inside,
        );
        let inner = rect.shrink(3.0);
        let columns = (inner.width() / 2.0) as usize;
        let dur = params.position / SR as f64;
        let played_x = inner.left() + (dur / track.duration.max(0.01) as f64) as f32 * inner.width();
        for i in 0..columns {
            let frac = (i as f32 + 0.5) / columns as f32;
            let bucket = (frac * track.peaks.len() as f32) as usize;
            let peak = track
                .peaks
                .get(bucket.min(track.peaks.len().saturating_sub(1)))
                .map(|v| v.iter().copied().fold(0.0f32, f32::max))
                .unwrap_or(0.0)
                .sqrt();
            let x = inner.left() + frac * inner.width();
            let h = (peak * inner.height() * 0.92).max(1.0);
            let mid = inner.center().y;
            let col = if x <= played_x {
                alpha(accent, 150)
            } else {
                alpha(accent, 66)
            };
            p.line_segment([pos2(x, mid - h * 0.5), pos2(x, mid + h * 0.5)], Stroke::new(1.5, col));
        }
        // loop band
        if params.loop_active && params.loop_out > params.loop_in {
            let x0 = inner.left()
                + (params.loop_in / SR as f64 / track.duration.max(0.01) as f64) as f32
                    * inner.width();
            let x1 = inner.left()
                + (params.loop_out / SR as f64 / track.duration.max(0.01) as f64) as f32
                    * inner.width();
            p.rect_filled(
                Rect::from_min_max(pos2(x0, inner.top()), pos2(x1.max(x0 + 2.0), inner.bottom())),
                CornerRadius::same(2),
                alpha(accent, 46),
            );
        }
        // hot cue markers
        for (i, cue) in hotcues.iter().enumerate() {
            if let Some(t) = cue {
                let x = inner.left()
                    + (*t / track.duration.max(0.01) as f64) as f32 * inner.width();
                p.rect_filled(
                    Rect::from_min_max(pos2(x - 1.0, inner.top()), pos2(x + 2.0, inner.bottom())),
                    CornerRadius::same(1),
                    alpha(accent, 200),
                );
                mono(
                    &p,
                    pos2(x + 3.0, inner.top() + 1.0),
                    format!("{}", i + 1),
                    6.5,
                    alpha(accent, 210),
                    Align2::LEFT_TOP,
                );
            }
        }
        // playhead
        p.line_segment(
            [pos2(played_x, inner.top()), pos2(played_x, inner.bottom())],
            Stroke::new(1.2, TEXT),
        );
        let resp = ui.interact(
            rect,
            ui.make_persistent_id(("overview", d)),
            Sense::click_and_drag(),
        );
        if resp.clicked() || resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let frac = ((pos.x - inner.left()) / inner.width()).clamp(0.0, 1.0) as f64;
                *seek_sec = Some(frac * track.duration as f64);
            }
        }
        txt(
            &p,
            pos2(rect.left() + 7.0, rect.center().y),
            fmt_time(dur),
            7.0,
            Color32::from_black_alpha(220),
            Align2::LEFT_CENTER,
        );
        mono(
            &p,
            pos2(rect.right() - 7.0, rect.center().y),
            fmt_time(track.duration as f64),
            7.0,
            Color32::from_black_alpha(220),
            Align2::RIGHT_CENTER,
        );
    }
}

fn draw_artwork(p: &egui::Painter, rect: Rect, track: &TrackData, accent: Color32) {
    let base = mix(PANEL3, accent, 0.14);
    p.rect_filled(rect, CornerRadius::same(9), base);
    let inner = rect.shrink(2.0);
    // abstract concentric groove
    for i in 0..5 {
        let r = 10.0 + i as f32 * 5.0;
        p.circle_stroke(
            inner.center(),
            r,
            Stroke::new(1.0, alpha(accent, 34 - i * 5)),
        );
    }
    // stem light bars
    for s in 0..4 {
        let x = inner.left() + 8.0 + s as f32 * 11.0;
        let h = if track.is_demo {
            [16.0, 23.0, 12.0, 19.0][s]
        } else {
            [15.0, 20.0, 13.0, 18.0][s]
        };
        p.rect_filled(
            Rect::from_min_max(
                pos2(x, inner.bottom() - 8.0 - h),
                pos2(x + 3.0, inner.bottom() - 8.0),
            ),
            CornerRadius::same(2),
            alpha(STEM_COLORS[s], 190),
        );
    }
    p.circle_filled(
        pos2(inner.right() - 8.0, inner.top() + 8.0),
        2.5,
        if track.is_demo { GREEN } else { accent },
    );
    p.rect_stroke(
        rect,
        CornerRadius::same(9),
        Stroke::new(1.0, alpha(accent, 90)),
        StrokeKind::Inside,
    );
}
