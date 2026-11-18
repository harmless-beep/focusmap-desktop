//! The application shell: window layout, state cache, tray, toasts, shortcuts.
//!
//! Screens live in `screens/` and are plain methods on [`FocusMapApp`]. The
//! shell owns everything expensive on purpose: one live snapshot per second, one
//! database pass per minute, and a fixed-size cache of day rollups. Nothing here
//! grows with how long the app has been running.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::Timelike;

use eframe::egui;
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Margin, Sense, Stroke, Vec2,
};

use focusmap_core::apps::AppCategory;
use focusmap_core::engine::{Engine, LiveState};
use focusmap_core::model::{
    day_range, format_minutes, pretty_day, shift_day, DaySummary, MinuteSample, Session,
};
use focusmap_core::platform;
use focusmap_core::settings::{MotionMode, Settings};
use focusmap_core::share_card::{self, ShareData};
use focusmap_core::stats::{self, AppStat, HourStat, TagStat};
use focusmap_core::storage::{DataCounts, Db};

use crate::ambient::Ambient;
use crate::autostart;
use crate::notify;
use crate::theme::{self, Theme, SPACE};
use crate::tray::{TrayAction, TrayHandle};
use crate::widgets;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Screen {
    Today,
    Week,
    Insights,
    Goals,
    Share,
    Privacy,
    Settings,
}

impl Screen {
    pub(crate) const ALL: [Screen; 7] = [
        Screen::Today,
        Screen::Week,
        Screen::Insights,
        Screen::Goals,
        Screen::Share,
        Screen::Privacy,
        Screen::Settings,
    ];

