//! Shared painted widgets and the small design system used by the app.

use eframe::egui::{
    self, pos2, vec2, Align2, Color32, CornerRadius, FontId, Id, Painter, Pos2, Rect, Response,
    Sense, Stroke, StrokeKind, Ui, Vec2,
};

// ---------------------------------------------------------------------------
// palette
// ---------------------------------------------------------------------------

pub const BG: Color32 = Color32::from_rgb(8, 10, 15);
pub const PANEL: Color32 = Color32::from_rgb(15, 18, 26);
pub const PANEL2: Color32 = Color32::from_rgb(21, 25, 35);
pub const PANEL3: Color32 = Color32::from_rgb(27, 32, 44);
pub const LINE: Color32 = Color32::from_rgb(36, 42, 57);
pub const LINE_SOFT: Color32 = Color32::from_rgb(27, 32, 44);
pub const TEXT: Color32 = Color32::from_rgb(233, 238, 248);
pub const MUTED: Color32 = Color32::from_rgb(137, 148, 170);
pub const DIM: Color32 = Color32::from_rgb(88, 98, 118);
pub const GREEN: Color32 = Color32::from_rgb(82, 226, 162);
pub const RED: Color32 = Color32::from_rgb(255, 78, 99);
pub const AMBER: Color32 = Color32::from_rgb(255, 190, 92);

pub const DECK_A: Color32 = Color32::from_rgb(255, 163, 72);
pub const DECK_B: Color32 = Color32::from_rgb(56, 214, 255);

pub const STEM_NAMES: [&str; 4] = ["Vocals", "Drums", "Bass", "Instruments"];
pub const STEM_SHORT: [&str; 4] = ["VOX", "DRM", "BAS", "INS"];
pub const STEM_COLORS: [Color32; 4] = [
    Color32::from_rgb(255, 95, 143),
    Color32::from_rgb(255, 182, 72),
    Color32::from_rgb(124, 108, 255),
    Color32::from_rgb(53, 214, 160),
];

pub fn deck_color(deck: usize) -> Color32 {
    if deck == 0 {
        DECK_A
    } else {
        DECK_B
    }
}

pub fn alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

// ---------------------------------------------------------------------------
// text helpers
// ---------------------------------------------------------------------------

pub fn txt(p: &Painter, pos: Pos2, text: impl ToString, size: f32, color: Color32, align: Align2) {
    p.text(pos, align, text, FontId::proportional(size), color);
}

pub fn mono(p: &Painter, pos: Pos2, text: impl ToString, size: f32, color: Color32, align: Align2) {
    p.text(pos, align, text, FontId::monospace(size), color);
}

pub fn label(p: &Painter, pos: Pos2, text: &str) {
    txt(p, pos, text, 9.5, MUTED, Align2::LEFT_TOP);
}

pub fn fmt_time(sec: f64) -> String {
    let s = sec.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

pub fn fmt_time_tenths(sec: f64) -> String {
    let s = sec.max(0.0);
    let m = (s / 60.0) as u64;
    let rest = s - (m * 60) as f64;
    format!("{}:{:04.1}", m, rest)
}

pub fn fit(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max_chars.saturating_sub(3)).collect();
        out.push_str("...");
        out
    }
}

pub fn db_text(db: f32) -> String {
    if db.abs() < 0.05 {
        "0.0".to_string()
    } else if db > 0.0 {
        format!("+{db:.1}")
    } else {
        format!("{db:.1}")
    }
}

// ---------------------------------------------------------------------------
// panels, pills, buttons
// ---------------------------------------------------------------------------

pub fn card(p: &Painter, rect: Rect, radius: u8) {
    let r = CornerRadius::same(radius);
    p.rect_filled(
        rect.translate(vec2(0.0, 4.0)).expand(1.0),
        r,
        Color32::from_black_alpha(80),
    );
    p.rect_filled(rect, r, PANEL);
    p.rect_stroke(rect, r, Stroke::new(1.0, LINE), StrokeKind::Inside);
}

