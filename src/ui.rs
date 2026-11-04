use crate::{
    app::{ByteScape, Mode},
    data::Category,
    hud,
    layout::{self, Bounds},
    model::{self, Choice, Entry, Scope},
};
use eframe::egui::{
    text::{LayoutJob, TextWrapping},
    *,
};
use std::{
    f32::consts::{PI, TAU},
    sync::Arc,
};
pub(crate) const BG: Color32 = Color32::from_rgb(5, 10, 16);
const PANEL: Color32 = Color32::from_rgb(9, 18, 27);
const RAISED: Color32 = Color32::from_rgb(13, 27, 39);
const LINE: Color32 = Color32::from_rgb(36, 61, 78);
const TEXT: Color32 = Color32::from_rgb(216, 231, 241);
const MUTED: Color32 = Color32::from_rgb(145, 167, 184);
const CYAN: Color32 = Color32::from_rgb(80, 202, 230);
const AMBER: Color32 = Color32::from_rgb(235, 183, 99);
fn ink(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}
fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(w.max(0.0), h.max(0.0)))
}
fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}
fn condensed(size: f32) -> FontId {
    FontId::proportional(size)
}
fn elide(p: &Painter, r: Rect, label: &str, font: FontId, c: Color32, align: Align) {
    let mut job = LayoutJob::simple(label.to_owned(), font, c, r.width());
    job.wrap = TextWrapping {
        max_width: r.width(),
        max_rows: 1,
        break_anywhere: true,
        ..Default::default()
    };
    let galley = p.layout_job(job);
    let xx = match align {
        Align::Min => r.left(),
        Align::Center => r.center().x - galley.size().x * 0.5,
        Align::Max => r.right() - galley.size().x,
    };
    p.with_clip_rect(r)
        .galley(pos2(xx, r.center().y - galley.size().y * 0.5), galley, c);
}
fn label(ui: &mut Ui, r: Rect, value: impl Into<String>, font: FontId, c: Color32) -> Response {
    let value = value.into();
    ui.scope_builder(
        UiBuilder::new()
            .max_rect(r)
            .layout(Layout::left_to_right(Align::Center)),
        |ui| {
            ui.add(
                Label::new(RichText::new(&value).font(font).color(c))
                    .truncate()
                    .halign(Align::Min),
            )
        },
    )
    .inner
    .on_hover_text(value)
}
fn color(c: Category) -> Color32 {
    match c {
        Category::Projects => CYAN,
        Category::Media => Color32::from_rgb(134, 164, 239),
        Category::Apps => Color32::from_rgb(114, 210, 182),
        Category::Documents => Color32::from_rgb(191, 162, 230),
        Category::System => Color32::from_rgb(165, 186, 205),
        Category::Other => AMBER,
    }
}
fn panel(p: &Painter, r: Rect, fill: Color32, stroke: Color32) {
    let k = 7.0_f32.min(r.width() * 0.1).min(r.height() * 0.2);
    let points = vec![
        r.left_top() + vec2(k, 0.0),
        r.right_top(),
        r.right_bottom() - vec2(0.0, k),
        r.right_bottom() - vec2(k, 0.0),
        r.left_bottom(),
        r.left_top() + vec2(0.0, k),
    ];
    p.add(Shape::convex_polygon(
        points,
        fill,
        Stroke::new(1.0, stroke),
    ));
}
fn line(p: &Painter, a: Pos2, b: Pos2) {
    p.line_segment([a, b], Stroke::new(1.0, LINE));
}
fn action(ui: &mut Ui, r: Rect, name: &str, enabled: bool, active: bool) -> Response {
    let response = ui
        .add_enabled_ui(enabled, |ui| {
            ui.interact(
                r,
                ui.make_persistent_id(("action", r.min.x.to_bits(), r.min.y.to_bits())),
                Sense::click(),
            )
        })
        .inner;
    response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, active, name));
    let hot = response.hovered() || response.has_focus();
    let fill = if active {
        Color32::from_rgb(14, 49, 63)
    } else if hot {
        RAISED
    } else {
        PANEL
    };
    panel(
        ui.painter(),
        r,
        fill,
        if hot || active { ink(CYAN, 170) } else { LINE },
    );
    if active {
        ui.painter().line_segment(
            [
                r.left_bottom() + vec2(9.0, -2.0),
                r.right_bottom() - vec2(9.0, 2.0),
            ],
            Stroke::new(1.0, CYAN),
        );
    }
    elide(
        ui.painter(),
        r.shrink2(vec2(9.0, 4.0)),
        name,
        mono(12.0),
        if enabled {
            if active { TEXT } else { MUTED }
        } else if active {
            TEXT
        } else {
            ink(MUTED, 100)
        },
        Align::Center,
    );
    if response.has_focus() {
        hud::brackets(ui.painter(), r.expand(2.0), CYAN, 6.0);
    }
    response
        .on_hover_cursor(if enabled {
            CursorIcon::PointingHand
        } else {
            CursorIcon::Default
        })
        .on_hover_text(name)
}
fn reveal_focus(response: Response) -> Response {
    if response.gained_focus() {
        response.scroll_to_me_animation(None, style::ScrollAnimation::none());
    }
    response
}
#[derive(Clone, Copy)]
enum Glyph {
    Folder,
    File,
    Group,
    Map,
    Ring,
    Search,
    Layers,
}
fn glyph(p: &Painter, r: Rect, which: Glyph, c: Color32) {
    let p = p.with_clip_rect(r.expand(2.0));
    let m = r.center();
    let s = r.width().min(r.height()) * 0.38;
    let stroke = Stroke::new(1.2, c);
    match which {
        Glyph::Folder => {
            let v = vec![
                m + vec2(-s, -s * 0.6),
                m + vec2(-s * 0.2, -s * 0.6),
                m + vec2(s * 0.05, -s * 0.3),
                m + vec2(s, -s * 0.3),
                m + vec2(s, s * 0.65),
                m + vec2(-s, s * 0.65),
            ];
            p.add(Shape::closed_line(v, stroke));
        }
        Glyph::File => {
            let v = vec![
                m + vec2(-s * 0.65, -s),
                m + vec2(s * 0.2, -s),
                m + vec2(s * 0.65, -s * 0.5),
                m + vec2(s * 0.65, s),
                m + vec2(-s * 0.65, s),
            ];
            p.add(Shape::closed_line(v, stroke));
            p.line_segment(
                [m + vec2(-s * 0.3, s * 0.1), m + vec2(s * 0.3, s * 0.1)],
                stroke,
            );
        }
        Glyph::Group | Glyph::Layers => {
            for j in 0..3 {
                let y = (j as f32 - 1.0) * s * 0.45;
                let v = vec![
                    m + vec2(-s, y),
                    m + vec2(0.0, y - s * 0.38),
                    m + vec2(s, y),
                    m + vec2(0.0, y + s * 0.38),
                ];
                p.add(Shape::closed_line(v, stroke));
            }
        }
        Glyph::Ring => {
            p.circle_stroke(m, s, stroke);
            p.circle_stroke(m, s * 0.48, stroke);
            p.line_segment([m + vec2(0.0, -s), m + vec2(0.0, -s * 0.48)], stroke);
        }
        Glyph::Map => {
            p.rect_stroke(
                Rect::from_center_size(m, vec2(s * 2.0, s * 1.8)),
                1,
                stroke,
                StrokeKind::Inside,
            );
            p.line_segment(
                [m + vec2(-s * 0.2, -s * 0.9), m + vec2(-s * 0.2, s * 0.9)],
                stroke,
            );
            p.line_segment([m + vec2(-s * 0.2, 0.0), m + vec2(s, 0.0)], stroke);
        }
        Glyph::Search => {
            p.circle_stroke(m - vec2(s * 0.2, s * 0.2), s * 0.65, stroke);
            p.line_segment([m + vec2(s * 0.3, s * 0.3), m + vec2(s, s)], stroke);
        }
    }
}
fn choice_glyph(app: &ByteScape, key: Choice) -> Glyph {
    match key {
        Choice::Group(_) => Glyph::Group,
        Choice::Node(id) => {
            if app.data.nodes[id].kind != crate::data::NodeKind::Directory {
                Glyph::File
            } else {
                Glyph::Folder
            }
        }
    }
}
pub(crate) fn configure(ctx: &Context) {
    let mut s = (*ctx.global_style()).clone();
    s.visuals = Visuals::dark();
    s.visuals.panel_fill = BG;
    s.visuals.window_fill = PANEL;
    s.visuals.override_text_color = Some(TEXT);
    s.visuals.weak_text_color = Some(MUTED);
    s.visuals.selection.bg_fill = Color32::from_rgb(16, 55, 70);
    s.visuals.selection.stroke = Stroke::new(1.0, CYAN);
    s.visuals.widgets.noninteractive.bg_fill = PANEL;
    s.visuals.widgets.noninteractive.weak_bg_fill = PANEL;
    s.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    s.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    s.visuals.widgets.inactive.bg_fill = PANEL;
    s.visuals.widgets.inactive.weak_bg_fill = PANEL;
    s.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
    s.visuals.widgets.hovered.bg_fill = RAISED;
    s.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, CYAN);
    s.visuals.widgets.active.bg_fill = RAISED;
    s.visuals.widgets.active.bg_stroke = Stroke::new(1.0, CYAN);
    s.spacing.button_padding = vec2(10.0, 7.0);
    s.spacing.item_spacing = vec2(8.0, 10.0);
    s.spacing.interact_size.y = 30.0;
    s.text_styles.insert(TextStyle::Body, mono(12.0));
    s.text_styles.insert(TextStyle::Button, mono(12.0));
    s.text_styles.insert(TextStyle::Small, mono(11.0));
    ctx.set_global_style(s);
}
impl ByteScape {
    pub(crate) fn draw(&mut self, ui: &mut Ui) {
        self.update_announcement();
        let reduce_motion = self.motion_off();
        ui.ctx().global_style_mut(|style| {
            style.animation_time = if reduce_motion { 0.0 } else { 0.15 };
            style.scroll_animation = if reduce_motion {
                style::ScrollAnimation::none()
            } else {
                style::ScrollAnimation::default()
            };
        });
        self.nav_focus.set(false);
        let r = ui.max_rect();
        let p = ui.painter().clone();
        p.rect_filled(r, 0, BG);
        let h = 64.0;
        let header = rect(r.left(), r.top(), r.width(), h);
        let foot = rect(r.left(), r.bottom() - 28.0, r.width(), 28.0);
        let body = Rect::from_min_max(header.left_bottom(), foot.right_top());
        let sidebar_width = if r.width() >= 1260.0 || self.expand_sidebar {
            190.0
        } else {
            62.0
        };
        let dock = self.detail_open && r.width() >= 1180.0;
        let detail_width = if dock { 280.0 } else { 0.0 };
        let side = body.with_max_x(body.left() + sidebar_width);
        let detail = body.with_min_x(body.right() - detail_width);
        let main = Rect::from_min_max(side.right_top(), pos2(detail.left(), body.bottom()));
        self.top_bar(ui, header, dock);
        self.side_bar(ui, side);
        self.workspace(ui, main);
        if dock {
            self.inspector(ui, detail);
        }
        if self.detail_popup {
            let mut open = true;
            let ctx = ui.ctx().clone();
            Window::new(self.tr("Detail výběru", "Selection details"))
                .id(Id::unique("detail-popup"))
                .open(&mut open)
                .resizable(false)
                .fixed_size(vec2(
                    (r.width() - 60.0).min(330.0),
                    (r.height() - 130.0).max(180.0),
                ))
                .constrain_to(r.shrink(12.0))
                .anchor(Align2::RIGHT_TOP, vec2(-16.0, 76.0))
                .frame(
                    Frame::new()
                        .fill(PANEL)
                        .stroke(Stroke::new(1.0, LINE))
                        .inner_margin(14.0)
                        .corner_radius(5),
                )
                .max_height((r.height() - 120.0).max(200.0))
                .show(&ctx, |ui| {
                    ScrollArea::vertical()
                        .id_salt("detail-overlay-scroll")
                        .max_height((r.height() - 180.0).max(180.0))
                        .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysVisible)
                        .show(ui, |ui| self.inspector_content(ui));
                });
            self.detail_popup = open;
        }
        p.rect_filled(foot, 0, PANEL);
        line(&p, foot.left_top(), foot.right_top());
        let status_rect = rect(foot.left() + 16.0, foot.top() + 3.0, 250.0, 22.0);
        let status = ui.interact(status_rect, Id::unique("scan-live-status"), Sense::hover());
        status.widget_info(|| WidgetInfo::labeled(Role::Label, true, self.source_status()));
        elide(
            ui.painter(),
            status_rect,
            self.source_status(),
            mono(10.5),
            AMBER,
            Align::Min,
        );
        ui.ctx().accesskit_node_builder(status.id, |node| {
            node.set_role(accesskit::Role::Status);
            node.set_live(accesskit::Live::Polite);
            node.set_live_atomic();
            node.set_label(self.announcement.clone());
        });
        if let Some(scan) = &self.scan {
            let count = if self
                .scan_status
                .as_ref()
                .is_some_and(|e| self.data.run_id == Some(e.run_id))
            {
                self.data.nodes.len().saturating_sub(1)
            } else {
                0
            };
            label(
                ui,
                rect(
                    foot.left() + 285.0,
                    foot.top() + 3.0,
                    (r.width() - 485.0).min(420.0),
                    22.0,
                ),
                format!(
                    "{}: {} · {:.1} s",
                    self.tr("Položky", "Items"),
                    count,
                    scan.elapsed().as_secs_f64()
                ),
                mono(10.5),
                AMBER,
            );
        } else if r.width() > 920.0 {
            label(
                ui,
                rect(
                    foot.left() + 285.0,
                    foot.top() + 3.0,
                    (r.width() - 485.0).min(420.0),
                    22.0,
                ),
                self.tr(
                    "* známý mezisoučet · ? neznámé · problémy v Detailu",
                    "* known subtotal · ? unknown · problems in Details",
                ),
                mono(10.5),
                MUTED,
            );
        }
        label(
            ui,
            rect(foot.right() - 180.0, foot.top() + 3.0, 165.0, 22.0),
            self.tr("Data zůstávají lokálně", "Your data stays local"),
            mono(10.5),
            MUTED,
        );
        self.keyboard(ui);
    }
    fn top_bar(&mut self, ui: &mut Ui, r: Rect, dock: bool) {
        let p = ui.painter().clone();
        p.rect_filled(r, 0, PANEL);
        line(&p, r.left_bottom(), r.right_bottom());
        glyph(
            &p,
            rect(r.left() + 20.0, r.top() + 20.0, 30.0, 30.0),
            Glyph::Layers,
            CYAN,
        );
        if r.width() >= 900.0 {
            label(
                ui,
                rect(r.left() + 60.0, r.top() + 10.0, 160.0, 37.0),
                "BYTESCAPE",
                condensed(30.0),
                TEXT,
            );
            hud::micro(
                &p,
                pos2(r.left() + 62.0, r.top() + 48.0),
                "STORAGE / CARTOGRAPHY",
                ink(CYAN, 180),
            );
        }
        if r.width() > 1120.0 {
            let x = r.left() + 240.0;
            label(
                ui,
                rect(x, r.top() + 17.0, 210.0, 22.0),
                self.tr("PROSTOR POD KONTROLOU", "SEE YOUR STORAGE CLEARLY"),
                mono(11.0),
                MUTED,
            );
            hud::micro(
                &p,
                pos2(x, r.top() + 44.0),
                if self.data.run_id.is_some() {
                    self.tr("LOKÁLNÍ ANALÝZA / SLOŽKA", "LOCAL ANALYSIS / FOLDER")
                } else {
                    self.tr("LOKÁLNÍ ANALÝZA / UKÁZKA", "LOCAL ANALYSIS / DEMO")
                },
                ink(CYAN, 150),
            );
        }
        let lang = if self.cs { "CZ / EN" } else { "EN / CZ" };
        let language = action(
            ui,
            rect(r.right() - 94.0, r.top() + 19.0, 76.0, 32.0),
            lang,
            true,
            false,
        );
        language.widget_info(|| {
            WidgetInfo::labeled(
                Role::Button,
                language.enabled(),
                self.tr("Přepnout do angličtiny", "Switch to Czech"),
            )
        });
        if language.clicked() {
            self.cs = !self.cs;
        }
        if action(
            ui,
            rect(r.right() - 186.0, r.top() + 19.0, 82.0, 32.0),
            self.tr("Detail", "Details"),
            true,
            dock || self.detail_popup,
        )
        .clicked()
        {
            if r.width() >= 1180.0 {
                self.detail_open = !self.detail_open;
            } else {
                self.detail_popup = !self.detail_popup;
                self.focus_details = self.detail_popup;
            }
        }
        let zoom_in = action(
            ui,
            rect(r.right() - 232.0, r.top() + 19.0, 36.0, 32.0),
            "A+",
            self.zoom < 1.4,
            false,
        );
        zoom_in.widget_info(|| {
            WidgetInfo::labeled(
                Role::Button,
                zoom_in.enabled(),
                self.tr("Zvětšit rozhraní", "Zoom in"),
            )
        });
        if zoom_in.clicked() {
            self.zoom = (self.zoom + 0.1).min(1.4);
            ui.ctx().set_zoom_factor(self.zoom);
        }
        let zoom_out = action(
            ui,
            rect(r.right() - 274.0, r.top() + 19.0, 36.0, 32.0),
            "A−",
            self.zoom > 0.9,
            false,
        );
        zoom_out.widget_info(|| {
            WidgetInfo::labeled(
                Role::Button,
                zoom_out.enabled(),
                self.tr("Zmenšit rozhraní", "Zoom out"),
            )
        });
        if zoom_out.clicked() {
            self.zoom = (self.zoom - 0.1).max(0.9);
            ui.ctx().set_zoom_factor(self.zoom);
        }
        if action(
            ui,
            rect(r.right() - 432.0, r.top() + 19.0, 140.0, 32.0),
            self.tr("Vybrat složku…", "Choose folder…"),
            self.scan.is_none(),
            false,
        )
        .clicked()
            && let Some(root) = crate::platform::choose_folder(self.cs)
        {
            self.start_scan(root);
        }
        if self.scan.is_none() && self.scan_root.is_some() {
            if action(
                ui,
                rect(r.right() - 580.0, r.top() + 19.0, 140.0, 32.0),
                self.tr("Skenovat znovu", "Scan again"),
                true,
                false,
            )
            .clicked()
            {
                self.start_scan(self.scan_root.clone().unwrap());
            }
        } else if let Some(scan) = &self.scan {
            let cancelled = self
                .scan_status
                .as_ref()
                .is_some_and(|e| e.state == crate::scan::State::Cancelled);
            let label = if cancelled {
                self.tr("Dočišťování…", "Finishing…")
            } else {
                self.tr("Zrušit sken", "Cancel scan")
            };
            if action(
                ui,
                rect(r.right() - 580.0, r.top() + 19.0, 140.0, 32.0),
                label,
                !cancelled,
                false,
            )
            .clicked()
            {
                scan.cancel();
            }
        }
    }
    fn source_status(&self) -> &str {
        if self.revision_waiting() {
            if self
                .scan_status
                .as_ref()
                .is_some_and(|e| e.state == crate::scan::State::Cancelled)
            {
                return self.tr(
                    "SKEN ZRUŠEN · prohlížíte předchozí revizi",
                    "SCAN CANCELLED · viewing previous revision",
                );
            }
            return self.tr(
                "AKTUALIZACE ČEKÁ · prohlížíte předchozí revizi",
                "UPDATE WAITING · viewing previous revision",
            );
        }
        if self.scan.is_some() && self.scan_status.is_none() {
            return self.tr(
                "SKENOVÁNÍ · čekání na metadata",
                "SCANNING · awaiting metadata",
            );
        }
        if let Some(event) = &self.scan_status {
            return self.scan_state_text(event.state);
        }
        self.tr("UKÁZKA · syntetická data", "DEMO · synthetic data")
    }

    pub(crate) fn scan_state_text(&self, state: crate::scan::State) -> &str {
        match state {
            crate::scan::State::Running => {
                self.tr("SKENOVÁNÍ · průběžný výsledek", "SCANNING · partial result")
            }
            crate::scan::State::Failed => {
                self.tr("SKEN SELHAL · dosavadní mapa", "SCAN FAILED · previous map")
            }
            crate::scan::State::Unavailable => self.tr(
                "ZDROJ NEDOSTUPNÝ · zachovaný výsledek",
                "SOURCE UNAVAILABLE · retained result",
            ),
            crate::scan::State::Limited => {
                self.tr("SLOŽKA · omezený výsledek", "FOLDER · limited result")
            }
            crate::scan::State::Cancelled => self.tr("SLOŽKA · zrušeno", "FOLDER · cancelled"),
            crate::scan::State::Complete => self.tr("SLOŽKA · dokončeno", "FOLDER · complete"),
        }
    }
    fn side_bar(&mut self, ui: &mut Ui, r: Rect) {
        let p = ui.painter().clone();
        p.rect_filled(r, 0, PANEL);
        line(&p, r.right_top(), r.right_bottom());
        hud::panel_texture(&p, r, false, self.hud_time());
        let compact = r.width() < 100.0;
        if action(
            ui,
            rect(
                r.left() + 14.0,
                r.top() + 16.0,
                if compact { 34.0 } else { r.width() - 28.0 },
                32.0,
            ),
            if compact {
                "☰"
            } else {
                self.tr("Přehled disku", "Disk overview")
            },
            true,
            false,
        )
        .clicked()
        {
            self.expand_sidebar = !self.expand_sidebar;
        }
        if !compact {
            label(
                ui,
                rect(r.left() + 18.0, r.top() + 68.0, r.width() - 36.0, 30.0),
                &self.data.nodes[0].name,
                condensed(24.0),
                TEXT,
            );
            label(
                ui,
                rect(r.left() + 18.0, r.top() + 104.0, r.width() - 36.0, 23.0),
                self.data.nodes[0]
                    .coverage
                    .format(self.data.nodes[0].size(self.allocated)),
                mono(19.0),
                CYAN,
            );
            label(
                ui,
                rect(r.left() + 18.0, r.top() + 132.0, r.width() - 36.0, 23.0),
                format!(
                    "{} {}",
                    model::count(self.data.files),
                    if self.data.run_id.is_some() {
                        self.tr("souborů · složka", "files · folder")
                    } else {
                        self.tr("souborů · ukázka", "files · demo")
                    }
                ),
                mono(11.0),
                MUTED,
            );
            let bar = rect(r.left() + 18.0, r.top() + 174.0, r.width() - 36.0, 4.0);
            let mut x = bar.left();
            for &id in &self.data.nodes[0].children {
                let n = &self.data.nodes[id];
                let w = bar.width() * n.size(self.allocated) as f32
                    / self.data.nodes[0].size(self.allocated).max(1) as f32;
                p.rect_filled(rect(x, bar.top(), w, 4.0), 0, color(n.category));
                x += w;
            }
            label(
                ui,
                rect(r.left() + 18.0, r.top() + 205.0, r.width() - 36.0, 22.0),
                self.tr("TYP OBSAHU", "CONTENT TYPE"),
                mono(11.0),
                MUTED,
            );
        }
        let start = if compact {
            r.top() + 75.0
        } else {
            r.top() + 240.0
        };
        for (i, cat) in Category::ALL.iter().enumerate() {
            let y = start + i as f32 * 39.0;
            let active = self.category == Some(*cat);
            let rr = rect(r.left() + 12.0, y, r.width() - 24.0, 32.0);
            let response = if compact {
                action(
                    ui,
                    rr,
                    match cat {
                        Category::Projects => "PR",
                        Category::Media => "AV",
                        Category::Apps => "AP",
                        Category::Documents => "DO",
                        Category::System => "SY",
                        Category::Other => "OT",
                    },
                    true,
                    active,
                )
                .on_hover_text(cat.label(self.cs))
            } else {
                action(ui, rr, cat.label(self.cs), true, active)
            };
            response.widget_info(|| {
                WidgetInfo::selected(Role::Button, true, active, cat.label(self.cs))
            });
            p.rect_filled(
                rect(rr.right() - 3.0, rr.top() + 10.0, 2.0, 12.0),
                0,
                color(*cat),
            );
            if response.clicked() {
                self.category = if active { None } else { Some(*cat) };
            }
        }
        if self.category.is_some()
            && !compact
            && action(
                ui,
                rect(r.left() + 16.0, start + 240.0, r.width() - 32.0, 28.0),
                self.tr("Zrušit filtr", "Clear filter"),
                true,
                false,
            )
            .clicked()
        {
            self.category = None;
        }
        let bottom = r.bottom() - 94.0;
        if !compact && bottom > start + 242.0 {
            line(
                &p,
                pos2(r.left() + 18.0, bottom - 12.0),
                pos2(r.right() - 18.0, bottom - 12.0),
            );
            let system = self.preferences.reduce_motion;
            ui.add_enabled_ui(!system, |ui| {
                ui.put(
                    rect(r.left() + 18.0, bottom, r.width() - 36.0, 28.0),
                    Checkbox::new(
                        &mut self.reduced,
                        if self.cs {
                            "Omezit pohyb"
                        } else {
                            "Reduce motion"
                        },
                    ),
                )
            })
            .inner
            .on_hover_text(self.tr(
                "Systémové omezení pohybu má přednost.",
                "System Reduce Motion takes precedence.",
            ));
            let off = self.motion_off();
            ui.add_enabled_ui(!off, |ui| {
                ui.put(
                    rect(r.left() + 18.0, bottom + 34.0, r.width() - 36.0, 28.0),
                    Checkbox::new(
                        &mut self.ambient,
                        if self.cs {
                            "Jemné efekty"
                        } else {
                            "Ambient effects"
                        },
                    ),
                )
            });
        }
        if compact {
            let mut reduced = self.reduced;
            if action(
                ui,
                rect(r.left() + 12.0, r.bottom() - 90.0, r.width() - 24.0, 32.0),
                "◌",
                !self.preferences.reduce_motion,
                self.motion_off(),
            )
            .on_hover_text(self.tr("Omezit pohyb", "Reduce motion"))
            .clicked()
            {
                reduced = !reduced;
                self.reduced = reduced;
            }
        }
        hud::micro(
            &p,
            pos2(
                r.left() + if compact { 17.0 } else { 19.0 },
                r.bottom() - 20.0,
            ),
            if compact {
                "B / 01"
            } else {
                "ORCHESTRA / DESIGN DNA"
            },
            ink(CYAN, 140),
        );
    }
    fn inspector(&mut self, ui: &mut Ui, r: Rect) {
        let p = ui.painter().clone();
        p.rect_filled(r, 0, PANEL);
        line(&p, r.left_top(), r.left_bottom());
        hud::panel_texture(&p, r, true, self.hud_time());
        let inside = r.shrink2(vec2(20.0, 18.0));
        let mut child = ui.new_child(UiBuilder::new().max_rect(inside).id_salt("inspector"));
        child.set_clip_rect(inside);
        ScrollArea::vertical()
            .id_salt("inspector-scroll")
            .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(&mut child, |ui| self.inspector_content(ui));
    }
    fn inspector_content(&mut self, ui: &mut Ui) {
        ui.spacing_mut().item_spacing.y = 6.0;
        let entry = self.info(self.selected);
        let c = color(entry.category);
        ui.label(
            RichText::new(self.tr("DETAIL VÝBĚRU", "SELECTION DETAILS"))
                .font(mono(11.0))
                .color(MUTED),
        );
        ui.add_space(12.0);
        let (rr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 44.0), Sense::hover());
        panel(
            ui.painter(),
            rect(rr.left(), rr.top(), 42.0, 42.0),
            RAISED,
            ink(c, 110),
        );
        glyph(
            ui.painter(),
            rect(rr.left() + 8.0, rr.top() + 8.0, 26.0, 26.0),
            choice_glyph(self, self.selected),
            c,
        );
        label(
            ui,
            rect(rr.left() + 55.0, rr.top() + 3.0, rr.width() - 55.0, 22.0),
            match self.selected {
                Choice::Group(_) => self.tr("SKUPINA POLOŽEK", "ITEM GROUP"),
                Choice::Node(id) => match self.data.nodes[id].kind {
                    crate::data::NodeKind::Directory if self.data.nodes[id].is_package => {
                        self.tr("BALÍČEK", "PACKAGE")
                    }
                    crate::data::NodeKind::Directory => self.tr("SLOŽKA", "FOLDER"),
                    crate::data::NodeKind::File => self.tr("SOUBOR", "FILE"),
                    crate::data::NodeKind::Symlink => self.tr("SYMBOLICKÝ ODKAZ", "SYMBOLIC LINK"),
                    crate::data::NodeKind::Special => self.tr("ZVLÁŠTNÍ OBJEKT", "SPECIAL OBJECT"),
                    crate::data::NodeKind::Unknown => self.tr("NEZNÁMÝ DRUH", "UNKNOWN KIND"),
                },
            },
            mono(11.0),
            MUTED,
        );
        hud::micro(
            ui.painter(),
            pos2(rr.left() + 55.0, rr.top() + 28.0),
            match self.selected {
                Choice::Node(id) if self.data.nodes[id].cloud != crate::data::CloudState::Local => {
                    self.tr("VÝBĚR / CLOUDOVÁ METADATA", "SELECTED / CLOUD METADATA")
                }
                _ => self.tr("VÝBĚR / LOKÁLNÍ", "SELECTED / LOCAL"),
            },
            ink(c, 160),
        );
        ui.add_space(12.0);
        ui.add(Label::new(RichText::new(&entry.name).font(condensed(27.0)).color(TEXT)).truncate())
            .on_hover_text(&entry.name);
        let parent = match self.selected {
            Choice::Node(id) => id,
            Choice::Group(s) => s.root,
        };
        let path = self.data.display_path(parent);
        ui.add(Label::new(RichText::new(&path).font(mono(11.0)).color(MUTED)).truncate())
            .on_hover_text(&path);
        let copy_path = ui.add_enabled(
            self.data.copy_path(parent).is_some(),
            Button::new(
                RichText::new(self.tr("Kopírovat celou cestu", "Copy full path")).font(mono(11.0)),
            )
            .fill(RAISED),
        );
        if self.focus_details {
            copy_path.request_focus();
            self.focus_details = false;
        }
        if reveal_focus(copy_path).clicked()
            && let Some(path) = self.data.copy_path(parent)
        {
            ui.ctx().copy_text(path);
        }
        if self.data.copy_path(parent).is_none() {
            ui.label(self.tr(
                "Cestu nelze bezeztrátově kopírovat jako text.",
                "This native path cannot be copied losslessly as text.",
            ));
        }
        if self.data.nodes[parent].coverage != crate::data::Coverage::Complete {
            ui.colored_label(
                AMBER,
                self.tr(
                    "* Omezené údaje / součet podle cest; neznámé části nejsou nula.",
                    "* Limited metadata / sum per path; unknown parts are not zero.",
                ),
            );
        }
        ui.add_space(10.0);
        let target = match self.selected {
            Choice::Group(s) => Some(s),
            Choice::Node(id) => {
                if self.data.nodes[id].kind != crate::data::NodeKind::Directory {
                    self.data.nodes[id].parent.map(Scope::folder)
                } else {
                    Some(Scope::folder(id))
                }
            }
        };
        if let Some(target) = target
            && target != self.scope
        {
            if ui
                .add_sized(
                    [ui.available_width(), 36.0],
                    Button::new(
                        RichText::new(self.tr("Prohlédnout položky  →", "Explore items  →"))
                            .font(mono(12.0)),
                    )
                    .fill(Color32::from_rgb(13, 47, 60)),
                )
                .clicked()
            {
                self.open_scope(target);
            }
        } else {
            ui.label(
                RichText::new(self.tr("Zobrazeno v aktuální mapě", "Shown in the current map"))
                    .font(mono(11.0))
                    .color(MUTED),
            );
        }
        ui.add_space(18.0);
        ui.label(
            RichText::new(entry.coverage.format(entry.size(self.allocated)))
                .font(condensed(39.0))
                .color(c),
        );
        let scope = model::scope_entry(&self.data, self.scope, self.cs);
        let share = entry.size(self.allocated) as f64 / scope.size(self.allocated).max(1) as f64;
        if entry.coverage != crate::data::Coverage::Unknown && share <= 1.00001 {
            ui.label(
                RichText::new(format!(
                    "{:.1}% {}",
                    share * 100.0,
                    self.tr("aktuálního rozsahu", "of current scope")
                ))
                .font(mono(12.0))
                .color(MUTED),
            );
        }
        ui.add_space(10.0);
        ui.separator();
        let reported = match self.selected {
            Choice::Node(id) => self.data.nodes[id].reported_size,
            Choice::Group(_) => None,
        }
        .unwrap_or((entry.logical, entry.allocated));
        if let Choice::Node(id) = self.selected {
            let node = &self.data.nodes[id];
            if node.is_package {
                ui.label(self.tr(
                    "Balíček: souhrn skutečných potomků. Obsah byl skenován i bez rozbalení; otevřete jej jako složku.",
                    "Package: total of real children. Contents were scanned even while collapsed; open it as a folder.",
                ));
            }
            let cloud_note = match node.cloud {
                crate::data::CloudState::Local => None,
                crate::data::CloudState::Cached => Some(self.tr(
                    "Cloudová větev: pouze dostupná metadata, bez požadavku na stažení.",
                    "Cloud branch: available metadata only, no download requested.",
                )),
                crate::data::CloudState::Dataless => Some(self.tr(
                    "Cloud: obsah není místně dostupný. Zobrazená metadata nejsou důkazem staženého obsahu; neznámý vnitřek složky se nezapočítává jako nula.",
                    "Cloud: contents are not locally available. Displayed metadata does not imply downloaded contents; unknown folder contents are not counted as zero.",
                )),
                crate::data::CloudState::LegacyPlaceholder => Some(self.tr(
                    "Cloudový zástupce .icloud: velikost vzdálené položky je neznámá. Zástupný soubor nebyl čten.",
                    "Cloud .icloud placeholder: remote item size is unknown. The stub was not read.",
                )),
            };
            if let Some(note) = cloud_note {
                ui.label(RichText::new(note).color(CYAN));
            }
            if let Some(owner) = node.hard_link_owner {
                ui.label(
                    RichText::new(self.tr("Další pevný odkaz", "Additional hard link")).color(CYAN),
                );
                ui.label(self.tr(
                    "Metadata patří témuž souboru. Do součtů a map přispívá tato cesta 0 B; započteno pod:",
                    "Metadata describes the same file. This path contributes 0 B to totals and maps; counted under:",
                ));
                ui.label(self.data.display_path(owner));
            } else if node.kind == crate::data::NodeKind::Symlink {
                ui.label(self.tr(
                    "Symbolický odkaz: pouze vlastní metadata, cíl se neskenuje.",
                    "Symbolic link: own metadata only; target is not scanned.",
                ));
            }
        }
        for (key, value) in [
            (
                self.tr("Velikost obsahu", "Logical size"),
                entry.coverage.format(reported.0),
            ),
            (
                self.tr("Přidělené místo", "Allocated size"),
                entry.coverage.format(reported.1),
            ),
            (
                self.tr("Přímé položky", "Direct items"),
                model::count(entry.count),
            ),
            (
                self.tr("Kategorie", "Category"),
                entry.category.label(self.cs).into(),
            ),
        ] {
            ui.add_space(6.0);
            ui.label(RichText::new(key).font(mono(11.0)).color(MUTED));
            ui.label(RichText::new(value).font(mono(13.0)).color(TEXT))
                .on_hover_text(if entry.coverage == crate::data::Coverage::Unknown {
                    self.tr("Neznámé údaje", "Unknown metadata").to_owned()
                } else {
                    match key {
                        k if k == self.tr("Velikost obsahu", "Logical size") => {
                            format!("{} B", model::count(reported.0 as usize))
                        }
                        k if k == self.tr("Přidělené místo", "Allocated size") => {
                            format!("{} B", model::count(reported.1 as usize))
                        }
                        _ => String::new(),
                    }
                });
        }
        let chart_members = match self.selected {
            Choice::Node(id) => Arc::clone(&self.data.children(id, true)),
            Choice::Group(s) => model::members(&self.data, s),
        };
        if !chart_members.is_empty() {
            ui.add_space(10.0);
            ui.label(
                RichText::new(self.tr("Relativní velikosti položek", "Relative item sizes"))
                    .font(mono(11.0))
                    .color(MUTED),
            );
            let (chart, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), 25.0), Sense::hover());
            let values: Vec<_> = chart_members
                .iter()
                .take(24)
                .map(|&i| self.data.nodes[i].size(self.allocated))
                .collect();
            hud::data_bars(ui.painter(), chart, &values, c);
        }
        ui.add_space(16.0);
        ui.add_space(20.0);
        Frame::new()
            .fill(RAISED)
            .inner_margin(12.0)
            .corner_radius(4)
            .show(ui, |ui| {
                ui.label(
                    RichText::new(self.source_status())
                        .font(mono(11.0))
                        .color(AMBER),
                );
                ui.add_space(5.0);
                ui.label(
                    RichText::new(if self.data.run_id.is_some() { self.tr(
                        "Metadata vybrané složky, nikoli atomický snapshot ani systémové obsazení disku. Přidělené místo není příslib uvolněného místa. Odkazy se nenásledují. Tentýž soubor přes pevné odkazy přispívá jen jednou; APFS úsporu nelze odvodit. Kategorie jsou orientační.",
                        "Selected folder metadata, not an atomic snapshot or system disk usage. Allocated size does not promise reclaimable space. Links are not followed. Files reached through hard links contribute only once; APFS reclaimable space is not implied. Categories are approximate.")
                    } else { self.tr("Grafický prototyp. Údaje jsou syntetické.", "Graphics prototype. Data is synthetic.") })
                    .font(mono(11.0))
                    .color(MUTED),
                );
            });
        if let Some(event) = &self.scan_status {
            ui.label(self.tr("Stav posledního skenu", "Latest scan status"));
            ui.label(self.source_status());
            ui.label(format!(
                "{}: {} · {}: {}",
                self.tr("Položky", "Items"),
                if self.data.run_id == Some(event.run_id) {
                    self.data.nodes.len().saturating_sub(1)
                } else {
                    0
                },
                self.tr("Problémy", "Problems"),
                event.problems.len()
            ));
            if let Some(scan) = &self.scan {
                ui.label(format!(
                    "{}: {:.1} s",
                    self.tr("Uplynulo", "Elapsed"),
                    scan.elapsed().as_secs_f64()
                ));
            }
            let mut next_source = None;
            for (index, problem) in event.problems.iter().enumerate() {
                let title = format!(
                    "{} {}: {}",
                    self.tr("Problém", "Problem"),
                    index + 1,
                    problem.path.to_string_lossy()
                );
                let mut job = LayoutJob::simple(
                    title,
                    FontId::proportional(14.0),
                    TEXT,
                    (ui.available_width() - 36.0).max(20.0),
                );
                job.wrap.max_rows = 1;
                let galley = ui.painter().layout_job(job);
                let problem_view = CollapsingHeader::new(WidgetText::Galley(galley))
                    .id_salt(("scan-problem", event.run_id, index))
                    .show(ui, |ui| {
                        ui.add(Label::new(problem.path.to_string_lossy()).wrap());
                        ui.label(problem.description(self.cs));
                        if let Some(detail) = problem.system_detail() {
                            ui.add(
                                Label::new(format!(
                                    "{}: {}",
                                    self.tr("Systémový detail", "System detail"),
                                    detail
                                ))
                                .wrap(),
                            );
                        }
                        if let Some(advice) = problem.advice(self.cs) {
                            ui.label(advice);
                        }
                        if problem.volume_boundary
                            && ui
                                .add_enabled(
                                    self.scan.is_none(),
                                    Button::new(self.tr(
                                        "Skenovat jako samostatný zdroj",
                                        "Scan as a separate source",
                                    )),
                                )
                                .clicked()
                        {
                            next_source = Some(problem.path.clone());
                        }
                        let path = problem.path.to_str();
                        if reveal_focus(ui.add_enabled(
                            path.is_some(),
                            Button::new(self.tr("Kopírovat cestu problému", "Copy problem path")),
                        ))
                        .clicked()
                            && let Some(path) = path
                        {
                            ui.ctx().copy_text(path.to_owned());
                        }
                        if path.is_none() {
                            ui.label(self.tr(
                                "Cestu nelze bezeztrátově kopírovat jako text.",
                                "This native path cannot be copied losslessly as text.",
                            ));
                        }
                    });
                reveal_focus(problem_view.header_response);
                ui.separator();
            }
            if let Some(root) = next_source {
                self.start_scan(root);
            }
        }
        ui.collapsing(self.tr("Klávesové zkratky", "Keyboard shortcuts"), |ui| {
            ui.label(self.tr("⌘O Vybrat a spustit · ⌘R Skenovat znovu · ⌘. Zrušit sken · ⌘I Detail a problémy",
                "⌘O Choose and scan · ⌘R Scan again · ⌘. Cancel scan · ⌘I Details and problems"));
            ui.label(self.tr("Tab / Shift+Tab: ovládání · Enter / mezerník: aktivovat · šipky: položky · Backspace: nadřazená složka. Problémy rozbalíte klávesou Enter.",
                "Tab / Shift+Tab: controls · Enter / Space: activate · arrows: items · Backspace: parent folder. Press Enter to expand a problem."));
        });
        if self.preferences.reduce_motion {
            ui.label(
                RichText::new(self.tr(
                    "Pohyb omezen nastavením macOS.",
                    "Motion reduced by macOS settings.",
                ))
                .font(mono(11.0))
                .color(MUTED),
            );
        }
        if self.preferences.screen_reader {
            ui.label(
                RichText::new(self.tr(
                    "VoiceOver: mapy mají ekvivalentní seznam položek.",
                    "VoiceOver: maps provide an equivalent item list.",
                ))
                .font(mono(11.0))
                .color(MUTED),
            );
        }
    }
    fn workspace(&mut self, ui: &mut Ui, r: Rect) {
        let x = r.left() + 22.0;
        let w = r.width() - 44.0;
        let p = ui.painter().clone();
        let y = r.top() + 14.0;
        let bh = 30.0;
        let back = action(ui, rect(x, y, 32.0, bh), "←", !self.back.is_empty(), false);
        back.widget_info(|| {
            WidgetInfo::labeled(Role::Button, !self.back.is_empty(), self.tr("Zpět", "Back"))
        });
        if back.on_hover_text(self.tr("Zpět", "Back")).clicked()
            && let Some(scope) = self.back.pop()
        {
            self.forward.push(self.scope);
            self.change_scope(scope);
        }
        let forward = action(
            ui,
            rect(x + 39.0, y, 32.0, bh),
            "→",
            !self.forward.is_empty(),
            false,
        );
        forward.widget_info(|| {
            WidgetInfo::labeled(
                Role::Button,
                !self.forward.is_empty(),
                self.tr("Vpřed", "Forward"),
            )
        });
        if forward.on_hover_text(self.tr("Vpřed", "Forward")).clicked()
            && let Some(scope) = self.forward.pop()
        {
            self.back.push(self.scope);
            self.change_scope(scope);
        }
        let path = self.data.path(self.scope.root);
        let visible = if path.len() > 2 {
            path[path.len() - 2..].to_vec()
        } else {
            path
        };
        let available = (w - 90.0) / visible.len().max(1) as f32;
        let mut bx = x + 86.0;
        for id in visible {
            let name = self.data.nodes[id].name.clone();
            let target = Scope::folder(id);
            let width = available.min(230.0) - 7.0;
            if action(
                ui,
                rect(bx, y, width, bh),
                &name,
                self.scope != target,
                self.scope == target,
            )
            .on_hover_text(self.data.display_path(id))
            .clicked()
            {
                self.open_scope(target);
            }
            bx += width + 7.0;
        }
        let current = model::scope_entry(&self.data, self.scope, self.cs);
        glyph(
            &p,
            rect(x, r.top() + 52.0, 24.0, 24.0),
            if self.mode == Mode::Blocks {
                Glyph::Map
            } else {
                Glyph::Ring
            },
            CYAN,
        );
        let title_r = rect(x + 34.0, r.top() + 48.0, w - 204.0, 35.0);
        label(ui, title_r, &current.name, condensed(29.0), TEXT);
        label(
            ui,
            rect(r.right() - 180.0, r.top() + 58.0, 158.0, 26.0),
            current.coverage.format(current.size(self.allocated)),
            mono(19.0),
            CYAN,
        );
        let controls = r.top() + 88.0;
        let mode_w = if w > 650.0 { 86.0 } else { 72.0 };
        if action(
            ui,
            rect(x, controls, mode_w, 32.0),
            self.tr("Mapa", "Map"),
            true,
            self.mode == Mode::Blocks,
        )
        .clicked()
        {
            self.set_mode(Mode::Blocks);
        }
        if action(
            ui,
            rect(x + mode_w + 4.0, controls, mode_w, 32.0),
            self.tr("Kruhy", "Rings"),
            true,
            self.mode == Mode::Rings,
        )
        .clicked()
        {
            self.set_mode(Mode::Rings);
        }
        let metric_x = x + mode_w * 2.0 + 17.0;
        let metric_w = if w > 650.0 { 155.0 } else { 130.0 };
        let mut metric_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(rect(metric_x, controls, metric_w, 32.0))
                .id_salt("metric"),
        );
        ComboBox::from_id_salt("size-metric")
            .selected_text(self.tr(
                if self.allocated {
                    "Místo na disku"
                } else {
                    "Velikost obsahu"
                },
                if self.allocated {
                    "Allocated size"
                } else {
                    "Logical size"
                },
            ))
            .width(metric_w - 14.0)
            .height(100.0)
            .show_ui(&mut metric_ui, |ui| {
                ui.selectable_value(
                    &mut self.allocated,
                    true,
                    if self.cs {
                        "Místo na disku"
                    } else {
                        "Allocated size"
                    },
                );
                ui.selectable_value(
                    &mut self.allocated,
                    false,
                    if self.cs {
                        "Velikost obsahu"
                    } else {
                        "Logical size"
                    },
                );
            });
        let search_x = metric_x + metric_w + 12.0;
        let search_w = (r.right() - 22.0 - search_x).max(80.0);
        glyph(
            &p,
            rect(search_x + 9.0, controls + 8.0, 16.0, 16.0),
            Glyph::Search,
            MUTED,
        );
        let sr = rect(search_x, controls, search_w, 32.0);
        ui.put(
            sr,
            TextEdit::singleline(&mut self.search)
                .hint_text(if self.cs {
                    "Hledat název nebo cestu…"
                } else {
                    "Search name or path…"
                })
                .font(mono(12.0))
                .margin(vec2(10.0, 7.0)),
        );
        let status_y = controls + 40.0;
        let status = if self.searching() {
            self.tr(
                "Hledání… rozhraní zůstává dostupné",
                "Searching… you can keep exploring",
            )
            .to_string()
        } else if let Some(result) = &self.result {
            format!(
                "{} {} · {} {} · {} {}",
                model::count(result.hits.len()),
                if self.cs {
                    model::plural(result.hits.len(), "shoda", "shody", "shod")
                } else if result.hits.len() == 1 {
                    "match"
                } else {
                    "matches"
                },
                model::count(result.files),
                if self.cs {
                    model::plural(result.files, "soubor", "soubory", "souborů")
                } else if result.files == 1 {
                    "file"
                } else {
                    "files"
                },
                result.coverage.format(if self.allocated {
                    result.allocated
                } else {
                    result.logical
                }),
                self.tr("v souborech", "in files")
            )
        } else {
            format!(
                "{} {} · {}",
                model::count(current.count),
                self.tr("přímých položek", "direct items"),
                self.tr(
                    "Dvojklik nebo Enter otevře složku",
                    "Double click or Enter to open a folder"
                )
            )
        };
        label(
            ui,
            rect(x, status_y, w - 100.0, 23.0),
            status,
            mono(11.0),
            if self.result.as_ref().is_some_and(|r| r.hits.is_empty()) {
                AMBER
            } else {
                MUTED
            },
        );
        if self.filter_active()
            && action(
                ui,
                rect(r.right() - 108.0, status_y - 1.0, 86.0, 25.0),
                self.tr("Zrušit", "Clear"),
                true,
                false,
            )
            .clicked()
        {
            self.search.clear();
            self.category = None;
        }
        let list_h = if self.list_more {
            206.0
        } else if r.height() < 650.0 {
            96.0
        } else {
            124.0
        };
        let map_r = Rect::from_min_max(
            pos2(x, status_y + 36.0),
            pos2(r.right() - 22.0, r.bottom() - list_h - 19.0),
        );
        self.draw_map(ui, map_r);
        let list_top = map_r.bottom() + 17.0;
        line(&p, pos2(x, list_top), pos2(r.right() - 22.0, list_top));
        label(
            ui,
            rect(x, list_top + 7.0, w - 115.0, 24.0),
            self.tr(
                if self.filter_active() {
                    "VÝSLEDKY HLEDÁNÍ"
                } else {
                    "POLOŽKY V TOMTO ROZSAHU"
                },
                if self.filter_active() {
                    "SEARCH RESULTS"
                } else {
                    "ITEMS IN THIS SCOPE"
                },
            ),
            mono(11.0),
            MUTED,
        );
        if action(
            ui,
            rect(r.right() - 130.0, list_top + 6.0, 108.0, 25.0),
            self.tr(
                if self.list_more {
                    "Menší seznam"
                } else {
                    "Větší seznam"
                },
                if self.list_more {
                    "Less list"
                } else {
                    "More list"
                },
            ),
            true,
            self.list_more,
        )
        .clicked()
        {
            self.list_more = !self.list_more;
        }
        self.rows(
            ui,
            Rect::from_min_max(
                pos2(x, list_top + 38.0),
                pos2(r.right() - 22.0, r.bottom() - 10.0),
            ),
        );
    }
    fn rows(&mut self, ui: &mut Ui, r: Rect) {
        let mut child = ui.new_child(UiBuilder::new().max_rect(r).id_salt("item-list"));
        child.set_clip_rect(r);
        let ids = self.display_rows();
        if ids.is_empty() {
            child.label(
                RichText::new(if self.searching() {
                    self.tr("Hledání pokračuje…", "Searching…")
                } else if self.filter_active() {
                    self.tr(
                        if self.scan_incomplete() {
                            "Zatím žádné shody · sken je neúplný."
                        } else {
                            "Žádné shody. Zkuste kratší název nebo zrušte filtr."
                        },
                        if self.scan_incomplete() {
                            "No matches yet · scan is incomplete."
                        } else {
                            "No matches. Try a shorter name or clear the filter."
                        },
                    )
                } else {
                    if self.data.nodes[self.scope.root].coverage == crate::data::Coverage::Complete
                    {
                        self.tr("Tato složka je prázdná.", "This folder is empty.")
                    } else {
                        self.tr(
                            "Obsah není známý nebo byl vynechán. Viz detail skenu.",
                            "Contents unknown or skipped. See scan details.",
                        )
                    }
                })
                .font(mono(12.0))
                .color(MUTED),
            );
            return;
        }
        ScrollArea::vertical()
            .id_salt("items-scroll")
            .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show_rows(&mut child, 32.0, ids.len(), |ui, range| {
                for index in range {
                    let id = ids[index];
                    let key = Choice::Node(id);
                    let n = &self.data.nodes[id];
                    let (rr, response) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
                    if response.has_focus() {
                        self.nav_focus.set(true);
                    }
                    let selected = self.selected == key;
                    let hot = response.hovered() || response.has_focus();
                    panel(
                        ui.painter(),
                        rr,
                        if selected {
                            Color32::from_rgb(12, 39, 52)
                        } else if hot {
                            RAISED
                        } else {
                            PANEL
                        },
                        if selected || response.has_focus() {
                            ink(CYAN, 150)
                        } else {
                            LINE
                        },
                    );
                    let value = n.coverage.format(n.size(self.allocated));
                    let mut name = if n.hard_link_owner.is_some() {
                        format!(
                            "{} · {}",
                            n.name,
                            self.tr("další pevný odkaz", "additional hard link")
                        )
                    } else {
                        n.name.clone()
                    };
                    if n.kind == crate::data::NodeKind::Symlink {
                        name.push_str(self.tr(" · symbolický odkaz", " · symbolic link"));
                    }
                    if n.is_package {
                        name.push_str(self.tr(" · balíček", " · package"));
                    }
                    if matches!(
                        n.cloud,
                        crate::data::CloudState::Dataless
                            | crate::data::CloudState::LegacyPlaceholder
                    ) {
                        name.push_str(self.tr(" · cloud, jen metadata", " · cloud, metadata only"));
                    }
                    response.widget_info(|| {
                        WidgetInfo::selected(
                            Role::Button,
                            true,
                            selected,
                            format!("{name}, {value}"),
                        )
                    });
                    glyph(
                        ui.painter(),
                        rect(rr.left() + 8.0, rr.top() + 7.0, 16.0, 16.0),
                        choice_glyph(self, key),
                        color(n.category),
                    );
                    elide(
                        ui.painter(),
                        rect(
                            rr.left() + 34.0,
                            rr.top() + 2.0,
                            (rr.width() - 162.0).max(20.0),
                            26.0,
                        ),
                        &name,
                        mono(12.0),
                        TEXT,
                        Align::Min,
                    );
                    elide(
                        ui.painter(),
                        rect(rr.right() - 122.0, rr.top() + 2.0, 108.0, 26.0),
                        &value,
                        mono(12.0),
                        color(n.category),
                        Align::Max,
                    );
                    let path = self.data.display_path(id);
                    response.clone().on_hover_text(format!("{path}\n{value}"));
                    if response.clicked() {
                        self.selected = key;
                    }
                    if response.double_clicked()
                        || (response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                    {
                        self.open_choice(key);
                    }
                }
            });
    }
    fn draw_map(&mut self, ui: &mut Ui, r: Rect) {
        if r.width() < 10.0 || r.height() < 30.0 {
            return;
        }
        let p = ui.painter().with_clip_rect(r.expand(10.0));
        if self.preferences.high_contrast {
            p.rect_filled(r, 0, BG);
        } else {
            crate::gpu::background(&p, r, self.hud_time());
        }
        hud::brackets(&p, r.expand(4.0), ink(CYAN, 130), 13.0);
        for i in 0..((r.width() / 16.0) as usize) {
            let x = r.left() + i as f32 * 16.0;
            p.line_segment(
                [
                    pos2(x, r.top() - 7.0),
                    pos2(x, r.top() - if i % 5 == 0 { 11.0 } else { 8.0 }),
                ],
                Stroke::new(0.6, ink(CYAN, 65)),
            );
        }
        if self.result.as_ref().is_some_and(|r| r.hits.is_empty()) {
            let rr = Rect::from_center_size(r.center(), vec2((r.width() - 50.0).min(450.0), 104.0));
            panel(&p, rr, RAISED, LINE);
            label(
                ui,
                rect(rr.left() + 20.0, rr.top() + 17.0, rr.width() - 40.0, 30.0),
                self.tr(
                    if self.scan_incomplete() {
                        "Zatím žádné odpovídající položky"
                    } else {
                        "Žádné odpovídající položky"
                    },
                    if self.scan_incomplete() {
                        "No matching items yet"
                    } else {
                        "No matching items"
                    },
                ),
                condensed(24.0),
                TEXT,
            );
            label(
                ui,
                rect(rr.left() + 20.0, rr.top() + 57.0, rr.width() - 40.0, 28.0),
                self.tr(
                    if self.scan_incomplete() {
                        "Průběžný výsledek · sken je neúplný."
                    } else {
                        "Upravte dotaz nebo zrušte filtr nad mapou."
                    },
                    if self.scan_incomplete() {
                        "Provisional result · scan is incomplete."
                    } else {
                        "Edit the query or clear the filter above."
                    },
                ),
                mono(12.0),
                MUTED,
            );
            return;
        }
        let progress = self
            .transition
            .as_ref()
            .map_or(1.0, |t| (t.start.elapsed().as_secs_f32() / 0.28).min(1.0));
        let mut pending = None;
        if progress < 1.0
            && let Some(t) = &self.transition
        {
            let mut old = p.clone();
            old.set_opacity(1.0 - progress);
            self.paint_map(ui, &old, t.scope, t.mode, r, false, &mut None);
        }
        let mut current = p.clone();
        current.set_opacity(progress);
        let eased = 1.0 - (1.0 - progress).powi(3);
        self.paint_map(
            ui,
            &current,
            self.scope,
            self.mode,
            r.scale_from_center(0.98 + 0.02 * eased),
            progress >= 1.0,
            &mut pending,
        );
        if progress < 1.0 {
            ui.ctx().request_repaint();
        } else {
            self.transition = None;
        }
        if let Some((key, open)) = pending {
            self.selected = key;
            if open {
                self.open_choice(key);
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn paint_map(
        &self,
        ui: &mut Ui,
        p: &Painter,
        scope: Scope,
        mode: Mode,
        r: Rect,
        interactive: bool,
        pending: &mut Option<(Choice, bool)>,
    ) {
        match mode {
            Mode::Blocks => self.blocks(ui, p, scope, r, 0, interactive, pending),
            Mode::Rings => self.rings(ui, p, scope, r, interactive, pending),
        }
    }
    fn map_response(&self, ui: &mut Ui, r: Rect, e: &Entry, kind: &'static str) -> Response {
        let response = ui.interact(r, Id::unique((kind, e.key)), Sense::click());
        if response.has_focus() {
            self.nav_focus.set(true);
        }
        response.widget_info(|| {
            WidgetInfo::selected(
                Role::Button,
                true,
                self.selected == e.key,
                format!(
                    "{}, {}, {} {}",
                    e.name,
                    e.coverage.format(e.size(self.allocated)),
                    model::count(e.count),
                    self.tr("položek", "items")
                ),
            )
        });
        response
    }
    #[allow(clippy::too_many_arguments)]
    fn blocks(
        &self,
        ui: &mut Ui,
        p: &Painter,
        scope: Scope,
        r: Rect,
        depth: usize,
        interactive: bool,
        pending: &mut Option<(Choice, bool)>,
    ) {
        let limit = if depth == 0 {
            48
        } else {
            ((r.area() / 1800.0) as usize).clamp(4, 18)
        };
        let entries = model::contents(&self.data, scope, limit, self.cs);
        let weights: Vec<_> = entries
            .iter()
            .enumerate()
            .map(|(i, e)| (i, e.size(self.allocated)))
            .collect();
        let tiles = layout::treemap(
            &weights,
            Bounds {
                x: r.left(),
                y: r.top(),
                w: r.width(),
                h: r.height(),
            },
        );
        for tile in tiles {
            let e = &entries[tile.id];
            let rr = rect(tile.rect.x, tile.rect.y, tile.rect.w, tile.rect.h).shrink(2.0);
            if rr.width() < 2.0 || rr.height() < 2.0 {
                continue;
            }
            let c = color(e.category);
            let matching = self.matches(e.key);
            let group = matches!(e.key, Choice::Group(_));
            let fill = if matching {
                Color32::from_rgb(
                    (10.0 + c.r() as f32 * 0.075) as u8,
                    (19.0 + c.g() as f32 * 0.085) as u8,
                    (28.0 + c.b() as f32 * 0.10) as u8,
                )
            } else {
                PANEL
            };
            panel(p, rr, fill, ink(c, if matching { 100 } else { 32 }));
            if matching && !self.preferences.high_contrast {
                hud::tile_material(p, rr, c, tile.id);
            }
            let expand = matches!(e.key, Choice::Node(_))
                && e.count > 0
                && depth == 0
                && rr.width() > 155.0
                && rr.height() > 145.0;
            let hit = if expand {
                rr.with_max_y(rr.top() + 33.0)
            } else {
                rr
            };
            let response = if interactive {
                Some(self.map_response(ui, hit, e, "block"))
            } else {
                None
            };
            let hot = response
                .as_ref()
                .is_some_and(|r| r.hovered() || r.has_focus());
            let heat = if interactive && !self.motion_off() {
                ui.ctx()
                    .animate_bool_with_time(Id::unique(("map-heat", e.key)), hot, 0.14)
            } else {
                f32::from(hot)
            };
            if self.selected == e.key || heat > 0.005 {
                let decay =
                    (1.0 - self.selection_time.elapsed().as_secs_f32() / 0.28).clamp(0.0, 1.0);
                let pad = if self.motion_off() {
                    1.5
                } else {
                    1.5 + decay * 2.5
                };
                hud::brackets(p, rr.expand(pad), c, 10.0);
                p.rect_stroke(rr, 2, Stroke::new(1.3, c), StrokeKind::Inside);
            }
            if rr.width() > 75.0 && rr.height() > 28.0 {
                glyph(
                    p,
                    rect(rr.left() + 8.0, rr.top() + 9.0, 15.0, 15.0),
                    if group {
                        Glyph::Group
                    } else {
                        choice_glyph(self, e.key)
                    },
                    if matching { c } else { MUTED },
                );
                let value_w = if expand && rr.width() > 245.0 {
                    86.0
                } else {
                    0.0
                };
                elide(
                    p,
                    rect(
                        rr.left() + 29.0,
                        rr.top() + 5.0,
                        rr.width() - 36.0 - value_w,
                        24.0,
                    ),
                    &e.name,
                    mono(if depth == 0 { 12.0 } else { 11.0 }),
                    if matching { TEXT } else { MUTED },
                    Align::Min,
                );
                if value_w > 0.0 {
                    elide(
                        p,
                        rect(rr.right() - value_w - 9.0, rr.top() + 5.0, value_w, 24.0),
                        &e.coverage.format(e.size(self.allocated)),
                        mono(11.0),
                        c,
                        Align::Max,
                    );
                } else if !expand && rr.height() > 58.0 {
                    elide(
                        p,
                        rect(rr.left() + 10.0, rr.top() + 31.0, rr.width() - 20.0, 23.0),
                        &e.coverage.format(e.size(self.allocated)),
                        mono(12.0),
                        if matching { c } else { MUTED },
                        Align::Min,
                    );
                }
                if group && rr.height() > 120.0 && rr.width() > 160.0 {
                    hud::hatch(
                        p,
                        Rect::from_min_max(rr.min + vec2(12.0, 70.0), rr.max - vec2(12.0, 37.0)),
                        ink(c, 12),
                        12.0,
                    );
                    elide(
                        p,
                        rect(
                            rr.left() + 12.0,
                            rr.bottom() - 31.0,
                            rr.width() - 24.0,
                            22.0,
                        ),
                        self.tr(
                            "Dvojklikem prohlédnout skupinu →",
                            "Double click to explore this group →",
                        ),
                        mono(11.0),
                        MUTED,
                        Align::Min,
                    );
                }
            }
            if let Some(response) = response {
                let tooltip = format!(
                    "{}\n{} · {} {}\n{}",
                    e.name,
                    e.coverage.format(e.size(self.allocated)),
                    model::count(e.count),
                    self.tr("položek", "items"),
                    self.tr("Dvojklik / Enter: otevřít", "Double click / Enter: open")
                );
                response.clone().on_hover_text(tooltip);
                if response.clicked() {
                    *pending = Some((e.key, false));
                }
                if response.double_clicked()
                    || (response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                {
                    *pending = Some((e.key, true));
                }
            }
            if expand && let Choice::Node(id) = e.key {
                self.blocks(
                    ui,
                    p,
                    Scope::folder(id),
                    Rect::from_min_max(rr.min + vec2(5.0, 35.0), rr.max - vec2(5.0, 5.0)),
                    depth + 1,
                    interactive,
                    pending,
                );
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn ring_level(
        &self,
        ui: &mut Ui,
        p: &Painter,
        scope: Scope,
        center: Pos2,
        inner: f32,
        step: f32,
        start: f32,
        end: f32,
        depth: usize,
        interactive: bool,
        pending: &mut Option<(Choice, bool)>,
    ) {
        let entries = model::contents(&self.data, scope, if depth == 0 { 12 } else { 6 }, self.cs);
        let total: u64 = entries.iter().map(|e| e.size(self.allocated)).sum();
        let mut angle = start;
        for e in entries {
            let next = angle
                + (end - start) * (e.size(self.allocated) as f64 / total.max(1) as f64) as f32;
            let gap = 0.004_f32.min((next - angle) * 0.15);
            let a = angle + gap;
            let b = next - gap;
            let outer = inner + step - 2.5;
            if b <= a {
                angle = next;
                continue;
            }
            let c = color(e.category);
            let matching = self.matches(e.key);
            let hover = ui.ctx().pointer_hover_pos().is_some_and(|v| {
                let d = v - center;
                let mut theta = d.angle();
                if theta < -PI / 2.0 {
                    theta += TAU;
                }
                d.length() >= inner && d.length() <= outer && theta >= a && theta <= b
            });
            let mid = (a + b) * 0.5;
            let anchor = center + vec2(mid.cos(), mid.sin()) * ((inner + outer) * 0.5);
            let response = if interactive {
                Some(self.map_response(
                    ui,
                    Rect::from_center_size(anchor, vec2(18.0, 18.0)),
                    &e,
                    "sector",
                ))
            } else {
                None
            };
            let focused = response.as_ref().is_some_and(|r| r.has_focus());
            let selected = self.selected == e.key;
            annulus(
                p,
                center,
                inner,
                outer,
                a,
                b,
                ink(
                    c,
                    if !matching {
                        16
                    } else if selected || hover || focused {
                        155
                    } else {
                        75
                    },
                ),
                ink(
                    c,
                    if selected || hover || focused {
                        240
                    } else if matching {
                        150
                    } else {
                        40
                    },
                ),
            );
            if interactive {
                if hover {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                    if ui.input(|i| i.pointer.primary_clicked()) {
                        *pending = Some((e.key, false));
                    }
                    if ui.input(|i| i.pointer.button_double_clicked(PointerButton::Primary)) {
                        *pending = Some((e.key, true));
                    }
                }
                if hover || focused {
                    Tooltip::always_open(
                        ui.ctx().clone(),
                        ui.layer_id(),
                        Id::unique(("sector-tooltip", e.key)),
                        if hover {
                            PopupAnchor::Pointer
                        } else {
                            PopupAnchor::Position(anchor)
                        },
                    )
                    .show(|ui| {
                        ui.label(RichText::new(&e.name).font(mono(12.0)));
                        ui.label(format!(
                            "{} · {} {}",
                            e.coverage.format(e.size(self.allocated)),
                            model::count(e.count),
                            self.tr("položek", "items")
                        ));
                    });
                }
                if let Some(response) = response {
                    if response.clicked() && !response.clicked_by(PointerButton::Primary) {
                        *pending = Some((e.key, false));
                    }
                    if response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                        *pending = Some((e.key, true));
                    }
                }
            }
            if depth == 0
                && let Choice::Node(id) = e.key
                && e.count > 0
                && step > 12.0
            {
                self.ring_level(
                    ui,
                    p,
                    Scope::folder(id),
                    center,
                    inner + step,
                    step,
                    angle,
                    next,
                    depth + 1,
                    interactive,
                    pending,
                );
            }
            angle = next;
        }
    }
    fn rings(
        &self,
        ui: &mut Ui,
        p: &Painter,
        scope: Scope,
        r: Rect,
        interactive: bool,
        pending: &mut Option<(Choice, bool)>,
    ) {
        let entries = model::contents(&self.data, scope, 12, self.cs);
        let legend_w = if r.width() > 700.0 { 204.0 } else { 166.0 };
        let chart = Rect::from_min_max(
            pos2(r.left() + legend_w + 10.0, r.top() + 5.0),
            r.max - vec2(6.0, 5.0),
        );
        let center = chart.center();
        let radius = (chart.width().min(chart.height()) * 0.5 - 30.0).max(28.0);
        let inner = radius * 0.43;
        let step = (radius - inner) * 0.5;
        hud::dial(p, center, radius + 5.0, CYAN, self.hud_time());
        self.ring_level(
            ui,
            p,
            scope,
            center,
            inner,
            step,
            -PI / 2.0,
            3.0 * PI / 2.0,
            0,
            interactive,
            pending,
        );
        let current = model::scope_entry(&self.data, scope, self.cs);
        let value = current.coverage.format(current.size(self.allocated));
        let (number, unit) = value.split_once(' ').unwrap_or((&value, ""));
        elide(
            p,
            Rect::from_center_size(center - vec2(0.0, 10.0), vec2(inner * 1.8, 30.0)),
            number,
            condensed((inner * 0.55).clamp(16.0, 30.0)),
            TEXT,
            Align::Center,
        );
        elide(
            p,
            Rect::from_center_size(center + vec2(0.0, 17.0), vec2(inner * 1.85, 22.0)),
            &format!("{unit} · {}", self.tr("celkem", "total")),
            mono(10.5),
            MUTED,
            Align::Center,
        );
        let lr = rect(
            r.left() + 8.0,
            r.top() + 5.0,
            legend_w - 8.0,
            r.height() - 10.0,
        );
        let mut child = ui.new_child(UiBuilder::new().max_rect(lr).id_salt("map-legend"));
        child.set_clip_rect(lr);
        ScrollArea::vertical()
            .id_salt("legend-scroll")
            .scroll_bar_visibility(scroll_area::ScrollBarVisibility::AlwaysVisible)
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.label(
                    RichText::new(self.tr("ROZDĚLENÍ PROSTORU", "SPACE DISTRIBUTION"))
                        .font(mono(10.5))
                        .color(MUTED),
                );
                ui.add_space(8.0);
                for e in entries {
                    let (rr, response) = ui.allocate_exact_size(
                        vec2(ui.available_width(), 45.0),
                        if interactive {
                            Sense::click()
                        } else {
                            Sense::hover()
                        },
                    );
                    if response.has_focus() {
                        self.nav_focus.set(true);
                    }
                    let c = color(e.category);
                    let active =
                        self.selected == e.key || response.hovered() || response.has_focus();
                    response.widget_info(|| {
                        WidgetInfo::selected(
                            Role::Button,
                            true,
                            self.selected == e.key,
                            format!("{}, {}", e.name, e.coverage.format(e.size(self.allocated))),
                        )
                    });
                    if active {
                        panel(p, rr.shrink(1.0), RAISED, ink(c, 90));
                    }
                    glyph(
                        p,
                        rect(rr.left() + 6.0, rr.top() + 8.0, 15.0, 15.0),
                        choice_glyph(self, e.key),
                        c,
                    );
                    elide(
                        p,
                        rect(rr.left() + 27.0, rr.top() + 3.0, rr.width() - 33.0, 22.0),
                        &e.name,
                        mono(11.0),
                        if self.matches(e.key) { TEXT } else { MUTED },
                        Align::Min,
                    );
                    let share =
                        e.size(self.allocated) as f64 / current.size(self.allocated).max(1) as f64;
                    let value = if e.coverage == crate::data::Coverage::Unknown {
                        e.coverage.format(e.size(self.allocated))
                    } else {
                        format!(
                            "{}  ·  {:.1}%",
                            e.coverage.format(e.size(self.allocated)),
                            share * 100.0
                        )
                    };
                    elide(
                        p,
                        rect(rr.left() + 27.0, rr.top() + 24.0, rr.width() - 33.0, 18.0),
                        &value,
                        mono(10.5),
                        MUTED,
                        Align::Min,
                    );
                    if interactive {
                        response
                            .clone()
                            .on_hover_text(format!("{}\n{}", e.name, value));
                        if response.clicked() {
                            *pending = Some((e.key, false));
                        }
                        if response.double_clicked()
                            || (response.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)))
                        {
                            *pending = Some((e.key, true));
                        }
                    }
                }
            });
    }
    fn keyboard(&mut self, ui: &mut Ui) {
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::O))
            && self.scan.is_none()
            && let Some(root) = crate::platform::choose_folder(self.cs)
        {
            self.start_scan(root);
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::R))
            && self.scan.is_none()
            && let Some(root) = self.scan_root.clone()
        {
            self.start_scan(root);
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Period))
            && let Some(scan) = &self.scan
        {
            scan.cancel();
        }
        if ui.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::I)) {
            self.detail_popup = !self.detail_popup;
            self.focus_details = self.detail_popup;
        }
        if ui.ctx().text_edit_focused()
            || (ui.ctx().egui_wants_keyboard_input()
                && !self.nav_focus.get()
                && !ui.input(|i| i.key_pressed(Key::Backspace) || i.key_pressed(Key::Escape)))
        {
            return;
        }
        let ids = self.display_rows();
        let pos = if let Choice::Node(id) = self.selected {
            ids.iter().position(|&i| i == id)
        } else {
            None
        };
        if !ids.is_empty() {
            if ui.input(|i| i.key_pressed(Key::ArrowDown)) {
                self.selected = Choice::Node(ids[pos.map_or(0, |p| (p + 1) % ids.len())]);
            }
            if ui.input(|i| i.key_pressed(Key::ArrowUp)) {
                self.selected =
                    Choice::Node(ids[pos.map_or(0, |p| (p + ids.len() - 1) % ids.len())]);
            }
        }
        if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(Key::Enter)) {
            self.open_choice(self.selected);
        }
        if ui.input(|i| i.key_pressed(Key::Backspace)) {
            if self.scope.start > 0 {
                self.open_scope(Scope::folder(self.scope.root));
            } else if let Some(parent) = self.data.nodes[self.scope.root].parent {
                self.open_scope(Scope::folder(parent));
            }
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.search.clear();
            self.category = None;
            self.detail_popup = false;
        }
    }
}
#[allow(clippy::too_many_arguments)]
fn annulus(
    p: &Painter,
    c: Pos2,
    inner: f32,
    outer: f32,
    start: f32,
    end: f32,
    fill: Color32,
    edge: Color32,
) {
    let n = (((end - start) * outer / 3.0).ceil() as usize).clamp(2, 256);
    let mut mesh = Mesh::default();
    let mut inside = Vec::with_capacity(n + 1);
    let mut outside = Vec::with_capacity(n + 1);
    for i in 0..=n {
        let a = start + (end - start) * i as f32 / n as f32;
        let v = vec2(a.cos(), a.sin());
        let ip = c + v * inner;
        let op = c + v * outer;
        mesh.colored_vertex(ip, fill);
        mesh.colored_vertex(op, fill.gamma_multiply(0.65));
        inside.push(ip);
        outside.push(op);
        if i < n {
            let j = (i * 2) as u32;
            mesh.add_triangle(j, j + 1, j + 2);
            mesh.add_triangle(j + 1, j + 3, j + 2);
        }
    }
    p.add(Shape::mesh(mesh));
    p.add(Shape::line(inside, Stroke::new(0.8, edge)));
    p.add(Shape::line(outside, Stroke::new(0.8, edge)));
}
#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(c: Color32) -> f64 {
        let f = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }
    #[test]
    fn functional_palette_has_contrast_reserve() {
        for fg in [TEXT, MUTED, CYAN, AMBER] {
            for bg in [BG, PANEL, RAISED] {
                let ratio = (luminance(fg) + 0.05) / (luminance(bg) + 0.05);
                assert!(ratio >= 4.5, "{fg:?} over {bg:?}: {ratio}");
            }
        }
    }
    #[test]
    fn long_localized_labels_elide_on_whole_rows() {
        let ctx = Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let painter = ui.painter().clone();
            for width in [72.0, 160.0, 240.0] {
                for name in [
                    "Dlouhý název složky – projekty a filmové materiály 2026",
                    "Extrémně_dlouhý_název_bez_mezer_資料_архив",
                    "Allocated size",
                ] {
                    let mut job = LayoutJob::simple(name.into(), mono(12.0), TEXT, width);
                    job.wrap = TextWrapping {
                        max_width: width,
                        max_rows: 1,
                        break_anywhere: true,
                        ..Default::default()
                    };
                    let galley = painter.layout_job(job);
                    assert_eq!(galley.rows.len(), 1);
                    assert!(galley.size().x <= width + 0.1);
                    assert!(galley.size().y < 25.0);
                }
            }
        });
        output.textures_delta.clear();
    }
}