    pub(crate) fn title(self) -> &'static str {
        match self {
            Screen::Today => "Today",
            Screen::Week => "Week & month",
            Screen::Insights => "Insights",
            Screen::Goals => "Goals & records",
            Screen::Share => "Share card",
            Screen::Privacy => "Privacy & data",
            Screen::Settings => "Settings",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Screen::Today => "One day, minute by minute",
            Screen::Week => "The shape of your week",
            Screen::Insights => "What your timing says about you",
            Screen::Goals => "The streak you are protecting",
            Screen::Share => "A card that leaves this device only when you say so",
            Screen::Privacy => "What is stored, and what never is",
            Screen::Settings => "Make FocusMap fit your day",
        }
    }

    fn hint(self) -> &'static str {
        match self {
            Screen::Today => "1",
            Screen::Week => "2",
            Screen::Insights => "3",
            Screen::Goals => "4",
            Screen::Share => "5",
            Screen::Privacy => "6",
            Screen::Settings => "7",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Period {
    Week,
    Month,
}

impl Period {
    pub(crate) fn days(self) -> u32 {
        match self {
            Period::Week => 7,
            Period::Month => 30,
        }
    }
}

/// Bounded in-memory copy of the database. `all_days` is one small row per day
/// and stays tiny even after years; minute rows are loaded one day at a time.
#[derive(Default)]
pub(crate) struct DataCache {
    pub summaries: Vec<DaySummary>,
    pub hours: Vec<HourStat>,
    pub apps: Vec<AppStat>,
    pub tags: Vec<TagStat>,
    pub counts: DataCounts,
    pub detail_day: String,
    pub detail_samples: Vec<MinuteSample>,
    pub detail_sessions: Vec<Session>,
}

impl DataCache {
    pub fn refresh(&mut self, db: &Db, today: &str) {
        let from = shift_day(today, -29);
        self.summaries = db.all_days().unwrap_or_default();
        self.hours = db.hour_stats(&from).unwrap_or_default();
        self.apps = db.app_stats(&from).unwrap_or_default();
        self.tags = db
            .sessions_since(&from)
            .map(|sessions| stats::tag_totals(&sessions))
            .unwrap_or_default();
        self.counts = db.counts().unwrap_or_default();
    }

    pub fn load_detail(&mut self, db: &Db, day: &str) {
        if self.detail_day == day {
            return;
        }
        self.detail_samples = db.samples(day).unwrap_or_default();
        self.detail_sessions = db.sessions(day).unwrap_or_default();
        self.detail_day = day.to_string();
    }

    pub fn forget_detail(&mut self) {
        self.detail_day.clear();
        self.detail_samples.clear();
        self.detail_sessions.clear();
    }
}

pub(crate) struct TagEdit {
    pub day: String,
    pub start: i64,
    pub text: String,
}

#[derive(Default)]
pub(crate) struct ShareState {
    pub texture: Option<egui::TextureHandle>,
    pub error: Option<String>,
    pub saved: Option<PathBuf>,
}

struct Toast {
    title: String,
    body: String,
    tint: Color32,
    until: Instant,
}

#[derive(Default)]
pub(crate) struct ResourceSample {
    pub working_set: u64,
    pub cpu_pct: f32,
}

pub struct FocusMapApp {
    pub(crate) engine: Engine,
    pub(crate) db: Arc<Db>,
    pub(crate) settings: Settings,
    pub(crate) demo: bool,
    pub(crate) screen: Screen,
    pub(crate) theme: Theme,
    pub(crate) live: LiveState,
    pub(crate) cache: DataCache,
    pub(crate) cache_at: Instant,
    pub(crate) live_at: Instant,
    pub(crate) tray: Option<TrayHandle>,
    toasts: Vec<Toast>,
    pub(crate) share: ShareState,
    pub(crate) period: Period,
    pub(crate) week_day: String,
    pub(crate) onboarding_step: usize,
    pub(crate) settings_dirty: bool,
    settings_changed_at: Instant,
    pub(crate) resources: ResourceSample,
    res_at: Instant,
    cpu_prev: (Instant, u64),
    pub(crate) font_name: Option<String>,
    pub(crate) data_dir_draft: String,
    pub(crate) confirm_wipe: bool,
    pub(crate) tag_edit: Option<TagEdit>,
    pub(crate) selected_hour: Option<u8>,
    pub(crate) city_view: bool,
    pub(crate) city_ready: bool,
    pub(crate) city_buildings: Arc<[focusmap_city::Building]>,
    pub(crate) city_mood: focusmap_city::Mood,
    pub(crate) city_intensity: f32,
    city_dirty: bool,
    pub(crate) city_focus: bool,
    /// GPU handles kept for the one-shot Appolis share export.
    gpu_device: Option<eframe::wgpu::Device>,
    gpu_queue: Option<eframe::wgpu::Queue>,
    gpu_format: Option<eframe::wgpu::TextureFormat>,
    /// A tower the user clicked, kept by app name across rebuilds.
    pub(crate) city_pinned: Option<String>,
    /// Minute of day to replay the city at, or `None` for live.
    pub(crate) city_replay: Option<u16>,
    ambient: Ambient,
    pub(crate) mini: bool,
    full_size: Vec2,
    start_hidden: bool,
    first_update: bool,
    quitting: bool,
    system_dark: bool,
    system_reduced: bool,
    tooltip_at: Instant,
    pub(crate) last_export: Option<PathBuf>,
    pub(crate) error: Option<String>,
}

impl FocusMapApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        engine: Engine,
        db: Arc<Db>,
        settings: Settings,
        demo: bool,
    ) -> Self {
        let ctx = &cc.egui_ctx;
        let font_name = theme::install_fonts(ctx);
        let system_dark = platform::system_prefers_dark();
        let system_reduced = platform::system_reduced_motion();
        let theme = Theme::resolve(
            settings.theme,
            system_dark,
            settings.high_contrast,
            reduced(&settings, system_reduced),
        );
        theme.apply(ctx);
        ctx.set_zoom_factor(settings.text_scale());

        let live = engine.snapshot();
        let mut cache = DataCache::default();
        cache.refresh(&db, &live.today);
        let tray = TrayHandle::new(live.paused);
        let start_hidden =
            settings.start_minimized || std::env::args().any(|a| a == "--minimized");
        let now = Instant::now();
        let data_dir_draft = settings.data_dir.clone();
        let gpu = cc.wgpu_render_state.as_ref().map(|render_state| {
            render_state
                .renderer
                .write()
                .callback_resources
                .insert(focusmap_city::CityRenderer::new(
                    &render_state.device,
                    render_state.target_format,
                ));
            (
                render_state.device.clone(),
                render_state.queue.clone(),
                render_state.target_format,
            )
        });
        let city_ready = gpu.is_some();
        let mut app = Self {
            engine,
            db,
            settings,
            demo,
            screen: Screen::Today,
            theme,
            live: live.clone(),
            cache,
            cache_at: now,
            live_at: now,
            tray,
            toasts: Vec::new(),
            share: ShareState::default(),
            period: Period::Week,
            week_day: live.today.clone(),
            onboarding_step: 0,
            settings_dirty: false,
            settings_changed_at: now,
            resources: ResourceSample::default(),
            res_at: now,
            cpu_prev: (now, platform::cpu_time_100ns()),
            font_name,
            data_dir_draft,
            confirm_wipe: false,
            tag_edit: None,
            selected_hour: None,
            city_view: false,
            city_ready,
            city_buildings: Arc::from(Vec::new()),
            city_mood: focusmap_city::Mood::Idle,
            city_intensity: 0.0,
            city_dirty: true,
            city_focus: false,
            gpu_device: gpu.as_ref().map(|g| g.0.clone()),
            gpu_queue: gpu.as_ref().map(|g| g.1.clone()),
            gpu_format: gpu.as_ref().map(|g| g.2),
            city_pinned: None,
            city_replay: None,
            ambient: Ambient::default(),
            mini: false,
            full_size: Vec2::new(1360.0, 880.0),
            start_hidden,
            first_update: true,
            quitting: false,
            system_dark,
            system_reduced,
            tooltip_at: now,
            last_export: None,
            error: None,
        };
        app.sync_ambient();
        app
    }

    // ---- pulse -----------------------------------------------------------

    fn pump(&mut self, ctx: &egui::Context) {
        if self.live_at.elapsed() >= Duration::from_millis(950) {
            self.live = self.engine.snapshot();
            self.live_at = Instant::now();
            self.city_dirty = true;
        }
        if self.cache_at.elapsed() >= Duration::from_secs(60) {
            self.refresh_cache();
        }
        if self.settings_dirty
            && self.settings_changed_at.elapsed() >= Duration::from_millis(700)
        {
            self.persist_now();
        }
        if self.res_at.elapsed() >= Duration::from_secs(2) {
            self.sample_resources();
        }
        self.collect_nudges();
        self.poll_tray(ctx);

        if self.tooltip_at.elapsed() >= Duration::from_secs(5) {
            self.tooltip_at = Instant::now();
            let text = if self.engine.is_paused() {
                "FocusMap - tracking paused".to_string()
            } else {
                format!(
                    "FocusMap - {} deep today",
                    format_minutes(self.live.summary.deep_minutes)
                )
            };
            if let Some(tray) = self.tray.as_mut() {
                tray.set_tooltip(&text);
            }
        }

        if self.first_update {
            self.first_update = false;
            if self.start_hidden {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        let city_visible = self.city_view
            && self.screen == Screen::Today
            && ctx.input(|i| {
                let viewport = i.viewport();
                !viewport.minimized.unwrap_or(false) && viewport.focused.unwrap_or(true)
            });
        if city_visible {
            let fps = self.settings.city_quality.fps().max(1);
            ctx.request_repaint_after(Duration::from_millis(1000 / fps as u64));
        }

        // One heartbeat per second also runs while the window is hidden, so the
        // tray stays responsive without repainting anything.
        ctx.request_repaint_after(Duration::from_millis(1000));
    }

    fn refresh_cache(&mut self) {
        self.cache.refresh(&self.db, &self.live.today);
        self.cache_at = Instant::now();
    }

    /// Switching Appolis on schedules a rebuild and brings the city into view.
    pub(crate) fn set_city_view(&mut self, on: bool) {
        if on && !self.city_view {
            self.city_dirty = true;
            self.city_focus = true;
        }
        if !on {
            self.city_pinned = None;
            self.city_replay = None;
        }
        self.city_view = on;
    }

    /// Scrub the city through the day. A change schedules a rebuild from the
    /// samples up to that minute.
    pub(crate) fn set_city_replay(&mut self, minute: Option<u16>) {
        if self.city_replay != minute {
            self.city_replay = minute;
            self.city_dirty = true;
        }
    }

    /// Rebuild the skyline from the live snapshot. The city only ever sees
    /// minute samples the Today screen already shows.
    pub(crate) fn refresh_city(&mut self) {
        if !self.city_view || !self.city_ready || !self.city_dirty {
            return;
        }
        let snapshot = self.city_snapshot(self.city_replay);
        self.city_mood = snapshot.mood;
        self.city_intensity = (snapshot.score / 100.0).clamp(0.0, 1.0);
        self.city_buildings = focusmap_city::layout(&snapshot).into();
        self.city_dirty = false;
    }

    fn city_snapshot(&self, cutoff: Option<u16>) -> focusmap_city::CitySnapshot {
        let mut rows: HashMap<&str, (u32, u32, u32, AppCategory)> = HashMap::new();
        let mut scored_sum = 0u32;
        let mut scored_minutes = 0u32;
        for sample in &self.live.samples {
            if cutoff.is_some_and(|minute| sample.minute_of_day > minute) {
                continue;
            }
            if sample.score > 0 {
                scored_sum += sample.score as u32;
                scored_minutes += 1;
            }
            if sample.app.is_empty() || sample.score == 0 {
                continue;
            }
            let row = rows
                .entry(sample.app.as_str())
                .or_insert((0, 0, 0, sample.category));
            row.0 += 1;
            row.1 += sample.score as u32;
            row.2 += sample.switches;
        }

        let mut apps: Vec<focusmap_city::CityApp> = rows
            .into_iter()
            .map(|(name, (minutes, total, switches, category))| focusmap_city::CityApp {
                name: name.to_string(),
                district: district_for(category),
                minutes,
                score: total as f32 / minutes.max(1) as f32,
                switches,
                active: self.live.current_app.eq_ignore_ascii_case(name),
            })
            .collect();
        apps.sort_by_key(|app| std::cmp::Reverse(app.minutes));
        apps.truncate(48);

        // Replay scores only the minutes before the cut-off; live follows the
        // day summary.
        let score = if cutoff.is_some() && scored_minutes > 0 {
            scored_sum as f32 / scored_minutes as f32
        } else {
            self.live.summary.score as f32
        };

        // The city describes the day, so it follows the day's score. The live
        // minute is FocusMap itself whenever the user is looking at the city,
        // which would otherwise flip the mood to scattered on every visit.
        let mood = if (cutoff.is_none() && self.live.idle_secs > 120) || apps.is_empty() {
            focusmap_city::Mood::Idle
        } else if score >= 70.0 {
            focusmap_city::Mood::Deep
        } else if score >= 45.0 {
            focusmap_city::Mood::Steady
        } else {
            focusmap_city::Mood::Distracted
        };

        focusmap_city::CitySnapshot { apps, mood, score }
    }

    fn collect_nudges(&mut self) {
        for nudge in self.engine.drain_nudges() {
            let tint = match nudge.kind {
                focusmap_core::engine::NudgeKind::Goal => self.theme.palette.good,
                focusmap_core::engine::NudgeKind::Break => self.theme.palette.medium,
                focusmap_core::engine::NudgeKind::Burnout => self.theme.palette.warn,
            };
            if !self.settings.focus_mode {
                notify::send(&nudge.title, &nudge.body);
            }
            self.toast(nudge.title, nudge.body, tint);
        }
    }

    fn poll_tray(&mut self, ctx: &egui::Context) {
        let actions = self.tray.as_ref().map(|t| t.poll()).unwrap_or_default();
        for action in actions {
            match action {
                TrayAction::Open => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayAction::TogglePause => self.toggle_pause(),
                TrayAction::Mini => self.set_mini(!self.mini, ctx),
                TrayAction::Quit => {
                    self.persist_now();
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn sample_resources(&mut self) {
        let now = Instant::now();
        let cpu = platform::cpu_time_100ns();
        let elapsed = now.duration_since(self.cpu_prev.0).as_secs_f64();
        if elapsed >= 1.0 && cpu >= self.cpu_prev.1 {
            let cpu_secs = (cpu - self.cpu_prev.1) as f64 / 10_000_000.0;
            self.resources.cpu_pct = (cpu_secs / elapsed * 100.0) as f32;
            if elapsed >= 1.5 {
                self.cpu_prev = (now, cpu);
            }
        }
        self.resources.working_set = platform::working_set_bytes();
        self.res_at = now;
    }

    // ---- actions ---------------------------------------------------------

    pub(crate) fn toggle_pause(&mut self) {
        let paused = !self.engine.is_paused();
        self.engine.set_paused(paused);
        self.live.paused = paused;
        if let Some(tray) = self.tray.as_ref() {
            tray.set_paused(paused);
        }
        if paused {
            self.toast(
                "Tracking paused",
                "Nothing is recorded until you resume.",
                self.theme.palette.warn,
            );
        } else {
            self.toast(
                "Tracking resumed",
                "The next full minute will be scored.",
                self.theme.palette.good,
            );
        }
    }

    pub(crate) fn toggle_manual(&mut self) {
        let on = !self.engine.is_manual();
        self.engine.set_manual(on);
        self.live.manual = on;
        if on {
            self.toast(
                "Manual focus on",
                "Reading and thinking score from now until you turn it off.",
                self.theme.palette.medium,
            );
        } else {
            self.toast(
                "Manual focus off",
                "Back to reading input timing only.",
                self.theme.palette.muted,
            );
        }
    }

    pub(crate) fn set_mini(&mut self, on: bool, ctx: &egui::Context) {
        self.mini = on;
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if on {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(if on {
            Vec2::new(368.0, 250.0)
        } else {
            self.full_size
        }));
        if !on {
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
    }

    pub(crate) fn go(&mut self, screen: Screen) {
        if self.screen == screen {
            return;
        }
        if self.screen == Screen::Share || screen == Screen::Share {
            self.share.texture = None;
            self.share.error = None;
        }
        self.screen = screen;
    }

    pub(crate) fn toast(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
        tint: Color32,
    ) {
        self.toasts.push(Toast {
            title: title.into(),
            body: body.into(),
            tint,
            until: Instant::now() + Duration::from_secs(7),
        });
        if self.toasts.len() > 3 {
            self.toasts.remove(0);
        }
    }

    /// Match the audio loop to the saved setting; toggling a switch is enough.
    pub(crate) fn sync_ambient(&mut self) {
        self.ambient.set(self.settings.ambient_sound);
    }

    pub(crate) fn settings_changed(&mut self) {
        self.settings.score.sanitize();
        self.engine.apply_settings(&self.settings);
        self.settings_dirty = true;
        self.settings_changed_at = Instant::now();
        self.sync_ambient();
    }

    pub(crate) fn appearance_changed(&mut self, ctx: &egui::Context) {
        self.settings_changed();
        self.theme = Theme::resolve(
            self.settings.theme,
            self.system_dark,
            self.settings.high_contrast,
            reduced(&self.settings, self.system_reduced),
        );
        self.theme.apply(ctx);
        ctx.set_zoom_factor(self.settings.text_scale());
    }

    pub(crate) fn persist_now(&mut self) {
        if self.demo {
            self.settings_dirty = false;
            return;
        }
        match self.settings.save() {
            Ok(()) => self.settings_dirty = false,
            Err(err) => {
                self.error = Some(format!("Could not save settings: {err}"));
                self.settings_dirty = false;
            }
        }
    }

    pub(crate) fn apply_start_on_boot(&mut self, enabled: bool) {
        if let Err(err) = autostart::set_enabled(enabled) {
            self.settings.start_on_boot = !enabled;
            self.toast(
                "Start on boot failed",
                err.to_string(),
                self.theme.palette.danger,
            );
        }
        self.settings_changed();
    }

    pub(crate) fn wipe_all(&mut self) {
        match self.db.wipe() {
            Ok(()) => {
                self.engine.reload();
                self.live = self.engine.snapshot();
                self.cache.refresh(&self.db, &self.live.today);
                self.cache.forget_detail();
                self.share.texture = None;
                self.share.saved = None;
                self.selected_hour = None;
                self.confirm_wipe = false;
                self.toast(
                    "All tracking data wiped",
                    "Settings and your goal were kept. This cannot be undone.",
                    self.theme.palette.good,
                );
            }
            Err(err) => self.toast("Wipe failed", err.to_string(), self.theme.palette.danger),
        }
    }

    pub(crate) fn save_tag(&mut self, day: &str, start: i64, text: &str) {
        let tag = text.trim();
        let tag = if tag.is_empty() { None } else { Some(tag) };
        match self.db.set_tag(day, start, tag) {
            Ok(()) => {
                self.engine.reload();
                self.live = self.engine.snapshot();
                self.cache.forget_detail();
                if self.cache.detail_day == day {
                    self.cache.detail_day.clear();
                }
            }
            Err(err) => self.toast("Tag not saved", err.to_string(), self.theme.palette.danger),
        }
    }

    pub(crate) fn ensure_detail(&mut self, day: &str) {
        if day == self.live.today {
            return;
        }
        self.cache.load_detail(&self.db, day);
    }

    pub(crate) fn summary_for(&self, day: &str) -> DaySummary {
        self.cache
            .summaries
            .iter()
            .find(|d| d.day == day)
            .cloned()
            .unwrap_or(DaySummary {
                day: day.to_string(),
                ..Default::default()
            })
    }

    pub(crate) fn share_data(&self) -> ShareData {
        let mut sums = [0u32; 24];
        let mut counts = [0u32; 24];
        for sample in &self.live.samples {
            if sample.score > 0 {
                let hour = (sample.minute_of_day / 60) as usize;
                sums[hour] += sample.score as u32;
                counts[hour] += 1;
            }
        }
        let mut hour_scores = [None; 24];
        for hour in 0..24 {
            if counts[hour] > 0 {
                hour_scores[hour] = Some((sums[hour] / counts[hour]) as u8);
            }
        }
        ShareData {
            day: self.live.today.clone(),
            score: self.live.summary.score,
            deep_minutes: self.live.summary.deep_minutes,
            longest_session_minutes: self.live.summary.longest_session_minutes,
            streak: stats::streak(
                &self.cache.summaries,
                &self.live.today,
                self.settings.goal_minutes,
            ),
            goal_minutes: self.settings.goal_minutes,
            hour_scores,
        }
    }

    pub(crate) fn ensure_share_texture(&mut self, ctx: &egui::Context) {
        if self.share.texture.is_some() || self.share.error.is_some() {
            return;
        }
        let data = self.share_data();
        match share_card::render_rgba(&data) {
            Ok((rgba, width, height)) => {
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [width as usize, height as usize],
                    &rgba,
                );
                // The pixmap is dropped here; only the GPU texture stays resident.
                let texture = ctx.load_texture("share-card", image, egui::TextureOptions::LINEAR);
                self.share.texture = Some(texture);
            }
            Err(err) => self.share.error = Some(err.to_string()),
        }
    }

    pub(crate) fn save_share(&mut self) {
        let data = self.share_data();
        let dir = pictures_dir().join("FocusMap");
        let path = dir.join(format!("focusmap-{}.png", data.day));
        match share_card::write_png(&data, &path) {
            Ok(()) => {
                self.share.saved = Some(path.clone());
                self.toast(
                    "Share card saved",
                    path.display().to_string(),
                    self.theme.palette.good,
                );
            }
            Err(err) => {
                self.share.error = Some(err.to_string());
                self.toast("Could not save the card", err.to_string(), self.theme.palette.danger);
            }
        }
    }

    /// Render the current Appolis snapshot straight to a 1080 square and save
    /// it next to the normal share card. A throwaway renderer keeps the live
    /// city's targets and frame budget untouched.
    pub(crate) fn save_city_share(&mut self) {
        // Build the snapshot here: the live city may never have been opened on
        // this screen, and a replay cut-off must not shrink the exported day.
        let snapshot = self.city_snapshot(None);
        let mood = if snapshot.apps.is_empty() {
            focusmap_city::Mood::Idle
        } else if snapshot.score >= 70.0 {
            focusmap_city::Mood::Deep
        } else if snapshot.score >= 45.0 {
            focusmap_city::Mood::Steady
        } else {
            focusmap_city::Mood::Distracted
        };
        let intensity = (snapshot.score / 100.0).clamp(0.0, 1.0);
        let buildings: Arc<[focusmap_city::Building]> =
            focusmap_city::layout(&snapshot).into();
        let (Some(device), Some(queue), Some(format)) =
            (self.gpu_device.clone(), self.gpu_queue.clone(), self.gpu_format)
        else {
            self.toast(
                "Appolis export unavailable",
                "This build has no GPU renderer.",
                self.theme.palette.danger,
            );
            return;
        };
        let now = chrono::Local::now();
        let look = focusmap_city::CityLook {
            mood,
            intensity,
            hour: (now.hour() as f32 + now.minute() as f32 / 60.0) / 24.0,
            highlight: None,
            render_scale: 1.0,
            traffic: true,
            fps: 30,
            blueprint: self.settings.city_style == focusmap_core::settings::CityStyle::Blueprint,
        };
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f32()
            % 3600.0;
        let day = self.live.today.clone();
        let dir = pictures_dir().join("FocusMap");
        let path = dir.join(format!("focusmap-appolis-{day}.png"));

        let mut renderer = focusmap_city::CityRenderer::new(&device, format);
        let result = renderer
            .export_rgba(&device, &queue, 1080, time, &look, &buildings)
            .and_then(|(rgba, w, h)| {
                share_card::encode_rgba_png(&rgba, w, h).map_err(|e| e.to_string())
            })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            .and_then(|png| {
                std::fs::create_dir_all(&dir)?;
                std::fs::write(&path, png)
            });
        match result {
            Ok(()) => {
                self.share.saved = Some(path.clone());
                self.toast(
                    "Appolis card saved",
                    path.display().to_string(),
                    self.theme.palette.good,
                );
            }
            Err(err) => self.toast(
                "Could not save the Appolis card",
                err.to_string(),
                self.theme.palette.danger,
            ),
        }
    }

    pub(crate) fn export_data(&mut self) {
        let dir = self.settings.data_dir().join("exports");
        match self
            .db
            .export_bundle()
            .and_then(|bundle| bundle.write_files(&dir))
        {
            Ok((csv, _json)) => {
                self.last_export = Some(csv);
                self.cache.counts = self.db.counts().unwrap_or_default();
                self.toast(
                    "Export written",
                    dir.display().to_string(),
                    self.theme.palette.good,
                );
            }
            Err(err) => self.toast("Export failed", err.to_string(), self.theme.palette.danger),
        }
    }

    pub(crate) fn apply_data_dir(&mut self) {
        let draft = self.data_dir_draft.trim().to_string();
        if !draft.is_empty() {
            if let Err(err) = std::fs::create_dir_all(&draft) {
                self.toast(
                    "Folder is not usable",
                    err.to_string(),
                    self.theme.palette.danger,
                );
                return;
            }
        }
        self.settings.data_dir = draft;
        self.settings_changed();
        self.persist_now();
        self.toast(
            "Folder saved",
            "Restart FocusMap to use it. Existing data is not moved automatically.",
            self.theme.palette.warn,
        );
    }

    // ---- chrome ----------------------------------------------------------

    fn draw_sidebar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        egui::Panel::left("sidebar")
            .exact_size(224.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                Frame::NONE
                    .fill(theme::alpha(p.panel, if p.dark { 224 } else { 232 }))
                    .inner_margin(Margin::symmetric(14, 18)),
            )
            .show(ui, |ui| {
                self.brand(ui);
                ui.add_space(SPACE * 2.0);
                for screen in Screen::ALL {
                    if nav_item(ui, &theme, screen.title(), screen.hint(), self.screen == screen) {
                        self.go(screen);
                    }
                }

                let remaining = ui.available_height();
                ui.add_space((remaining - 208.0).max(6.0));
                self.sidebar_status(ui);
            });
    }

    fn brand(&self, ui: &mut egui::Ui) {
        let p = self.theme.palette;
        ui.horizontal(|ui| {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
            let painter = ui.painter();
            theme::glow(painter, rect.center(), 26.0, p.accent, 90);
            painter.circle_filled(rect.center(), 13.0, p.accent);
            painter.circle_stroke(rect.center(), 7.0, Stroke::new(1.8, Color32::WHITE));
            painter.circle_filled(rect.center() + Vec2::new(7.5, -7.5), 2.4, Color32::WHITE);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(theme::bold(16.5, "FocusMap").color(p.text));
                ui.label(theme::regular(9.5, "strava for your pc").color(p.faint));
            });
        });
    }

    fn sidebar_status(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        let paused = self.engine.is_paused();
        let manual = self.engine.is_manual();
        let goal = self.settings.goal_minutes.max(1);
        let deep = self.live.summary.deep_minutes;
        let status_color = if paused {
            p.warn
        } else if manual {
            p.medium
        } else if self.live.tracking {
            p.good
        } else {
            p.danger
        };
        let status = if paused {
            "Paused"
        } else if manual {
            "Manual focus"
        } else if self.live.tracking {
            "Tracking"
        } else {
            "Capture unavailable"
        };
        egui::Frame::NONE
            .fill(theme::alpha(p.card, 224))
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(14)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    widgets::dot(ui, status_color, 3.5);
                    ui.label(theme::bold(10.5, status.to_uppercase()).color(p.muted));
                });
                ui.add_space(5.0);
                let app = if self.live.current_app.is_empty() {
                    "Waiting for input"
                } else {
                    &self.live.current_app
                };
                ui.label(theme::regular(11.5, app).color(p.text));
                ui.add_space(7.0);
                widgets::progress(
                    ui,
                    &theme,
                    deep as f32 / goal as f32,
                    p.accent,
                    5.0,
                );
                ui.add_space(4.0);
                ui.label(
                    theme::regular(10.0, format!("{} of {} goal", format_minutes(deep), format_minutes(goal)))
                        .color(p.faint),
                );
                ui.add_space(8.0);
                let label = if paused { "Resume tracking" } else { "Pause tracking" };
                if ui
                    .add(
                        egui::Button::new(theme::regular(12.0, label).color(p.text))
                            .fill(p.card_alt)
                            .stroke(Stroke::new(1.0, p.border))
                            .corner_radius(9)
                            .min_size(Vec2::new(ui.available_width(), 30.0)),
                    )
                    .clicked()
                {
                    self.toggle_pause();
                }
            });
    }

    fn draw_topbar(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        egui::Panel::top("topbar")
            .exact_size(66.0)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                Frame::NONE
                    .fill(theme::alpha(p.bg, if p.dark { 150 } else { 200 }))
                    .inner_margin(Margin::symmetric(26, 12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 1.0;
                        ui.label(theme::bold(18.0, self.screen.title()).color(p.text));
                        ui.label(theme::regular(11.5, self.screen.subtitle()).color(p.faint));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_button(ui, &theme, if self.mini { "Expand" } else { "Mini" }) {
                            self.set_mini(!self.mini, ui.ctx());
                        }
                        if small_button(ui, &theme, if self.engine.is_paused() { "Resume" } else { "Pause" }) {
                            self.toggle_pause();
                        }
                        let app = if self.live.current_app.is_empty() {
                            "no input yet".to_string()
                        } else {
                            self.live.current_app.clone()
                        };
                        ui.label(theme::regular(11.0, app).color(p.muted));
                        score_chip(ui, &theme, self.live.live_score, self.engine.is_paused());
                    });
                });
            });
    }

    fn draw_mini(&mut self, ui: &mut egui::Ui) {
        let theme = self.theme;
        let p = theme.palette;
        egui::CentralPanel::default()
            .frame(
                Frame::NONE
                    .inner_margin(Margin::symmetric(16, 12)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(theme::bold(14.0, "FocusMap").color(p.text));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if small_button(ui, &theme, "Expand") {
                            self.set_mini(false, ui.ctx());
                        }
                    });
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let tint = theme.score_color(self.live.live_score);
                    widgets::ring(
                        ui,
                        &theme,
                        92.0,
                        self.live.live_score as f32 / 100.0,
                        &self.live.live_score.to_string(),
                        "focus",
                        tint,
                    );
                    ui.add_space(10.0);
                    ui.vertical(|ui| {
                        let paused = self.engine.is_paused();
                        let status = if paused {
                            "Paused"
                        } else if self.engine.is_manual() {
                            "Manual focus"
                        } else {
                            "Tracking"
                        };
                        widgets::dot(
                            ui,
                            if paused { p.warn } else { p.good },
                            3.5,
                        );
                        ui.label(theme::bold(12.0, status).color(p.text));
                        ui.label(
                            theme::regular(11.0, format_minutes(self.live.summary.deep_minutes))
                                .color(p.muted),
                        );
                        ui.add_space(4.0);
                        if small_button(ui, &theme, if paused { "Resume" } else { "Pause" }) {
                            self.toggle_pause();
                        }
                    });
                });
            });
    }

    fn draw_toasts(&mut self, ctx: &egui::Context) {
        self.toasts.retain(|t| t.until > Instant::now());
        if self.toasts.is_empty() {
            return;
        }
        let theme = self.theme;
        let p = theme.palette;
        egui::Area::new(egui::Id::unique("toasts"))
            .anchor(Align2::RIGHT_TOP, Vec2::new(-18.0, 76.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_width(322.0);
                for toast in &self.toasts {
                    egui::Frame::NONE
                        .fill(theme::alpha(p.card, 240))
                        .stroke(Stroke::new(1.0, toast.tint.gamma_multiply(0.5)))
                        .corner_radius(12)
                        .inner_margin(Margin::symmetric(14, 11))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                widgets::dot(ui, toast.tint, 3.5);
                                ui.label(theme::bold(12.5, &toast.title).color(p.text));
                            });
                            ui.add_space(3.0);
                            ui.label(theme::regular(11.5, &toast.body).color(p.muted));
                        });
                    ui.add_space(6.0);
                }
            });
        ctx.request_repaint_after(Duration::from_millis(150));
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let (pause, manual, pick) = ctx.input(|i| {
            let keys = [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
                egui::Key::Num6,
                egui::Key::Num7,
            ];
            let pick = if i.modifiers.command || i.modifiers.ctrl {
                keys.iter().position(|k| i.key_pressed(*k))
            } else {
                None
            };
            (
                i.modifiers.ctrl && i.key_pressed(egui::Key::P),
                i.modifiers.ctrl && i.key_pressed(egui::Key::M),
                pick,
            )
        });
        if let Some(index) = pick {
            self.go(Screen::ALL[index]);
        }
        if pause {
            self.toggle_pause();
        }
        if manual {
            self.toggle_manual();
        }
    }
}