pub fn inset(p: &Painter, rect: Rect, radius: u8, fill: Color32) {
    p.rect_filled(rect, CornerRadius::same(radius), fill);
}

pub fn divider(p: &Painter, x0: f32, x1: f32, y: f32) {
    p.line_segment(
        [pos2(x0, y), pos2(x1, y)],
        Stroke::new(1.0, LINE_SOFT),
    );
}

pub fn pill(p: &Painter, rect: Rect, text: &str, color: Color32, active: bool) {
    let bg = if active {
        alpha(color, 36)
    } else {
        PANEL2
    };
    p.rect_filled(rect, CornerRadius::same(7), bg);
    if active {
        p.rect_stroke(
            rect,
            CornerRadius::same(7),
            Stroke::new(1.0, alpha(color, 130)),
            StrokeKind::Inside,
        );
    }
    txt(
        p,
        rect.center(),
        text,
        9.5,
        if active { color } else { MUTED },
        Align2::CENTER_CENTER,
    );
}

pub fn chip(
    ui: &mut Ui,
    rect: Rect,
    text: &str,
    color: Color32,
    active: bool,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> Response {
    let resp = ui.interact(rect, ui.make_persistent_id(id), Sense::click());
    pill(ui.painter(), rect, text, color, active);
    if resp.hovered() {
        let p = ui.painter();
        p.rect_stroke(
            rect,
            CornerRadius::same(7),
            Stroke::new(1.0, alpha(color, 90)),
            StrokeKind::Inside,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn text_button(
    ui: &mut Ui,
    rect: Rect,
    text: &str,
    color: Color32,
    filled: bool,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> Response {
    let resp = ui.interact(rect, ui.make_persistent_id(id), Sense::click());
    let p = ui.painter();
    if filled {
        p.rect_filled(rect, CornerRadius::same(8), color);
        txt(p, rect.center(), text, 10.0, BG, Align2::CENTER_CENTER);
    } else {
        p.rect_filled(rect, CornerRadius::same(8), PANEL2);
        p.rect_stroke(
            rect,
            CornerRadius::same(8),
            Stroke::new(1.0, LINE),
            StrokeKind::Inside,
        );
        let hover = resp.hovered();
        txt(
            p,
            rect.center(),
            text,
            10.0,
            if hover { color } else { TEXT },
            Align2::CENTER_CENTER,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

pub fn icon_play(p: &Painter, center: Pos2, size: f32, color: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![
            center + vec2(-size * 0.32, -size * 0.42),
            center + vec2(size * 0.48, 0.0),
            center + vec2(-size * 0.32, size * 0.42),
        ],
        color,
        Stroke::NONE,
    ));
}

pub fn icon_pause(p: &Painter, center: Pos2, size: f32, color: Color32) {
    let w = size * 0.18;
    let h = size * 0.78;
    p.rect_filled(
        Rect::from_center_size(center + vec2(-w * 1.1, 0.0), vec2(w, h)),
        CornerRadius::same(1),
        color,
    );
    p.rect_filled(
        Rect::from_center_size(center + vec2(w * 1.1, 0.0), vec2(w, h)),
        CornerRadius::same(1),
        color,
    );
}

pub fn icon_eject(p: &Painter, center: Pos2, size: f32, color: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![
            center + vec2(0.0, -size * 0.42),
            center + vec2(size * 0.42, size * 0.05),
            center + vec2(-size * 0.42, size * 0.05),
        ],
        color,
        Stroke::NONE,
    ));
    p.rect_filled(
        Rect::from_center_size(center + vec2(0.0, size * 0.28), vec2(size * 0.8, size * 0.12)),
        CornerRadius::same(1),
        color,
    );
}

// ---------------------------------------------------------------------------
// knobs and faders
// ---------------------------------------------------------------------------

fn arc(
    p: &Painter,
    center: Pos2,
    radius: f32,
    a0: f32,
    a1: f32,
    color: Color32,
    width: f32,
) {
    if (a1 - a0).abs() < 0.001 {
        return;
    }
    let steps = (((a1 - a0).abs() / 0.07).ceil() as usize).max(2);
    let mut prev = None;
    for i in 0..=steps {
        let a = a0 + (a1 - a0) * i as f32 / steps as f32;
        let pt = center + vec2(a.cos(), a.sin()) * radius;
        if let Some(q) = prev {
            p.line_segment([q, pt], Stroke::new(width, color));
        }
        prev = Some(pt);
    }
}

/// A round rotary control. Returns its response; value is written live while
/// dragging. Double-click snaps back to `default`.
pub fn knob(
    ui: &mut Ui,
    center: Pos2,
    radius: f32,
    value: &mut f32,
    min: f32,
    max: f32,
    default: f32,
    accent: Color32,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> Response {
    let hit = Rect::from_center_size(center, Vec2::splat(radius * 2.5));
    let resp = ui.interact(hit, ui.make_persistent_id(id), Sense::click_and_drag());
    if resp.dragged() {
        let d = resp.drag_delta();
        *value = (*value + (-d.y + d.x * 0.35) * (max - min) / 150.0).clamp(min, max);
    }
    if resp.double_clicked() {
        *value = default;
    }
    let p = ui.painter();
    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    let start = 135f32.to_radians();
    let sweep = 270f32.to_radians();
    arc(
        p,
        center,
        radius + 5.0,
        start,
        start + sweep,
        alpha(LINE, 220),
        2.5,
    );
    let mid = start + sweep * 0.5;
    let end = start + sweep * t;
    arc(
        p,
        center,
        radius + 5.0,
        mid.min(end),
        mid.max(end),
        accent,
        2.5,
    );
    // tick marks
    for i in 0..=10 {
        let a = start + sweep * i as f32 / 10.0;
        let outer = center + vec2(a.cos(), a.sin()) * (radius + 9.0);
        let inner = center + vec2(a.cos(), a.sin()) * (radius + 6.5);
        p.line_segment(
            [inner, outer],
            Stroke::new(1.0, if i == 5 { alpha(accent, 150) } else { alpha(DIM, 90) }),
        );
    }
    p.circle_filled(center + vec2(0.0, 2.0), radius, Color32::from_black_alpha(120));
    p.circle_filled(center, radius, PANEL3);
    p.circle_stroke(center, radius, Stroke::new(1.0, LINE));
    p.circle_stroke(center, radius - 3.5, Stroke::new(1.0, alpha(BG, 120)));
    let pointer = center + vec2(end.cos(), end.sin()) * (radius - 6.0);
    p.line_segment(
        [center + vec2(end.cos(), end.sin()) * 2.0, pointer],
        Stroke::new(2.2, accent),
    );
    p.circle_filled(pointer, 1.8, TEXT);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    resp
}

pub fn hfader(
    ui: &mut Ui,
    rect: Rect,
    value: &mut f32,
    min: f32,
    max: f32,
    accent: Color32,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> Response {
    let resp = ui.interact(
        rect.expand2(vec2(2.0, 7.0)),
        ui.make_persistent_id(id),
        Sense::click_and_drag(),
    );
    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = ((pos.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
            *value = min + (max - min) * t;
        }
    }
    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    let cy = rect.center().y;
    let p = ui.painter();
    p.rect_filled(
        Rect::from_center_size(rect.center(), vec2(rect.width(), 4.0)),
        CornerRadius::same(2),
        LINE_SOFT,
    );
    let thumb_x = rect.left() + t * rect.width();
    if t > 0.005 {
        p.rect_filled(
            Rect::from_min_max(pos2(rect.left(), cy - 2.0), pos2(thumb_x, cy + 2.0)),
            CornerRadius::same(2),
            alpha(accent, 170),
        );
    }
    for i in 0..=4 {
        let x = rect.left() + rect.width() * i as f32 / 4.0;
        p.line_segment(
            [pos2(x, cy + 8.0), pos2(x, cy + 11.0)],
            Stroke::new(1.0, alpha(DIM, 90)),
        );
    }
    let thumb = Rect::from_center_size(pos2(thumb_x, cy), vec2(11.0, 18.0));
    p.rect_filled(thumb, CornerRadius::same(3), TEXT);
    p.rect_filled(thumb.shrink(1.0), CornerRadius::same(2), accent);
    p.rect_stroke(
        thumb,
        CornerRadius::same(3),
        Stroke::new(1.0, BG),
        StrokeKind::Inside,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    resp
}

pub fn vfader(
    ui: &mut Ui,
    rect: Rect,
    value: &mut f32,
    min: f32,
    max: f32,
    accent: Color32,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> Response {
    let resp = ui.interact(
        rect.expand2(vec2(8.0, 2.0)),
        ui.make_persistent_id(id),
        Sense::click_and_drag(),
    );
    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let t = 1.0 - ((pos.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
            *value = min + (max - min) * t;
        }
    }
    let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    let cx = rect.center().x;
    let p = ui.painter();
    p.rect_filled(
        Rect::from_center_size(rect.center(), vec2(4.0, rect.height())),
        CornerRadius::same(2),
        LINE_SOFT,
    );
    let thumb_y = rect.bottom() - t * rect.height();
    if t > 0.005 {
        p.rect_filled(
            Rect::from_min_max(pos2(cx - 2.0, thumb_y), pos2(cx + 2.0, rect.bottom())),
            CornerRadius::same(2),
            alpha(accent, 170),
        );
    }
    let thumb = Rect::from_center_size(pos2(cx, thumb_y), vec2(26.0, 12.0));
    p.rect_filled(thumb, CornerRadius::same(3), TEXT);
    p.rect_filled(thumb.shrink(1.0), CornerRadius::same(2), accent);
    p.rect_stroke(
        thumb,
        CornerRadius::same(3),
        Stroke::new(1.0, BG),
        StrokeKind::Inside,
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    resp
}

pub fn meter(p: &Painter, rect: Rect, level: f32, horizontal: bool, segments: usize) {
    let level = level.clamp(0.0, 1.0);
    let gap = 1.5;
    for i in 0..segments {
        let t = (i + 1) as f32 / segments as f32;
        let color = if t > 0.9 {
            RED
        } else if t > 0.72 {
            AMBER
        } else {
            GREEN
        };
        let (r, c) = if horizontal {
            let w = (rect.width() - gap * (segments - 1) as f32) / segments as f32;
            (
                Rect::from_min_size(
                    pos2(rect.left() + i as f32 * (w + gap), rect.top()),
                    vec2(w, rect.height()),
                ),
                color,
            )
        } else {
            let h = (rect.height() - gap * (segments - 1) as f32) / segments as f32;
            (
                Rect::from_min_size(
                    pos2(rect.left(), rect.bottom() - (i + 1) as f32 * h - i as f32 * gap),
                    vec2(rect.width(), h),
                ),
                color,
            )
        };
        let on = level >= t - 0.5 / segments as f32;
        p.rect_filled(r, CornerRadius::same(1), if on { c } else { alpha(c, 22) });
    }
}

pub fn small_dot(p: &Painter, pos: Pos2, radius: f32, color: Color32) {
    p.circle_filled(pos, radius + 2.5, alpha(color, 24));
    p.circle_filled(pos, radius, color);
}

pub fn keycap(p: &Painter, rect: Rect, text: &str) {
    p.rect_filled(rect, CornerRadius::same(4), PANEL2);
    p.rect_stroke(
        rect,
        CornerRadius::same(4),
        Stroke::new(1.0, LINE),
        StrokeKind::Inside,
    );
    mono(
        p,
        rect.center() + vec2(0.0, 0.5),
        text,
        8.5,
        MUTED,
        Align2::CENTER_CENTER,
    );
}

pub fn interact_rect(
    ui: &mut Ui,
    rect: Rect,
    sense: Sense,
    id: impl std::hash::Hash + std::fmt::Debug,
) -> Response {
    ui.interact(rect, ui.make_persistent_id(id), sense)
}

pub fn tooltip(resp: Response, text: &str) -> Response {
    resp.on_hover_text(text)
}

pub fn unused_id() -> Id {
    Id::new("stemforge-unused")
}
