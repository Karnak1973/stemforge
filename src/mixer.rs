//! Centre mixer: channel faders, master, crossfader and the session recorder.

use std::time::Instant;

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui,
};

use crate::widgets::*;
use crate::StemForgeApp;
use crate::track::SR;

impl StemForgeApp {
    pub(crate) fn draw_mixer(&mut self, ui: &mut Ui, rect: Rect) {
        card(ui.painter(), rect, 13);
        let p = ui.painter().with_clip_rect(rect);
        let left = rect.left() + 12.0;
        let right = rect.right() - 12.0;
        let top = rect.top() + 12.0;
        let inner_w = right - left;

        let (levels, master_level, cross0, master0, gains, rec, engine_sr, device) = {
            let e = self.lock();
            (
                e.levels,
                e.master_level,
                e.crossfader,
                e.master,
                [e.p[0].gain, e.p[1].gain],
                e.is_recording(),
                e.sample_rate,
                self.audio
                    .as_ref()
                    .map(|a| a.device_name.clone())
                    .unwrap_or_else(|| "Render device".to_string()),
            )
        };
        let mut cross = cross0;
        let mut master = master0;
        let mut gain = gains;

        label(&p, pos2(left, top), "MIXER");
        txt(
            &p,
            pos2(right, top),
            "A + B",
            7.5,
            DIM,
            Align2::RIGHT_TOP,
        );
        p.line_segment(
            [pos2(left, top + 16.0), pos2(right, top + 16.0)],
            Stroke::new(1.0, LINE_SOFT),
        );

        // vertical geometry, anchored to the bottom
        let inner_bottom = rect.bottom() - 12.0;
        let engine_h = 92.0;
        let rec_h = 128.0;
        let cross_h = 58.0;
        let master_h = 80.0;
        let gap = 8.0;
        let engine_r = Rect::from_min_max(
            pos2(left, inner_bottom - engine_h),
            pos2(right, inner_bottom),
        );
        let rec_r = Rect::from_min_max(
            pos2(left, engine_r.top() - gap - rec_h),
            pos2(right, engine_r.top() - gap),
        );
        let cross_r = Rect::from_min_max(
            pos2(left, rec_r.top() - gap - cross_h),
            pos2(right, rec_r.top() - gap),
        );
        let master_r = Rect::from_min_max(
            pos2(left, cross_r.top() - gap - master_h),
            pos2(right, cross_r.top() - gap),
        );
        let channel_top = top + 22.0;
        let channel_bottom = master_r.top() - 10.0;

        // -------------------------------------------------------------------
        // channel strips
        // -------------------------------------------------------------------
        let strip_gap = 10.0;
        let strip_w = (inner_w - strip_gap) / 2.0;
        for d in 0..2 {
            let accent = deck_color(d);
            let sx = left + d as f32 * (strip_w + strip_gap);
            let strip = Rect::from_min_size(
                pos2(sx, channel_top),
                vec2(strip_w, (channel_bottom - channel_top).max(120.0)),
            );
            p.rect_filled(strip, CornerRadius::same(8), PANEL2);
            p.rect_stroke(
                strip,
                CornerRadius::same(8),
                Stroke::new(1.0, LINE_SOFT),
                StrokeKind::Inside,
            );
            txt(
                &p,
                pos2(strip.center().x, strip.top() + 6.0),
                if d == 0 { "CHANNEL A" } else { "CHANNEL B" },
                7.5,
                accent,
                Align2::CENTER_TOP,
            );
            mono(
                &p,
                pos2(strip.center().x, strip.top() + 18.0),
                format!("{:>3}%", (gain[d] * 100.0).round() as i32),
                8.0,
                MUTED,
                Align2::CENTER_TOP,
            );
            let meter_rect = Rect::from_min_max(
                pos2(strip.left() + 8.0, strip.top() + 36.0),
                pos2(strip.left() + 17.0, strip.bottom() - 30.0),
            );
            meter(&p, meter_rect, levels[d], false, 26);
            let fader_rect = Rect::from_min_max(
                pos2(strip.left() + 28.0, strip.top() + 34.0),
                pos2(strip.right() - 16.0, strip.bottom() - 30.0),
            );
            vfader(
                ui,
                fader_rect,
                &mut gain[d],
                0.0,
                1.15,
                accent,
                ("mix", "gain", d),
            );
            mono(
                &p,
                pos2(strip.center().x, strip.bottom() - 14.0),
                if gain[d] < 0.005 {
                    "-inf".to_string()
                } else {
                    format!("{:+.1} dB", 20.0 * (gain[d] / 0.82).log10())
                },
                7.5,
                DIM,
                Align2::CENTER_CENTER,
            );
        }

        // -------------------------------------------------------------------
        // master
        // -------------------------------------------------------------------
        label(&p, pos2(left, master_r.top()), "MASTER OUTPUT");
        let knob_c = pos2(left + 24.0, master_r.top() + 44.0);
        knob(
            ui,
            knob_c,
            16.0,
            &mut master,
            0.0,
            1.3,
            0.72,
            GREEN,
            "master-gain",
        );
        mono(
            &p,
            pos2(knob_c.x, master_r.top() + 68.0),
            format!("{:.0}%", master * 100.0),
            7.8,
            MUTED,
            Align2::CENTER_CENTER,
        );
        let m_meter_x = left + 52.0;
        let m_meter_w = inner_w - 52.0;
        for ch in 0..2 {
            let y = master_r.top() + 30.0 + ch as f32 * 14.0;
            txt(
                &p,
                pos2(m_meter_x, y + 3.5),
                if ch == 0 { "L" } else { "R" },
                6.5,
                DIM,
                Align2::LEFT_CENTER,
            );
            meter(
                &p,
                Rect::from_min_size(pos2(m_meter_x + 10.0, y), vec2(m_meter_w - 10.0, 7.0)),
                master_level[ch],
                true,
                24,
            );
        }
        txt(
            &p,
            pos2(right, master_r.top() + 64.0),
            "LIMITER ON",
            6.5,
            alpha(GREEN, 180),
            Align2::RIGHT_CENTER,
        );

        // -------------------------------------------------------------------
        // crossfader
        // -------------------------------------------------------------------
        label(&p, pos2(left, cross_r.top()), "CROSSFADER");
        txt(
            &p,
            pos2(right, cross_r.top()),
            "A  <  BLEND  >  B",
            7.0,
            DIM,
            Align2::RIGHT_TOP,
        );
        let track = Rect::from_min_size(
            pos2(left + 6.0, cross_r.top() + 21.0),
            vec2(inner_w - 12.0, 22.0),
        );
        let resp = ui.interact(
            track.expand2(vec2(0.0, 5.0)),
            ui.make_persistent_id("crossfader"),
            Sense::click_and_drag(),
        );
        if resp.dragged() {
            if let Some(pos) = resp.interact_pointer_pos() {
                cross = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
            }
        }
        if resp.double_clicked() {
            cross = 0.5;
        }
        p.rect_filled(
            Rect::from_center_size(track.center(), vec2(track.width(), 5.0)),
            CornerRadius::same(3),
            LINE_SOFT,
        );
        let mid = track.center().x;
        p.rect_filled(
            Rect::from_min_max(
                pos2(track.left(), track.center().y - 2.5),
                pos2(mid, track.center().y + 2.5),
            ),
            CornerRadius::same(2),
            alpha(DECK_A, 90),
        );
        p.rect_filled(
            Rect::from_min_max(
                pos2(mid, track.center().y - 2.5),
                pos2(track.right(), track.center().y + 2.5),
            ),
            CornerRadius::same(2),
            alpha(DECK_B, 90),
        );
        p.line_segment(
            [
                pos2(mid, track.top() - 3.0),
                pos2(mid, track.bottom() + 3.0),
            ],
            Stroke::new(1.0, alpha(TEXT, 90)),
        );
        let thumb_x = track.left() + cross * track.width();
        let thumb = Rect::from_center_size(pos2(thumb_x, track.center().y), vec2(13.0, 24.0));
        p.rect_filled(thumb, CornerRadius::same(3), TEXT);
        p.rect_filled(
            thumb.shrink(1.0),
            CornerRadius::same(2),
            mix(DECK_A, DECK_B, cross),
        );
        p.rect_stroke(
            thumb,
            CornerRadius::same(3),
            Stroke::new(1.0, BG),
            StrokeKind::Inside,
        );
        for i in 0..=4 {
            let x = track.left() + track.width() * i as f32 / 4.0;
            p.line_segment(
                [pos2(x, track.bottom() + 7.0), pos2(x, track.bottom() + 10.0)],
                Stroke::new(1.0, alpha(DIM, 80)),
            );
        }
        txt(
            &p,
            pos2(left, cross_r.bottom() - 8.0),
            "A",
            8.0,
            DECK_A,
            Align2::LEFT_CENTER,
        );
        txt(
            &p,
            pos2(right, cross_r.bottom() - 8.0),
            "B",
            8.0,
            DECK_B,
            Align2::RIGHT_CENTER,
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        // -------------------------------------------------------------------
        // recorder
        // -------------------------------------------------------------------
        p.rect_filled(rec_r, CornerRadius::same(9), PANEL2);
        p.rect_stroke(
            rec_r,
            CornerRadius::same(9),
            Stroke::new(1.0, if rec { alpha(RED, 120) } else { LINE_SOFT }),
            StrokeKind::Inside,
        );
        small_dot(
            &p,
            pos2(rec_r.left() + 12.0, rec_r.top() + 13.0),
            3.0,
            if rec { RED } else { DIM },
        );
        txt(
            &p,
            pos2(rec_r.left() + 20.0, rec_r.top() + 7.0),
            "SESSION RECORDER",
            8.0,
            if rec { RED } else { MUTED },
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(rec_r.right() - 10.0, rec_r.top() + 7.0),
            "WAV",
            6.8,
            DIM,
            Align2::RIGHT_TOP,
        );
        let elapsed = self
            .record_start
            .map(|t| t.elapsed())
            .unwrap_or_else(|| std::time::Duration::from_secs(0));
        let total = elapsed.as_secs();
        mono(
            &p,
            pos2(rec_r.center().x, rec_r.top() + 27.0),
            format!("{:02}:{:02}", total / 60, total % 60),
            26.0,
            if rec { TEXT } else { MUTED },
            Align2::CENTER_TOP,
        );
        let button = Rect::from_min_size(
            pos2(rec_r.left() + 10.0, rec_r.top() + 62.0),
            vec2(rec_r.width() - 20.0, 28.0),
        );
        if text_button(
            ui,
            button,
            if rec { "STOP AND SAVE" } else { "START RECORDING" },
            if rec { RED } else { TEXT },
            rec,
            "rec-toggle",
        )
        .clicked()
        {
            self.toggle_recording();
        }
        txt(
            &p,
            pos2(rec_r.center().x, rec_r.top() + 96.0),
            "Captures the master mix as a stereo WAV.",
            7.3,
            DIM,
            Align2::CENTER_TOP,
        );
        let note = if let Some((text, _)) = &self.toast {
            if text.starts_with("Session saved") || text.starts_with("Could not save") {
                fit(text, 32)
            } else {
                format!("16-bit / {:.1} kHz", engine_sr / 1000.0)
            }
        } else {
            format!("16-bit / {:.1} kHz", engine_sr / 1000.0)
        };
        txt(
            &p,
            pos2(rec_r.center().x, rec_r.bottom() - 12.0),
            note,
            6.8,
            if rec { alpha(RED, 200) } else { DIM },
            Align2::CENTER_CENTER,
        );

        // -------------------------------------------------------------------
        // engine panel
        // -------------------------------------------------------------------
        p.rect_filled(engine_r, CornerRadius::same(9), PANEL2);
        p.rect_stroke(
            engine_r,
            CornerRadius::same(9),
            Stroke::new(1.0, LINE_SOFT),
            StrokeKind::Inside,
        );
        txt(
            &p,
            pos2(engine_r.left() + 10.0, engine_r.top() + 7.0),
            "AUDIO ENGINE",
            8.0,
            alpha(STEM_COLORS[3], 220),
            Align2::LEFT_TOP,
        );
        small_dot(
            &p,
            pos2(engine_r.right() - 12.0, engine_r.top() + 13.0),
            2.5,
            if self.audio.is_some() { GREEN } else { AMBER },
        );
        let rows = [
            ("DEVICE", fit(&device, 22)),
            ("RATE", format!("{:.1} kHz / 512", engine_sr / 1000.0)),
            ("STEMS", format!("4 x {:.1} kHz", SR / 1000.0)),
        ];
        for (i, (k, v)) in rows.iter().enumerate() {
            let y = engine_r.top() + 26.0 + i as f32 * 18.0;
            txt(
                &p,
                pos2(engine_r.left() + 10.0, y),
                *k,
                6.8,
                DIM,
                Align2::LEFT_TOP,
            );
            mono(
                &p,
                pos2(engine_r.right() - 10.0, y),
                v,
                6.8,
                MUTED,
                Align2::RIGHT_TOP,
            );
        }

        // -------------------------------------------------------------------
        // write back
        // -------------------------------------------------------------------
        {
            let mut e = self.lock();
            e.crossfader = cross;
            e.master = master;
            e.p[0].gain = gain[0];
            e.p[1].gain = gain[1];
        }
    }
}