impl eframe::App for FocusMapApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.pump(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        theme::paint_background(ui, &self.theme);
        if !self.settings.onboarded {
            self.screen_onboarding(ui);
        } else if self.mini {
            self.draw_mini(ui);
        } else {
            self.draw_sidebar(ui);
            self.draw_topbar(ui);
            egui::CentralPanel::default()
                .frame(
                    Frame::NONE
                        .inner_margin(Margin::symmetric(26, 14)),
                )
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .id_salt("page")
                        .show(ui, |ui| {
                            widgets::center_page(ui, |ui| {
                                ui.add_space(2.0);
                                match self.screen {
                                    Screen::Today => self.screen_today(ui),
                                    Screen::Week => self.screen_week(ui),
                                    Screen::Insights => self.screen_insights(ui),
                                    Screen::Goals => self.screen_goals(ui),
                                    Screen::Share => self.screen_share(ui),
                                    Screen::Privacy => self.screen_privacy(ui),
                                    Screen::Settings => self.screen_settings(ui),
                                }
                                ui.add_space(20.0);
                            });
                        });
                });
            self.shortcuts(ui.ctx());
        }

        let ctx = ui.ctx().clone();
        self.draw_toasts(&ctx);
        if let Some(error) = self.error.take() {
            let tint = self.theme.palette.danger;
            self.toast("Something went wrong", error, tint);
        }
    }
}

