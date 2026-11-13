//! Left-hand library: demo tracks, file loading, stem legend and quick start.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, Rect, Stroke, StrokeKind, Ui,
};

use crate::widgets::*;
use crate::StemForgeApp;
use crate::track::demo_catalog;

impl StemForgeApp {
    pub(crate) fn draw_library(&mut self, ui: &mut Ui, rect: Rect) {
        card(ui.painter(), rect, 13);
        let p = ui.painter().with_clip_rect(rect);
        let left = rect.left() + 12.0;
        let right = rect.right() - 12.0;
        let top = rect.top() + 12.0;
        let inner_w = right - left;

        label(&p, pos2(left, top), "LIBRARY");
        txt(
            &p,
            pos2(right, top),
            "4 DEMOS",
            7.5,
            DIM,
            Align2::RIGHT_TOP,
        );
        p.line_segment(
            [pos2(left, top + 16.0), pos2(right, top + 16.0)],
            Stroke::new(1.0, LINE_SOFT),
        );

        // search field
        let search_rect = Rect::from_min_size(pos2(left, top + 24.0), vec2(inner_w, 28.0));
        p.rect_filled(search_rect, CornerRadius::same(7), PANEL2);
        p.rect_stroke(
            search_rect,
            CornerRadius::same(7),
            Stroke::new(1.0, LINE),
            StrokeKind::Inside,
        );
        // magnifier
        p.circle_stroke(
            pos2(search_rect.left() + 12.0, search_rect.center().y - 1.0),
            4.0,
            Stroke::new(1.2, DIM),
        );
        p.line_segment(
            [
                pos2(search_rect.left() + 15.0, search_rect.center().y + 2.0),
                pos2(search_rect.left() + 18.0, search_rect.center().y + 5.0),
            ],
            Stroke::new(1.2, DIM),
        );
        let edit_rect = Rect::from_min_max(
            pos2(search_rect.left() + 23.0, search_rect.top() + 2.0),
            pos2(search_rect.right() - 6.0, search_rect.bottom() - 2.0),
        );
        ui.put(
            edit_rect,
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search demos")
                .font(egui::FontId::proportional(10.0))
                .text_color(TEXT)
                .frame(
                    egui::Frame::new()
                        .fill(Color32::TRANSPARENT)
                        .inner_margin(egui::Margin::symmetric(0, 4)),
                ),
        );

        // demo list
        let query = self.search.trim().to_lowercase();
        let demos: Vec<(usize, &'static crate::track::DemoMeta)> = demo_catalog()
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                query.is_empty()
                    || m.name.to_lowercase().contains(&query)
                    || m.artist.to_lowercase().contains(&query)
            })
            .collect();

