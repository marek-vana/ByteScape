use crate::layout::{self, Bounds};
use crate::{
    data::{Category, Dataset},
    model::{self, Choice, Entry, Scope, SearchResult},
    platform::{self, Preferences},
};
use eframe::egui::*;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    Blocks,
    Rings,
}
pub(crate) struct Transition {
    pub scope: Scope,
    pub mode: Mode,
    pub start: Instant,
}
pub struct ByteScape {
    pub(crate) scan: Option<crate::scan::Scan>,
    pub(crate) scan_status: Option<crate::scan::Event>,
    scan_run: u64,
    pub(crate) scan_root: Option<std::path::PathBuf>,
    pub(crate) announcement: String,
    announced_state: Option<(u64, crate::scan::State, bool)>,
    last_announcement: Instant,
    pending_scan: Option<crate::scan::Event>,
    last_interaction: Instant,
    pub(crate) nav_focus: std::cell::Cell<bool>,
    pub(crate) data: Arc<Dataset>,
    pub(crate) scope: Scope,
    pub(crate) selected: Choice,
    pub(crate) mode: Mode,
    pub(crate) allocated: bool,
    pub(crate) cs: bool,
    pub(crate) reduced: bool,
    pub(crate) ambient: bool,
    pub(crate) preferences: Preferences,
    pub(crate) search: String,
    pub(crate) category: Option<Category>,
    pub(crate) result: Option<Arc<SearchResult>>,
    version: Arc<AtomicU64>,
    last_filter: (String, Option<Category>),
    search_changed: Instant,
    launched: u64,
    search_mailbox: Arc<Mutex<Option<SearchResult>>>,
    pub(crate) back: Vec<Scope>,
    pub(crate) forward: Vec<Scope>,
    pub(crate) transition: Option<Transition>,
    pub(crate) current_members: Arc<Vec<usize>>,
    pub(crate) detail_open: bool,
    pub(crate) detail_popup: bool,
    pub(crate) focus_details: bool,
    pub(crate) expand_sidebar: bool,
    pub(crate) list_more: bool,
    pub(crate) zoom: f32,
    pub(crate) created: Instant,
    pub(crate) selection_time: Instant,
    last_selection: Choice,
    last_preferences: Instant,
    profile: bool,
    profile_samples: Vec<f64>,
    scan_profile_samples: Vec<f64>,
    profile_cancel_after: Option<Duration>,
    profile_cancel_at: Option<Instant>,
    profile_cancel_observed_ms: Option<f64>,
    ui_wall: Vec<f64>,
    ui_cpu: Vec<f64>,
    cpu_samples: Vec<f64>,
    last_frame: Instant,
    profile_frames: usize,
    last_diag_frame: Option<u64>,
    record: Option<String>,
    renderer: String,
    capture: Option<String>,
    capture_frames: usize,
    idle_profile: bool,
}
impl ByteScape {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = FontDefinitions::default();
        fonts.font_data.insert(
            "plex".into(),
            FontData::from_static(include_bytes!("../assets/fonts/IBMPlexMono-Regular.ttf")).into(),
        );
        fonts.font_data.insert(
            "saira".into(),
            FontData::from_static(include_bytes!(
                "../assets/fonts/SairaCondensed-SemiBold.ttf"
            ))
            .into(),
        );
        fonts
            .families
            .get_mut(&FontFamily::Monospace)
            .unwrap()
            .insert(0, "plex".into());
        fonts
            .families
            .get_mut(&FontFamily::Proportional)
            .unwrap()
            .insert(0, "saira".into());
        cc.egui_ctx.set_fonts(fonts);
        crate::ui::configure(&cc.egui_ctx);
        crate::gpu::initialize(cc);
        let args: Vec<_> = std::env::args().collect();
        let has = |flag: &str| args.iter().any(|a| a == flag);
        let files = if has("--million") { 1_000_000 } else { 12_000 };
        let mut data = Dataset::demo(files);
        if has("--stress-labels") {
            data.nodes[1].name = "Dlouhý název složky – projekty a filmové materiály 2026".into();
            data.nodes[2].name =
                "Extrémně_dlouhý_název_bez_mezer_s_českou_diakritikou_a_koncovkou_архив_資料"
                    .into();
        }
        data.prepare_orders();
        let data = Arc::new(data);
        let renderer = cc
            .wgpu_render_state
            .as_ref()
            .map(|s| {
                let i = s.adapter.get_info();
                format!("{} / {:?}", i.name, i.backend)
            })
            .unwrap_or_default();
        let app = Self::from_data(data, &args, renderer);
        cc.egui_ctx.set_zoom_factor(app.zoom);
        app
    }
    fn from_data(data: Arc<Dataset>, args: &[String], renderer: String) -> Self {
        let has = |flag: &str| args.iter().any(|a| a == flag);
        let scope = if has("--nested") {
            Scope::folder(2)
        } else {
            Scope::ROOT
        };
        let selected = if has("--aggregate") {
            Choice::Group(Scope { root: 2, start: 48 })
        } else {
            Choice::Node(if has("--nested") { 2 } else { 1 })
        };
        let current_members = model::members(&data, scope);
        let zoom = if has("--zoom125") { 1.25 } else { 1.0 };
        let mut app = Self {
            scan: None,
            scan_status: None,
            scan_run: 0,
            scan_root: None,
            announcement: String::new(),
            announced_state: None,
            last_announcement: Instant::now(),
            pending_scan: None,
            last_interaction: Instant::now() - Duration::from_secs(1),
            nav_focus: std::cell::Cell::new(false),
            data,
            scope,
            selected,
            mode: if has("--sunburst") {
                Mode::Rings
            } else {
                Mode::Blocks
            },
            allocated: true,
            cs: !has("--english"),
            reduced: has("--reduced"),
            ambient: !has("--idle-profile"),
            preferences: platform::preferences(),
            search: args
                .iter()
                .position(|a| a == "--query")
                .and_then(|i| args.get(i + 1))
                .cloned()
                .unwrap_or_default(),
            category: None,
            result: None,
            version: Arc::new(AtomicU64::new(0)),
            last_filter: (String::new(), None),
            search_changed: Instant::now(),
            launched: 0,
            search_mailbox: Arc::new(Mutex::new(None)),
            back: Vec::new(),
            forward: Vec::new(),
            transition: None,
            current_members,
            detail_open: true,
            detail_popup: has("--detail"),
            focus_details: false,
            expand_sidebar: false,
            list_more: false,
            zoom,
            created: Instant::now(),
            selection_time: Instant::now(),
            last_selection: selected,
            last_preferences: Instant::now(),
            profile: has("--profile"),
            profile_samples: Vec::new(),
            scan_profile_samples: Vec::new(),
            profile_cancel_after: args
                .iter()
                .position(|a| a == "--profile-cancel-after-ms")
                .and_then(|i| args.get(i + 1))
                .and_then(|s| s.parse().ok())
                .map(Duration::from_millis),
            profile_cancel_at: None,
            profile_cancel_observed_ms: None,
            ui_wall: Vec::new(),
            ui_cpu: Vec::new(),
            cpu_samples: Vec::new(),
            last_frame: Instant::now(),
            profile_frames: 0,
            last_diag_frame: None,
            record: args
                .iter()
                .position(|a| a == "--record")
                .and_then(|i| args.get(i + 1))
                .cloned(),
            renderer,
            capture: args
                .iter()
                .position(|a| a == "--capture")
                .and_then(|i| args.get(i + 1))
                .cloned(),
            capture_frames: 0,
            idle_profile: has("--idle-profile"),
        };
        if has("--aggregate") {
            app.scope = Scope { root: 2, start: 0 };
            app.current_members = model::members(&app.data, app.scope);
        }
        if let Some(i) = args.iter().position(|a| a == "--scan")
            && let Some(root) = std::env::args_os().nth(i + 1)
        {
            app.start_scan(root.into());
        }
        app
    }
    pub(crate) fn start_scan(&mut self, root: std::path::PathBuf) {
        if self.scan.is_some() {
            return;
        }
        self.scan_root = Some(root.clone());
        self.scan_run += 1;
        self.scan_status = None;
        self.pending_scan = None;
        let request = crate::scan::Request::new(root, self.scan_run);
        // Debug-only metadata adapter for reproducible native cancellation QA.
        #[cfg(debug_assertions)]
        let request = {
            let mut request = request;
            let args: Vec<_> = std::env::args().collect();
            request.metadata_delay = Duration::from_millis(
                args.iter()
                    .position(|s| s == "--scan-test-delay-ms")
                    .and_then(|i| args.get(i + 1))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            );
            request
        };
        self.scan = Some(crate::scan::Scan::start(request));
    }
    fn interacting(&self) -> bool {
        self.last_interaction.elapsed() < Duration::from_millis(350)
            || self
                .transition
                .as_ref()
                .is_some_and(|t| t.start.elapsed() < Duration::from_millis(280))
    }
    pub(crate) fn revision_waiting(&self) -> bool {
        self.pending_scan.is_some()
    }
    pub(crate) fn scan_incomplete(&self) -> bool {
        self.pending_scan.is_some()
            || self.scan_status.as_ref().is_some_and(|e| {
                matches!(
                    e.state,
                    crate::scan::State::Running | crate::scan::State::Cancelled
                )
            })
    }
    fn update_scan(&mut self, ctx: &Context) {
        // Include the release frame: the UI must dispatch its click against the
        // same immutable tree that was painted when the button went down.
        if ctx.input(|i| i.pointer.any_down() || !i.events.is_empty()) {
            self.last_interaction = Instant::now();
        }
        if let Some(scan) = &self.scan {
            if let Ok(event) = scan.poll()
                && event.run_id == self.scan_run
                && event.revision > self.scan_status.as_ref().map_or(0, |e| e.revision)
            {
                self.pending_scan = Some(event.clone());
                let mut status = event;
                status.data = None;
                self.scan_status = Some(status);
            }
            if scan.worker_finished()
                && self
                    .scan_status
                    .as_ref()
                    .is_some_and(|e| e.state != crate::scan::State::Running)
            {
                self.scan = None;
            }
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        if self.pending_scan.is_some() {
            if self.interacting() {
                ctx.request_repaint_after(Duration::from_millis(50));
                return;
            }
            let event = self.pending_scan.take().unwrap();
            if let Some(data) = event.data {
                self.version.fetch_add(1, Ordering::Relaxed);
                if self.data.run_id != data.run_id {
                    self.scope = Scope::ROOT;
                    self.selected = Choice::Node(0);
                    self.last_selection = self.selected;
                    self.back.clear();
                    self.forward.clear();
                    self.search.clear();
                    self.category = None;
                }
                // Animation scopes and aggregate offsets belong to the old revision.
                self.transition = None;
                self.data = data;
                self.result = None;
                self.current_members = model::members(&self.data, self.scope);
            }
        }
    }
    pub(crate) fn update_announcement(&mut self) {
        let Some(event) = &self.scan_status else {
            if self.scan.is_some() {
                self.announcement = self
                    .tr(
                        "Skenování · čekání na metadata",
                        "Scanning · awaiting metadata",
                    )
                    .into();
            } else {
                self.announcement = self
                    .tr("UKÁZKA · syntetická data", "DEMO · synthetic data")
                    .into();
            }
            return;
        };
        let state = (event.run_id, event.state, self.cs);
        if self.announced_state != Some(state)
            || self.last_announcement.elapsed() >= Duration::from_secs(5)
        {
            let count = self
                .pending_scan
                .as_ref()
                .and_then(|e| e.data.as_ref())
                .filter(|d| d.run_id == Some(event.run_id))
                .or_else(|| (self.data.run_id == Some(event.run_id)).then_some(&self.data))
                .map_or(0, |d| d.nodes.len().saturating_sub(1));
            self.announcement = format!(
                "{} · {}: {} · {}: {}",
                self.scan_state_text(event.state),
                self.tr("Položky", "Items"),
                count,
                self.tr("Problémy", "Problems"),
                event.problems.len()
            );
            self.announced_state = Some(state);
            self.last_announcement = Instant::now();
        }
    }
    pub(crate) fn tr<'a>(&self, cs: &'a str, en: &'a str) -> &'a str {
        if self.cs { cs } else { en }
    }
    pub(crate) fn motion_off(&self) -> bool {
        self.reduced || self.preferences.reduce_motion
    }
    pub(crate) fn hud_time(&self) -> f32 {
        if self.ambient && !self.motion_off() {
            self.created.elapsed().as_secs_f32()
        } else {
            0.0
        }
    }
    pub(crate) fn filter_active(&self) -> bool {
        !self.search.trim().is_empty() || self.category.is_some()
    }
    pub(crate) fn searching(&self) -> bool {
        self.filter_active() && self.result.is_none()
    }
    pub(crate) fn matches(&self, key: Choice) -> bool {
        let Some(r) = &self.result else {
            return true;
        };
        match key {
            Choice::Node(id) => r.subtree[id],
            Choice::Group(s) => self
                .data
                .children(s.root, true)
                .iter()
                .skip(s.start)
                .any(|&id| r.subtree[id]),
        }
    }
    pub(crate) fn info(&self, key: Choice) -> Entry {
        model::entry(&self.data, key, self.cs)
    }
    pub(crate) fn open_scope(&mut self, scope: Scope) {
        if scope == self.scope {
            return;
        }
        self.back.push(self.scope);
        self.forward.clear();
        self.change_scope(scope);
        self.search.clear();
        self.category = None;
    }
    pub(crate) fn change_scope(&mut self, scope: Scope) {
        if !self.motion_off() {
            self.transition = Some(Transition {
                scope: self.scope,
                mode: self.mode,
                start: Instant::now(),
            });
        }
        self.scope = scope;
        self.current_members = model::members(&self.data, scope);
        self.selected = if scope.start == 0 {
            Choice::Node(scope.root)
        } else {
            Choice::Group(scope)
        };
    }
    pub(crate) fn open_choice(&mut self, key: Choice) {
        let scope = match key {
            Choice::Group(s) => s,
            Choice::Node(id) => {
                if self.data.nodes[id].kind != crate::data::NodeKind::Directory {
                    return;
                }
                Scope::folder(id)
            }
        };
        self.open_scope(scope);
    }
    pub(crate) fn set_mode(&mut self, mode: Mode) {
        if mode == self.mode {
            return;
        }
        if !self.motion_off() {
            self.transition = Some(Transition {
                scope: self.scope,
                mode: self.mode,
                start: Instant::now(),
            });
        }
        self.mode = mode;
    }
    pub(crate) fn display_rows(&self) -> Arc<Vec<usize>> {
        self.result.as_ref().map_or_else(
            || Arc::clone(&self.current_members),
            |r| Arc::clone(&r.hits),
        )
    }
    fn update_search(&mut self, ctx: &Context) {
        if self.search != self.last_filter.0 || self.category != self.last_filter.1 {
            self.last_filter = (self.search.clone(), self.category);
            self.version.fetch_add(1, Ordering::Relaxed);
            self.result = None;
            self.search_changed = Instant::now();
        }
        let generation = self.version.load(Ordering::Relaxed);
        if self.filter_active() && self.launched != generation {
            if self.search_changed.elapsed() < Duration::from_millis(140) {
                ctx.request_repaint_after(Duration::from_millis(140));
            } else {
                self.launched = generation;
                let data = Arc::clone(&self.data);
                let query = self.search.clone();
                let category = self.category;
                let version = Arc::clone(&self.version);
                let mailbox = Arc::clone(&self.search_mailbox);
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    if let Some(result) =
                        model::search(&data, &query, category, generation, &version)
                    {
                        let mut pending = mailbox.lock().unwrap();
                        if version.load(Ordering::Relaxed) == generation {
                            *pending = Some(result);
                            drop(pending);
                            ctx.request_repaint();
                        }
                    }
                });
            }
        }
        if self.filter_active() && self.interacting() {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
        let mut pending = self.search_mailbox.lock().unwrap();
        if pending.as_ref().is_some_and(|r| r.generation != generation) {
            *pending = None;
        }
        if !self.interacting()
            && let Some(result) = pending.take()
        {
            self.result = Some(Arc::new(result));
        }
        if !self.filter_active() {
            self.result = None;
        }
    }
    fn profile_tick(&mut self, ctx: &Context, interval_ms: f64, cpu: Option<f32>) {
        let scanning = self.scan.is_some() || self.pending_scan.is_some();
        if scanning {
            self.scan_profile_samples.push(interval_ms);
            if self.profile_cancel_at.is_none()
                && self
                    .profile_cancel_after
                    .is_some_and(|after| self.created.elapsed() >= after)
                && let Some(scan) = &self.scan
            {
                self.profile_cancel_at = Some(Instant::now());
                scan.cancel();
            }
        } else {
            self.profile_frames += 1;
            if self.profile_frames > 90 {
                self.profile_samples.push(interval_ms);
                if let Some(cpu) = cpu {
                    self.cpu_samples.push(cpu as f64 * 1000.0);
                }
            }
        }
        let tick = self.profile_frames + self.scan_profile_samples.len();
        if tick.is_multiple_of(120) {
            self.set_mode(if self.mode == Mode::Blocks {
                Mode::Rings
            } else {
                Mode::Blocks
            });
            // A real tree can be empty or contain only files. Never navigate to a made-up ID.
            let target = if self.scope == Scope::ROOT {
                self.data.nodes[0]
                    .children
                    .iter()
                    .copied()
                    .find(|&id| self.data.nodes[id].kind == crate::data::NodeKind::Directory)
                    .unwrap_or(0)
            } else {
                0
            };
            self.change_scope(Scope::folder(target));
        }
        if self.profile_frames >= 690 {
            let (mean, p95, max) = crate::validation::summary(&self.profile_samples);
            let (scan_mean, scan_p95, scan_max) =
                crate::validation::summary(&self.scan_profile_samples);
            let (_, cpu95, _) = crate::validation::summary(&self.cpu_samples);
            let (_, ui_wall, _) = crate::validation::summary(&self.ui_wall);
            let (_, ui_cpu, _) = crate::validation::summary(&self.ui_cpu);
            let source = if self.data.run_id.is_some() {
                "filesystem"
            } else {
                "synthetic"
            };
            let state = self
                .scan_status
                .as_ref()
                .map_or("None".to_owned(), |e| format!("{:?}", e.state));
            let report = format!(
                "{{\"renderer\":\"{}\",\"source\":\"{source}\",\"scan_state\":\"{state}\",\"files\":{},\"nodes\":{},\"logical\":{},\"allocated\":{},\"frames\":{},\"frame_interval_mean_ms\":{mean:.3},\"frame_interval_p95_ms\":{p95:.3},\"frame_interval_max_ms\":{max:.3},\"during_scan_frames\":{},\"during_scan_mean_ms\":{scan_mean:.3},\"during_scan_p95_ms\":{scan_p95:.3},\"during_scan_max_ms\":{scan_max:.3},\"previous_frame_cpu_p95_ms\":{cpu95:.3},\"ui_pass_wall_p95_ms\":{ui_wall:.3},\"ui_pass_thread_cpu_p95_ms\":{ui_cpu:.3},\"peak_rss_kib\":{},\"cancel_status_drawn_ms\":{},\"scope\":\"UI frame-start intervals; forced repaint; alternating root/folder and mode; 90 completed-tree warmup frames; no GPU timestamps or presented-frame proof\"}}\n",
                self.renderer,
                self.data.files,
                self.data.nodes.len(),
                self.data.nodes[0].logical,
                self.data.nodes[0].allocated,
                self.profile_samples.len(),
                self.scan_profile_samples.len(),
                crate::validation::peak_rss_kib().unwrap_or(0),
                self.profile_cancel_observed_ms
                    .map_or_else(|| "null".into(), |v| format!("{v:.3}"))
            );
            let path = std::env::var("BYTESCAPE_PROFILE_TO")
                .unwrap_or_else(|_| "bytescape-profile.json".into());
            if let Err(e) = std::fs::write(&path, &report) {
                eprintln!("Profile write failed: {e}");
            }
            if let Ok(path) = std::env::var("BYTESCAPE_PROFILE_FRAMES_TO") {
                let mut rows = String::from("phase,interval_ms\n");
                for (phase, values) in [
                    ("scan", &self.scan_profile_samples),
                    ("completed", &self.profile_samples),
                ] {
                    for value in values {
                        rows.push_str(&format!("{phase},{value:.6}\n"));
                    }
                }
                if let Err(e) = std::fs::write(path, rows) {
                    eprintln!("Frame samples write failed: {e}");
                }
            }
            println!("{report}");
            self.profile = false;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        } else {
            ctx.request_repaint();
        }
    }
    fn diagnostics(&mut self, ctx: &Context, cpu: Option<f32>) {
        let frame_id = ctx.cumulative_frame_nr();
        if self.last_diag_frame == Some(frame_id) {
            return;
        }
        self.last_diag_frame = Some(frame_id);
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        if self.profile {
            self.profile_tick(ctx, delta * 1000.0, cpu);
        }
        self.capture_frames += 1;
        if let Some(dir) = self.record.clone() {
            let n = self.capture_frames;
            if n == 24 {
                self.set_mode(Mode::Rings);
            }
            if n == 90 {
                self.open_scope(Scope::folder(1));
            }
            if n == 155 {
                self.set_mode(Mode::Blocks);
            }
            if n == 210 {
                self.open_scope(Scope::ROOT);
            }
            if n >= 16 && n.is_multiple_of(2) {
                let ctx = ctx.clone();
                let time = self.created.elapsed().as_secs_f64();
                ctx.clone().request_screenshot(move |image| {
                    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                        std::fs::create_dir_all(&dir)?;
                        let stem = format!("{dir}/frame-{n:04}");
                        let file = std::fs::File::create(format!("{stem}.png"))?;
                        let mut encoder = png::Encoder::new(
                            std::io::BufWriter::new(file),
                            image.size[0] as u32,
                            image.size[1] as u32,
                        );
                        encoder.set_color(png::ColorType::Rgba);
                        encoder.set_depth(png::BitDepth::Eight);
                        encoder.set_compression(png::Compression::Fast);
                        let mut writer = encoder.write_header()?;
                        let pixels: Vec<u8> =
                            image.pixels.iter().flat_map(|p| p.to_array()).collect();
                        writer.write_image_data(&pixels)?;
                        std::fs::write(format!("{stem}.time"), time.to_string())?;
                        Ok(())
                    })();
                    if let Err(e) = result {
                        eprintln!("Recording failed: {e}");
                    }
                    if n >= 240 {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                    ctx.request_repaint();
                });
            }
            ctx.request_repaint();
        }
        if self.capture.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        if self.capture_frames >= 8
            && !self.searching()
            && self.scan.is_none()
            && self.created.elapsed() > Duration::from_millis(450)
            && let Some(path) = self.capture.take()
        {
            let ctx = ctx.clone();
            ctx.clone().request_screenshot(move |image| {
                let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                    let file = std::fs::File::create(&path)?;
                    let mut encoder = png::Encoder::new(
                        std::io::BufWriter::new(file),
                        image.size[0] as u32,
                        image.size[1] as u32,
                    );
                    encoder.set_color(png::ColorType::Rgba);
                    encoder.set_depth(png::BitDepth::Eight);
                    let mut writer = encoder.write_header()?;
                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
                    writer.write_image_data(&pixels)?;
                    Ok(())
                })();
                match result {
                    Ok(()) => println!("Captured {}x{} to {path}", image.size[0], image.size[1]),
                    Err(e) => eprintln!("Capture failed: {e}"),
                };
                ctx.send_viewport_cmd(ViewportCommand::Close);
                ctx.request_repaint();
            });
        }
        if self.idle_profile {
            if self.created.elapsed() >= Duration::from_secs(5) {
                println!(
                    "idle_elapsed_ms={} ui_passes={}",
                    self.created.elapsed().as_millis(),
                    self.capture_frames
                );
                self.idle_profile = false;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            } else {
                ctx.request_repaint_after(Duration::from_secs(5) - self.created.elapsed());
            }
        }
    }
}
impl eframe::App for ByteScape {
    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        if self.last_preferences.elapsed() > Duration::from_secs(2) {
            self.preferences = platform::preferences();
            self.last_preferences = Instant::now();
        }
        ui.ctx().request_repaint_after(Duration::from_secs(2));
        self.update_scan(ui.ctx());
        self.update_search(ui.ctx());
        if self.motion_off() {
            self.transition = None;
        }
        if self.selected != self.last_selection {
            self.last_selection = self.selected;
            self.selection_time = Instant::now();
        }
        if !self.motion_off() && self.selection_time.elapsed() < Duration::from_millis(280) {
            ui.ctx().request_repaint();
        }
        self.diagnostics(ui.ctx(), frame.info().cpu_usage);
        let render_start = Instant::now();
        let cpu_start = thread_cpu_seconds();
        self.draw(ui);
        if self.profile_cancel_observed_ms.is_none()
            && self
                .scan_status
                .as_ref()
                .is_some_and(|e| e.state == crate::scan::State::Cancelled)
            && let Some(at) = self.profile_cancel_at
        {
            self.profile_cancel_observed_ms = Some(at.elapsed().as_secs_f64() * 1000.0);
        }
        if self.profile && self.profile_frames > 90 {
            self.ui_wall
                .push(render_start.elapsed().as_secs_f64() * 1000.0);
            if let (Some(start), Some(end)) = (cpu_start, thread_cpu_seconds()) {
                self.ui_cpu.push((end - start) * 1000.0);
            }
        }
        if self.ambient && !self.motion_off() && ui.input(|i| i.focused) {
            ui.ctx().request_repaint_after(Duration::from_millis(33));
        }
    }
}
pub fn benchmark() {
    let start = Instant::now();
    let data = Dataset::demo(1_000_000);
    let generation = start.elapsed();
    let aggregate_start = Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(model::entry(
            &data,
            Choice::Group(Scope { root: 2, start: 6 }),
            true,
        ));
    }
    let aggregate_ms = aggregate_start.elapsed().as_secs_f64() * 1000.0;
    println!("aggregate_1000_ms={aggregate_ms:.3} budget_ms=50");
    if aggregate_ms > 50.0 {
        eprintln!("FAIL: repeated aggregate metadata exceeds CPU budget");
        std::process::exit(1);
    }
    let roots = [0, 1, 2, 14, 15];
    let mut samples = Vec::new();
    let mut tiles = 0;
    for _ in 0..100 {
        let t = Instant::now();
        for root in roots {
            let weights: Vec<_> = data
                .children(root, true)
                .iter()
                .copied()
                .map(|id| (id, data.nodes[id].allocated))
                .collect();
            tiles += layout::treemap(
                &weights,
                Bounds {
                    x: 0.0,
                    y: 0.0,
                    w: 1000.0,
                    h: 600.0,
                },
            )
            .len();
        }
        samples.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "files={} nodes={} generation_ms={:.3} five_layouts_p95_ms={:.3} total_tiles={}\nScope: CPU generation and geometry only; no GPU, scan, or frame-rate claim.",
        data.files,
        data.nodes.len(),
        generation.as_secs_f64() * 1000.0,
        samples[95],
        tiles
    );
}

