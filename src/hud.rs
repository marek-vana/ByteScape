//! Original procedural HUD detailing inspired by cinematic information design.
//! All marks are decorative except explicitly labelled data; map geometry stays fixed.
use eframe::egui::*;
use std::f32::consts::{PI, TAU};
const CYAN: Color32 = Color32::from_rgb(64, 188, 224);
const DIM: Color32 = Color32::from_rgb(91, 120, 139);
fn ink(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}
pub fn micro(p: &Painter, pos: Pos2, label: impl ToString, c: Color32) {
    p.text(pos, Align2::LEFT_TOP, label, FontId::monospace(7.0), c);
}
pub fn brackets(p: &Painter, r: Rect, c: Color32, length: f32) {
    for (origin, dx, dy) in [
        (r.left_top(), 1.0, 1.0),
        (r.right_top(), -1.0, 1.0),
        (r.left_bottom(), 1.0, -1.0),
        (r.right_bottom(), -1.0, -1.0),
    ] {
        p.line_segment(
            [origin + vec2(dx * length, 0.0), origin],
            Stroke::new(1.0, c),
        );
        p.line_segment(
            [origin, origin + vec2(0.0, dy * length)],
            Stroke::new(1.0, c),
        );
        p.circle_filled(origin + vec2(dx * 3.0, dy * 3.0), 0.8, ink(c, 100));
    }
}
pub fn panel_texture(p: &Painter, r: Rect, right: bool, time: f32) {
    let p = p.with_clip_rect(r);
    // Fine etched vertical rails, with a phase travelling only along the rail.
    let x = if right {
        r.left() + 5.0
    } else {
        r.right() - 6.0
    };
    p.line_segment(
        [pos2(x, r.top() + 14.0), pos2(x, r.bottom() - 14.0)],
        Stroke::new(0.5, ink(CYAN, 24)),
    );
    for i in 0..((r.height() / 9.0) as usize) {
        let y = r.top() + 14.0 + i as f32 * 9.0;
        let len = if i % 5 == 0 { 4.0 } else { 1.5 };
        p.line_segment(
            [pos2(x, y), pos2(x + if right { len } else { -len }, y)],
            Stroke::new(0.5, ink(CYAN, if i % 5 == 0 { 75 } else { 35 })),
        );
    }
    if time > 0.0 {
        let y = r.top() + 20.0 + (time * 11.0) % (r.height() - 45.0).max(1.0);
        p.line_segment(
            [pos2(x, y), pos2(x, y + 14.0)],
            Stroke::new(1.0, ink(CYAN, 160)),
        );
    }
    let corner = Rect::from_min_size(pos2(r.right() - 58.0, r.top() + 7.0), vec2(40.0, 17.0));
    hatch(&p, corner, ink(CYAN, 32), 6.0);
    p.line_segment(
        [
            r.left_top() + vec2(12.0, 3.0),
            r.left_top() + vec2(38.0, 3.0),
        ],
        Stroke::new(1.0, ink(CYAN, 110)),
    );
}
pub fn hatch(p: &Painter, r: Rect, c: Color32, pitch: f32) {
    let p = p.with_clip_rect(r);
    let count = ((r.width() + r.height()) / pitch) as usize;
    for i in 0..=count {
        let x = r.left() - r.height() + i as f32 * pitch;
        p.line_segment(
            [pos2(x, r.bottom()), pos2(x + r.height(), r.top())],
            Stroke::new(0.5, c),
        );
    }
}
pub fn tile_material(p: &Painter, r: Rect, c: Color32, seed: usize) {
    let mut mesh = Mesh::default();
    for (point, alpha) in [
        (r.left_top(), 12),
        (r.right_top(), 23),
        (r.right_bottom(), 1),
        (r.left_bottom(), 3),
    ] {
        mesh.colored_vertex(point, ink(c, alpha));
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(Shape::mesh(mesh));
    if r.width() > 60.0 && r.height() > 40.0 {
        let corner = Rect::from_min_max(
            pos2(r.right() - 27.0, r.top() + 5.0),
            pos2(r.right() - 5.0, r.top() + 16.0),
        );
        hatch(p, corner, ink(c, 30), 4.0);
        // Short keyed index marks; they do not represent capacity or activity.
        for j in 0..5 {
            let xx = r.left() + 7.0 + j as f32 * 4.0;
            let h = if (seed >> j) & 1 == 1 { 4.0 } else { 2.0 };
            p.line_segment(
                [pos2(xx, r.bottom() - 6.0), pos2(xx, r.bottom() - 6.0 - h)],
                Stroke::new(0.7, ink(c, 70)),
            );
        }
        p.line_segment(
            [
                pos2(r.right() - 16.0, r.bottom() - 5.0),
                pos2(r.right() - 5.0, r.bottom() - 5.0),
            ],
            Stroke::new(0.6, ink(c, 75)),
        );
    }
}
pub fn dial(p: &Painter, center: Pos2, radius: f32, c: Color32, time: f32) {
    let width = 1.0;
    for i in 0..120 {
        let a = i as f32 / 120.0 * TAU - PI / 2.0;
        let v = vec2(a.cos(), a.sin());
        let major = i % 10 == 0;
        let len = if major { 6.0 } else { 2.5 };
        p.line_segment(
            [center + v * (radius - len), center + v * radius],
            Stroke::new(
                if major { 0.9 } else { 0.5 },
                ink(c, if major { 120 } else { 45 }),
            ),
        );
        if major {
            let pos = center + v * (radius + 10.0);
            p.text(
                pos,
                Align2::CENTER_CENTER,
                format!("{:03}", i * 3),
                FontId::monospace(6.0),
                ink(DIM, 155),
            );
        }
    }
    for j in 0..4 {
        let start = j as f32 * PI / 2.0 + 0.12 + time * 0.018;
        arc(
            p,
            center,
            radius + 20.0,
            start,
            start + 0.65,
            width,
            ink(c, 85),
        );
        arc(
            p,
            center,
            radius + 23.0,
            start + 0.06,
            start + 0.4,
            0.5,
            ink(c, 40),
        );
    }
    for direction in [
        vec2(1.0, 0.0),
        vec2(-1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(0.0, -1.0),
    ] {
        p.line_segment(
            [
                center + direction * (radius + 31.0),
                center + direction * (radius + 38.0),
            ],
            Stroke::new(0.5, ink(c, 85)),
        );
    }
}
pub fn arc(p: &Painter, center: Pos2, radius: f32, start: f32, end: f32, width: f32, c: Color32) {
    let count = (((end - start).abs() * radius / 3.0) as usize).clamp(4, 160);
    let points = (0..=count)
        .map(|i| {
            let a = start + (end - start) * i as f32 / count as f32;
            center + vec2(a.cos(), a.sin()) * radius
        })
        .collect();
    p.add(Shape::line(points, Stroke::new(width, c)));
}
pub fn data_bars(p: &Painter, r: Rect, values: &[u64], c: Color32) {
    let max = values.iter().copied().max().unwrap_or(1).max(1) as f64;
    let step = r.width() / values.len().max(1) as f32;
    for (i, &v) in values.iter().enumerate() {
        let h = (v as f64 / max) as f32 * r.height();
        p.rect_filled(
            Rect::from_min_max(
                pos2(r.left() + i as f32 * step, r.bottom() - h),
                pos2(r.left() + (i + 1) as f32 * step - 2.0, r.bottom()),
            ),
            0,
            ink(c, 90),
        );
    }
}