        let list_top = top + 62.0;
        let row_h = 56.0;
        let gap = 4.0;
        if demos.is_empty() {
            txt(
                &p,
                pos2(rect.center().x, list_top + 40.0),
                "No demos match",
                9.5,
                DIM,
                Align2::CENTER_CENTER,
            );
            txt(
                &p,
                pos2(rect.center().x, list_top + 56.0),
                "Clear the search or load a file.",
                8.0,
                DIM,
                Align2::CENTER_CENTER,
            );
        }
        for (row, (index, meta)) in demos.iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(left, list_top + row as f32 * (row_h + gap)),
                vec2(inner_w, row_h),
            );
            let hovered = r.contains(ui.ctx().pointer_hover_pos().unwrap_or(pos2(-99.0, -99.0)));
            if hovered {
                p.rect_filled(r, CornerRadius::same(8), alpha(PANEL3, 150));
            }
            // thumbnail
            let art = Rect::from_min_size(pos2(r.left() + 6.0, r.top() + 6.0), vec2(44.0, 44.0));
            let tint = STEM_COLORS[meta.accent as usize % 4];
            p.rect_filled(art, CornerRadius::same(7), mix(PANEL3, tint, 0.25));
            for i in 0..3 {
                p.circle_stroke(
                    art.center(),
                    8.0 + i as f32 * 5.0,
                    Stroke::new(1.0, alpha(tint, 90 - i * 20)),
                );
            }
            p.circle_filled(art.center(), 3.0, alpha(tint, 220));

            let tx = art.right() + 8.0;
            txt(
                &p,
                pos2(tx, r.top() + 8.0),
                fit(meta.name, 15),
                11.0,
                TEXT,
                Align2::LEFT_TOP,
            );
            txt(
                &p,
                pos2(tx, r.top() + 23.0),
                fit(meta.artist, 16),
                8.2,
                MUTED,
                Align2::LEFT_TOP,
            );
            mono(
                &p,
                pos2(tx, r.top() + 38.0),
                format!("{:.0}  {}", meta.bpm, meta.key),
                7.2,
                DIM,
                Align2::LEFT_TOP,
            );

            let a_rect = Rect::from_min_size(pos2(r.right() - 42.0, r.top() + 10.0), vec2(17.0, 16.0));
            let b_rect = Rect::from_min_size(pos2(r.right() - 22.0, r.top() + 10.0), vec2(17.0, 16.0));
            if chip(ui, a_rect, "A", DECK_A, false, ("lib", index, "a")).clicked() {
                self.load_demo(0, *index);
            }
            if chip(ui, b_rect, "B", DECK_B, false, ("lib", index, "b")).clicked() {
                self.load_demo(1, *index);
            }
            mono(
                &p,
                pos2(r.right() - 22.0, r.top() + 40.0),
                format!("{}", index + 1),
                6.8,
                alpha(DIM, 120),
                Align2::RIGHT_TOP,
            );
        }

        // file loading
        let load_y = list_top + 4.0 * (row_h + gap) + 4.0;
        label(&p, pos2(left, load_y), "LOAD AUDIO FILE");
        let half = (inner_w - 6.0) / 2.0;
        let a_load = Rect::from_min_size(pos2(left, load_y + 16.0), vec2(half, 28.0));
        let b_load = Rect::from_min_size(pos2(left + half + 6.0, load_y + 16.0), vec2(half, 28.0));
        if text_button(ui, a_load, "TO DECK A", DECK_A, false, "lib-load-a").clicked() {
            self.pick_file(0);
        }
        if text_button(ui, b_load, "TO DECK B", DECK_B, false, "lib-load-b").clicked() {
            self.pick_file(1);
        }

        // stem legend
        let legend_y = load_y + 64.0;
        label(&p, pos2(left, legend_y), "STEMS IN MEMORY");
        txt(
            &p,
            pos2(right, legend_y),
            "4 BUSES",
            7.2,
            DIM,
            Align2::RIGHT_TOP,
        );
        for s in 0..4 {
            let y = legend_y + 18.0 + s as f32 * 21.0;
            let color = STEM_COLORS[s];
            p.circle_filled(pos2(left + 4.0, y + 5.0), 3.0, color);
            txt(
                &p,
                pos2(left + 14.0, y),
                STEM_NAMES[s],
                9.0,
                MUTED,
                Align2::LEFT_TOP,
            );
            let bar = Rect::from_min_size(pos2(left + 92.0, y + 5.0), vec2(inner_w - 134.0, 3.0));
            p.rect_filled(bar, CornerRadius::same(1), alpha(color, 28));
            p.rect_filled(
                Rect::from_min_size(bar.min, vec2(bar.width() * [0.82, 0.7, 0.64, 0.9][s], 3.0)),
                CornerRadius::same(1),
                alpha(color, 190),
            );
            mono(
                &p,
                pos2(right, y + 2.0),
                STEM_SHORT[s],
                6.8,
                alpha(color, 200),
                Align2::RIGHT_TOP,
            );
        }

        // quick start card
        let tip_y = legend_y + 112.0;
        let tip = Rect::from_min_max(
            pos2(left, tip_y),
            pos2(right, (rect.bottom() - 28.0).max(tip_y + 140.0)),
        );
        p.rect_filled(tip, CornerRadius::same(9), PANEL2);
        p.rect_stroke(
            tip,
            CornerRadius::same(9),
            Stroke::new(1.0, LINE_SOFT),
            StrokeKind::Inside,
        );
        txt(
            &p,
            pos2(tip.left() + 10.0, tip.top() + 9.0),
            "QUICK START",
            8.0,
            alpha(DECK_A, 230),
            Align2::LEFT_TOP,
        );
        let steps = [
            "1  Load a song on each deck.",
            "2  Play, then mute or solo stems.",
            "3  Blend with the crossfader.",
            "4  Press REC to capture the mix.",
        ];
        for (i, step) in steps.iter().enumerate() {
            txt(
                &p,
                pos2(tip.left() + 10.0, tip.top() + 26.0 + i as f32 * 15.0),
                *step,
                8.2,
                MUTED,
                Align2::LEFT_TOP,
            );
        }
        if tip.height() > 200.0 {
            let y = tip.top() + 92.0;
            p.line_segment(
                [
                    pos2(tip.left() + 10.0, y - 8.0),
                    pos2(tip.right() - 10.0, y - 8.0),
                ],
                Stroke::new(1.0, LINE_SOFT),
            );
            label(&p, pos2(tip.left() + 10.0, y), "ON THE FLY");
            let moves = [
                ("HOT CUES", "tap to jump"),
                ("LOOPS", "1/4 to 4 beats"),
                ("FILTER", "LP / HP sweep"),
                ("SYNC", "one-click match"),
            ];
            let col_w = (inner_w - 20.0) / 2.0;
            for (i, (name, what)) in moves.iter().enumerate() {
                let cx = tip.left() + 10.0 + (i % 2) as f32 * col_w;
                let yy = y + 19.0 + (i / 2) as f32 * 33.0;
                txt(
                    &p,
                    pos2(cx, yy),
                    *name,
                    7.2,
                    alpha(TEXT, 220),
                    Align2::LEFT_TOP,
                );
                txt(
                    &p,
                    pos2(cx, yy + 10.0),
                    *what,
                    7.2,
                    DIM,
                    Align2::LEFT_TOP,
                );
            }
            if tip.height() > 235.0 {
                let idea = Rect::from_min_max(
                    pos2(tip.left() + 8.0, tip.bottom() - 66.0),
                    pos2(tip.right() - 8.0, tip.bottom() - 42.0),
                );
                p.rect_filled(idea, CornerRadius::same(5), alpha(DECK_A, 16));
                txt(
                    &p,
                    pos2(idea.left() + 7.0, idea.top() + 5.0),
                    "Try: mute A's vocals while B keeps its own.",
                    7.2,
                    alpha(DECK_A, 230),
                    Align2::LEFT_TOP,
                );
                txt(
                    &p,
                    pos2(idea.left() + 7.0, idea.top() + 14.5),
                    "Same tempo, different words, one live mashup.",
                    7.0,
                    DIM,
                    Align2::LEFT_TOP,
                );
            }
        }
        p.line_segment(
            [
                pos2(tip.left() + 10.0, tip.bottom() - 34.0),
                pos2(tip.right() - 10.0, tip.bottom() - 34.0),
            ],
            Stroke::new(1.0, LINE_SOFT),
        );
        txt(
            &p,
            pos2(tip.left() + 10.0, tip.bottom() - 27.0),
            "Everything runs on this computer.",
            7.5,
            DIM,
            Align2::LEFT_TOP,
        );
        txt(
            &p,
            pos2(tip.left() + 10.0, tip.bottom() - 16.0),
            "No account, no cloud, no upload.",
            7.5,
            DIM,
            Align2::LEFT_TOP,
        );

        // footer
        txt(
            &p,
            pos2(rect.center().x, rect.bottom() - 13.0),
            "STEMFORGE 0.1  /  44.1 kHz",
            6.8,
            alpha(DIM, 140),
            Align2::CENTER_CENTER,
        );
    }
}