// ---- small helpers -------------------------------------------------------

pub(crate) fn reduced(settings: &Settings, system: bool) -> bool {
    match settings.motion {
        MotionMode::Auto => system,
        MotionMode::Full => false,
        MotionMode::Reduced => true,
    }
}

pub(crate) fn nav_item(
    ui: &mut egui::Ui,
    theme: &Theme,
    label: &str,
    hint: &str,
    active: bool,
) -> bool {
    let p = theme.palette;
    let width = ui.available_width();
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 38.0), Sense::click());
    let painter = ui.painter();
    if active {
        theme::gradient_rect(
            painter,
            rect,
            theme::alpha(p.accent, 58),
            theme::alpha(p.accent, 12),
        );
        painter.rect_stroke(
            rect,
            CornerRadius::same(12),
            Stroke::new(1.0, theme::alpha(p.accent, 80)),
            egui::StrokeKind::Inside,
        );
        let bar = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 3.0, rect.center().y - 8.0),
            Vec2::new(3.0, 16.0),
        );
        theme::glow(painter, bar.center(), 14.0, p.accent, 120);
        painter.rect_filled(bar, CornerRadius::same(2), p.accent);
    } else if response.hovered() {
        painter.rect_filled(rect, CornerRadius::same(12), p.card.gamma_multiply(0.55));
    }
    let color = if active { p.text } else { p.muted };
    painter.text(
        egui::pos2(rect.left() + 16.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        if active {
            FontId::new(13.0, theme::bold_family())
        } else {
            FontId::proportional(13.0)
        },
        color,
    );
    painter.text(
        egui::pos2(rect.right() - 10.0, rect.center().y),
        Align2::RIGHT_CENTER,
        format!("^{hint}"),
        FontId::proportional(9.5),
        p.faint,
    );
    response.clicked()
}