fn thread_cpu_seconds() -> Option<f64> {
    #[cfg(unix)]
    {
        let mut value = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: clock_gettime writes one initialized timespec through a valid pointer.
        let status = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut value) };
        if status == 0 {
            return Some(value.tv_sec as f64 + value.tv_nsec as f64 / 1e9);
        }
    }
    None
}

#[cfg(test)]
mod scan_navigation_tests {
    use super::*;
    use crate::scan::{Request, Scan, State};

    #[test]
    fn completed_batch_waits_for_pointer_release_before_replacing_navigation() {
        let root = std::env::temp_dir().join(format!("bytescape-ui-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        std::fs::write(root.join("found.txt"), b"1234567").unwrap();
        let mut app = ByteScape::from_data(Arc::new(Dataset::demo(100)), &[], String::new());
        app.scan_run = 606;
        app.scan = Some(Scan::start(Request::new(root.clone(), 606)));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !app.scan.as_ref().unwrap().worker_finished() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let old = Arc::clone(&app.data);
        let old_members = Arc::clone(&app.current_members);
        let ctx = Context::default();
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: pos2(400.0, 400.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::default(),
        });
        let mut output = ctx.run_ui(input, |ui| app.update_scan(ui.ctx()));
        output.textures_delta.clear();
        assert!(
            Arc::ptr_eq(&old, &app.data),
            "pressed target must keep its revision"
        );
        assert!(Arc::ptr_eq(&old_members, &app.current_members));
        assert_eq!(app.scan_status.as_ref().unwrap().state, State::Complete);
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: pos2(400.0, 400.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::default(),
        });
        let mut output = ctx.run_ui(input, |ui| app.update_scan(ui.ctx()));
        output.textures_delta.clear();
        assert!(
            Arc::ptr_eq(&old, &app.data),
            "release click uses the old target"
        );
        std::thread::sleep(Duration::from_millis(400));
        let mut output = ctx.run_ui(RawInput::default(), |ui| app.update_scan(ui.ctx()));
        output.textures_delta.clear();
        assert_eq!(app.data.run_id, Some(606));
        assert_eq!(app.data.nodes[0].logical, 7);
        assert_eq!(app.selected, Choice::Node(0));
        assert_eq!(app.scope, Scope::ROOT);
        assert!(app.back.is_empty());
        assert!(app.transition.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn search_and_aggregate_navigation_follow_the_displayed_scan_revision() {
        let root = std::env::temp_dir().join(format!("bytescape-search-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        for i in 0..400 {
            std::fs::write(root.join(format!("item-{i:04}.txt")), b"abc").unwrap();
        }
        std::fs::write(root.join("empty.txt"), b"").unwrap();
        let mut app = ByteScape::from_data(Arc::new(Dataset::demo(100)), &[], String::new());
        app.scan_run = 607;
        let mut request = Request::new(root.clone(), 607);
        request.metadata_delay = Duration::from_millis(2);
        app.scan = Some(Scan::start(request));
        let ctx = Context::default();
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.data.run_id != Some(607) || app.data.files < 10 {
            assert!(Instant::now() < deadline);
            frame(&mut app, &ctx, RawInput::default());
            std::thread::sleep(Duration::from_millis(5));
        }
        app.search = "item-".into();
        app.category = Some(Category::Documents);
        // A batch may arrive while the asynchronous query is in flight.
        while app.result.is_none() {
            assert!(Instant::now() < deadline);
            frame(&mut app, &ctx, RawInput::default());
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: pos2(500.0, 400.0),
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::default(),
        });
        frame(&mut app, &ctx, input);
        let displayed = Arc::clone(&app.data);
        let generation = app.version.load(Ordering::Relaxed);
        while app.scan.is_some() {
            assert!(Instant::now() < deadline);
            frame(&mut app, &ctx, RawInput::default());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(Arc::ptr_eq(&displayed, &app.data));
        assert_eq!(app.version.load(Ordering::Relaxed), generation);
        if let Some(result) = &app.result {
            assert_eq!(result.subtree.len(), displayed.nodes.len());
        }
        app.search = "empty.txt".into();
        frame(&mut app, &ctx, RawInput::default());
        let mut input = RawInput::default();
        input.events.push(Event::PointerButton {
            pos: pos2(500.0, 400.0),
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::default(),
        });
        frame(&mut app, &ctx, input);
        std::thread::sleep(Duration::from_millis(400));
        frame(&mut app, &ctx, RawInput::default());
        assert_eq!(app.data.files, 401);
        assert_eq!(app.search, "empty.txt");
        assert_eq!(app.category, Some(Category::Documents));
        while app.result.is_none() {
            assert!(Instant::now() < deadline);
            frame(&mut app, &ctx, RawInput::default());
            std::thread::sleep(Duration::from_millis(5));
        }
        let result = app.result.as_ref().unwrap();
        assert_eq!(result.generation, app.version.load(Ordering::Relaxed));
        assert_eq!(result.hits.len(), 1);
        assert_eq!(app.data.nodes[result.hits[0]].name, "empty.txt");
        assert_eq!(result.logical, 0);
        assert_eq!(result.subtree.len(), app.data.nodes.len());
        let entries = model::contents(&app.data, Scope::ROOT, 4, true);
        let group = entries.last().unwrap().key;
        assert!(matches!(group, Choice::Group(_)));
        app.open_choice(group);
        assert_eq!(app.current_members.len(), 397);
        assert_eq!(
            app.info(group).logical,
            app.current_members
                .iter()
                .map(|&id| app.data.nodes[id].logical)
                .sum::<u64>()
        );
        app.set_mode(Mode::Rings);
        app.allocated = false;
        assert_eq!(app.selected, group);
        let previous = app.back.pop().unwrap();
        app.change_scope(previous);
        assert_eq!(app.scope, Scope::ROOT);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scan_workflow_exposes_localized_controls_and_live_terminal_status() {
        let root = std::env::temp_dir().join(format!("bytescape-a11y-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        std::fs::write(root.join(".remote.txt.icloud"), b"stub").unwrap();
        let mut app = ByteScape::from_data(Arc::new(Dataset::demo(10)), &[], String::new());
        app.start_scan(root.clone());
        let ctx = Context::default();
        ctx.enable_accesskit();
        let deadline = Instant::now() + Duration::from_secs(10);
        while app.scan.is_some() {
            assert!(Instant::now() < deadline);
            frame(&mut app, &ctx, RawInput::default());
            std::thread::sleep(Duration::from_millis(5));
        }
        for (cs, choose, again) in [
            (true, "Vybrat složku…", "Skenovat znovu"),
            (false, "Choose folder…", "Scan again"),
        ] {
            app.cs = cs;
            app.detail_popup = true;
            app.preferences.reduce_motion = true;
            app.reduced = false;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(714.0, 500.0))),
                ..Default::default()
            };
            // egui sizes a newly opened window before exposing its contents.
            let mut sizing = ctx.run_ui(input.clone(), |ui| app.draw(ui));
            sizing.textures_delta.clear();
            let mut output = ctx.run_ui(input, |ui| app.draw(ui));
            output.textures_delta.clear();
            assert_eq!(
                ctx.global_style().animation_time,
                0.0,
                "system motion preference takes priority"
            );
            let tree = output.platform_output.accesskit_update.unwrap();
            let copy = tree
                .nodes
                .iter()
                .map(|(_, n)| n)
                .find(|n| {
                    n.label()
                        == Some(if cs {
                            "Kopírovat celou cestu"
                        } else {
                            "Copy full path"
                        })
                })
                .unwrap();
            let bounds = copy.bounds().unwrap();
            assert!(
                bounds.x0 >= 0.0 && bounds.x1 <= 714.0 && bounds.y0 >= 0.0 && bounds.y1 <= 500.0,
                "details must remain inside the compact enlarged viewport: {bounds:?}"
            );
            assert!(tree.nodes.iter().any(|(_, n)| n.label() == Some(choose)));
            assert!(
                tree.nodes.iter().any(|(_, n)| n.label() == Some(again)),
                "repeat scan is accessible"
            );
            assert!(
                tree.nodes
                    .iter()
                    .any(|(_, n)| n.role() == accesskit::Role::Status
                        && n.live() == Some(accesskit::Live::Polite)
                        && n.label().is_some_and(|s| !s.is_empty())),
                "terminal scan state is a polite live status"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn keyboard_cancellation_announces_terminal_state_without_progress_chatter() {
        let root = std::env::temp_dir().join(format!("bytescape-live-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        for i in 0..1000 {
            std::fs::write(root.join(format!("file-{i}.txt")), b"abc").unwrap();
        }
        let mut app = ByteScape::from_data(Arc::new(Dataset::demo(10)), &[], String::new());
        app.cs = false;
        app.scan_run = 707;
        app.scan_root = Some(root.clone());
        let mut request = Request::new(root.clone(), 707);
        request.metadata_delay = Duration::from_millis(3);
        app.scan = Some(Scan::start(request));
        let ctx = Context::default();
        ctx.enable_accesskit();
        let deadline = Instant::now() + Duration::from_secs(15);
        while app.data.run_id != Some(707) || app.data.files == 0 {
            assert!(Instant::now() < deadline);
            ax_frame(&mut app, &ctx, Vec::new());
            std::thread::sleep(Duration::from_millis(5));
        }
        let before = app.data.files;
        let spoken = app.announcement.clone();
        while app.data.files == before {
            assert!(Instant::now() < deadline);
            ax_frame(&mut app, &ctx, Vec::new());
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(app.scan_status.as_ref().unwrap().state, State::Running);
        assert_eq!(
            app.announcement, spoken,
            "progress batches do not each announce"
        );
        ax_frame(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::Period,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        let nodes = ax_frame(&mut app, &ctx, Vec::new());
        assert_eq!(app.scan_status.as_ref().unwrap().state, State::Cancelled);
        assert!(nodes.iter().any(|n| n.role() == accesskit::Role::Status
            && n.label().is_some_and(|s| s.contains("cancelled"))));
        while app.scan.is_some() {
            assert!(Instant::now() < deadline);
            ax_frame(&mut app, &ctx, Vec::new());
        }
        ax_frame(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::R,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        assert_eq!(app.scan_run, 708, "keyboard repeat starts a new run");
        app.scan.as_ref().unwrap().cancel();
        while app.scan.is_some() {
            assert!(Instant::now() < deadline);
            ax_frame(&mut app, &ctx, Vec::new());
        }
        let nodes = ax_frame(&mut app, &ctx, Vec::new());
        let bounds = nodes
            .iter()
            .find(|n| n.label() == Some("Scan again"))
            .unwrap()
            .bounds()
            .unwrap();
        let position = pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        );
        for pressed in [true, false] {
            ax_frame(
                &mut app,
                &ctx,
                vec![Event::PointerButton {
                    pos: position,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::default(),
                }],
            );
        }
        while app.scan_status.is_none() {
            assert!(Instant::now() < deadline);
            ax_frame(&mut app, &ctx, Vec::new());
        }
        assert_eq!(app.scan_run, 709, "pointer repeat starts one new run");
        assert_ne!(
            app.scan_status.as_ref().unwrap().state,
            State::Cancelled,
            "repeat click must not activate cancel in the same frame"
        );
        if let Some(scan) = &app.scan {
            scan.cancel();
        }
        while app.scan.is_some() {
            assert!(Instant::now() < deadline);
            ax_frame(&mut app, &ctx, Vec::new());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cancellation_is_painted_while_pointer_keeps_previous_revision() {
        use std::sync::atomic::AtomicBool;
        let root =
            std::env::temp_dir().join(format!("bytescape-cancel-visible-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        std::fs::write(root.join("blocked"), b"abc").unwrap();
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let mut request = Request::new(root.clone(), 809);
        request.metadata_pause = Some((entered.clone(), release.clone()));
        let mut app = ByteScape::from_data(Arc::new(Dataset::demo(10)), &[], String::new());
        app.cs = false;
        app.scan_run = 809;
        app.scan = Some(Scan::start(request));
        let deadline = Instant::now() + Duration::from_secs(5);
        while !entered.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let ctx = Context::default();
        ctx.enable_accesskit();
        ax_frame(&mut app, &ctx, vec![]);
        ax_frame(
            &mut app,
            &ctx,
            vec![Event::PointerButton {
                pos: pos2(400.0, 400.0),
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::default(),
            }],
        );
        ax_frame(
            &mut app,
            &ctx,
            vec![Event::Key {
                key: Key::Period,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::COMMAND,
            }],
        );
        std::thread::sleep(Duration::from_millis(1100));
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            app.update_scan(ui.ctx());
            app.draw(ui);
        });
        output.textures_delta.clear();
        release.store(true, Ordering::SeqCst);
        assert_eq!(app.scan_status.as_ref().unwrap().state, State::Cancelled);
        assert!(
            app.pending_scan.is_some(),
            "interaction must retain the revision"
        );
        assert!(output.shapes.iter().any(|shape| matches!(&shape.shape,
            eframe::egui::epaint::Shape::Text(text) if text.galley.job.text.contains("SCAN CANCELLED"))),
            "visible painted text must acknowledge cancellation while interaction continues");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn non_utf8_problem_path_cannot_be_copied_lossily() {
        use std::os::unix::ffi::OsStrExt;
        let root =
            std::env::temp_dir().join(format!("bytescape-raw-problem-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        let missing = root.join(std::ffi::OsStr::from_bytes(b"missing-\xff"));
        let mut app = ByteScape::from_data(Arc::new(Dataset::demo(10)), &[], String::new());
        app.cs = false;
        app.start_scan(missing);
        let ctx = Context::default();
        ctx.enable_accesskit();
        let deadline = Instant::now() + Duration::from_secs(5);
        while app.scan.is_some() {
            assert!(Instant::now() < deadline);
            frame(&mut app, &ctx, RawInput::default());
            std::thread::sleep(Duration::from_millis(2));
        }
        app.detail_popup = true;
        let render = |app: &mut ByteScape, events: Vec<Event>| {
            let mut output = ctx.run_ui(
                RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 1800.0))),
                    events,
                    ..Default::default()
                },
                |ui| app.draw(ui),
            );
            output.textures_delta.clear();
            output
                .platform_output
                .accesskit_update
                .unwrap()
                .nodes
                .into_iter()
                .map(|(_, n)| n)
                .collect::<Vec<_>>()
        };
        render(&mut app, vec![]);
        let nodes = render(&mut app, vec![]);
        let bounds = nodes
            .iter()
            .find(|n| n.label().is_some_and(|s| s.starts_with("Problem 1:")))
            .unwrap()
            .bounds()
            .unwrap();
        let pos = pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        );
        for pressed in [true, false] {
            render(
                &mut app,
                vec![Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::default(),
                }],
            );
        }
        let nodes = render(&mut app, vec![]);
        let copy = nodes
            .iter()
            .find(|n| n.label() == Some("Copy problem path"))
            .expect("expanded problem copy control");
        assert!(
            copy.is_disabled(),
            "a lossy problem path must not be copied as if exact"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    fn ax_frame(app: &mut ByteScape, ctx: &Context, events: Vec<Event>) -> Vec<accesskit::Node> {
        let mut output = ctx.run_ui(
            RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))),
                events,
                ..Default::default()
            },
            |ui| {
                app.update_scan(ui.ctx());
                app.draw(ui);
            },
        );
        output.textures_delta.clear();
        output
            .platform_output
            .accesskit_update
            .unwrap()
            .nodes
            .into_iter()
            .map(|(_, n)| n)
            .collect()
    }

    fn frame(app: &mut ByteScape, ctx: &Context, input: RawInput) {
        let mut output = ctx.run_ui(input, |ui| {
            app.update_scan(ui.ctx());
            app.update_search(ui.ctx());
        });
        output.textures_delta.clear();
    }
}