fn small_button(ui: &mut egui::Ui, theme: &Theme, text: &str) -> bool {
    let p = theme.palette;
    ui.add(
        egui::Button::new(theme::regular(12.0, text).color(p.text))
            .fill(p.card_alt)
            .stroke(Stroke::new(1.0, p.border))
            .corner_radius(9)
            .min_size(Vec2::new(0.0, 30.0)),
    )
    .clicked()
}

fn score_chip(ui: &mut egui::Ui, theme: &Theme, score: u8, paused: bool) {
    let p = theme.palette;
    let tint = if paused { p.faint } else { theme.score_color(score) };
    egui::Frame::NONE
        .fill(p.card)
        .stroke(Stroke::new(1.0, p.border))
        .corner_radius(100)
        .inner_margin(Margin::symmetric(11, 5))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                widgets::dot(ui, tint, 3.5);
                ui.label(theme::bold(12.0, score.to_string()).color(p.text));
                ui.label(theme::regular(10.0, "focus").color(p.faint));
            });
        });
}

pub(crate) fn open_folder(path: &Path) {
    let _ = std::fs::create_dir_all(path);
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

pub(crate) fn pictures_dir() -> PathBuf {
    home().join("Pictures")
}

/// Map the scoring categories onto the five city districts.
fn district_for(category: AppCategory) -> focusmap_city::District {
    use focusmap_city::District;
    match category {
        AppCategory::Editor | AppCategory::Terminal => District::Coding,
        AppCategory::Chat | AppCategory::Meeting => District::Communication,
        AppCategory::Browser | AppCategory::Other => District::Browsing,
        AppCategory::Media | AppCategory::Game => District::Entertainment,
        AppCategory::Office => District::Creative,
    }
}

fn home() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Seven day keys ending today, for the week screen.
pub(crate) fn last_days(today: &str, count: u32) -> Vec<String> {
    day_range(today, count)
}

/// `42`, `142`, `1.4k` compact numbers for small cards.
pub(crate) fn compact_number(value: u64) -> String {
    if value >= 1000 {
        format!("{:.1}k", value as f64 / 1000.0)
    } else {
        value.to_string()
    }
}

/// `Sat 15 Nov` with a `Today` shortcut.
pub(crate) fn day_label(day: &str, today: &str) -> String {
    if day == today {
        "Today".to_string()
    } else if day == shift_day(today, -1) {
        "Yesterday".to_string()
    } else {
        pretty_day(day)
    }
}

/// Common session list used by Today and the week detail.
impl FocusMapApp {
    pub(crate) fn sessions_block(
        &mut self,
        ui: &mut egui::Ui,
        day: &str,
        sessions: &[Session],
        title: &str,
        limit: usize,
    ) {
        let theme = self.theme;
        let p = theme.palette;
        widgets::card(ui, &theme, |ui| {
            ui.horizontal(|ui| {
                ui.label(theme::bold(15.0, title).color(p.text));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if !sessions.is_empty() {
                        ui.label(
                            theme::regular(10.5, format!("{} sessions", sessions.len()))
                                .color(p.faint),
                        );
                    }
                });
            });
            ui.add_space(2.0);
            ui.label(
                theme::regular(11.0, "Runs of two or more deep minutes. Tags stay with the session.")
                    .color(p.faint),
            );
            ui.add_space(8.0);
            if sessions.is_empty() {
                widgets::empty_state(
                    ui,
                    &theme,
                    "No deep session yet",
                    "A couple of quiet, steady minutes will appear here.",
                );
                return;
            }
            for (index, session) in sessions.iter().take(limit).enumerate() {
                if index > 0 {
                    widgets::divider(ui, &theme);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(theme::bold(13.0, session.time_range()).color(p.text));
                        ui.label(
                            theme::regular(10.5, format!(
                                "{}  ·  avg {}  ·  {}",
                                format_minutes(session.minutes),
                                session.avg_score,
                                if session.top_app.is_empty() {
                                    "unknown app"
                                } else {
                                    &session.top_app
                                }
                            ))
                            .color(p.muted),
                        );
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        self.tag_control(ui, day, session);
                    });
                });
                ui.add_space(6.0);
            }
            if sessions.len() > limit {
                ui.label(
                    theme::regular(11.0, format!("and {} more", sessions.len() - limit))
                        .color(p.faint),
                );
            }
        });
    }

    fn tag_control(&mut self, ui: &mut egui::Ui, day: &str, session: &Session) {
        let theme = self.theme;
        let p = theme.palette;
        let editing = self
            .tag_edit
            .as_ref()
            .map(|e| e.day == day && e.start == session.start_minute)
            .unwrap_or(false);
        if editing {
            let mut save = false;
            let mut cancel = false;
            ui.horizontal(|ui| {
                if let Some(edit) = self.tag_edit.as_mut() {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut edit.text)
                            .desired_width(116.0)
                            .hint_text("Tag this session"),
                    );
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        save = true;
                    }
                }
                if ui.small_button("Save").clicked() {
                    save = true;
                }
                if ui.small_button("Cancel").clicked() {
                    cancel = true;
                }
            });
            if save {
                if let Some(edit) = self.tag_edit.take() {
                    self.save_tag(&edit.day, edit.start, &edit.text);
                }
            } else if cancel {
                self.tag_edit = None;
            }
            return;
        }
        let tint = p.accent;
        let label = session.tag.clone().unwrap_or_else(|| "+ tag".to_string());
        if widgets::tag_button(ui, &theme, &label, tint).clicked() {
            self.tag_edit = Some(TagEdit {
                day: day.to_string(),
                start: session.start_minute,
                text: session.tag.clone().unwrap_or_default(),
            });
        }
    }
}

impl Drop for FocusMapApp {
    fn drop(&mut self) {
        self.ambient.stop();
    }
}
