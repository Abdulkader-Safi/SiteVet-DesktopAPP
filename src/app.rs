//! The window: state, the audit engine and the shell (site list and top bar).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sitevet::{AuditEvent, AuditHandle, AuditReport, ReportDiff, RunId, Status};

use crate::data::{self, AuditSettings, Site, time};
use crate::editor::OptionsEditor;
use crate::theme::{self, ActivePalette, TextStyles};
use crate::ui;

#[derive(Clone, Copy, PartialEq)]
pub enum Screen {
    Report,
    NewAudit,
    Settings,
    Running(usize),
    Batch,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Overview,
    Pages,
    SiteWide,
    Changes,
    History,
}

#[derive(Clone, Copy, PartialEq)]
pub enum DetailTab {
    Findings,
    Measurements,
    Passed,
}

#[derive(Clone, Copy, PartialEq)]
pub enum PageSort {
    Lowest,
    Path,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Range {
    All,
    Days30,
    Days90,
}

#[derive(Clone, Copy, PartialEq)]
pub enum OpenSelect {
    None,
    Metric,
    CompareA,
    CompareB,
    Export,
}

#[derive(Clone, PartialEq)]
pub enum RunStatus {
    Queued,
    Running,
    Saving,
    Done,
    Failed(String),
    Cancelled,
}

/// One audit started from this window, live or finished.
/// One line of a run's activity log.
pub struct LogLine {
    /// Milliseconds since the run started.
    pub at_ms: u64,
    pub level: LogLevel,
    pub text: String,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Fail,
}

/// A page as the crawl fetched it.
pub struct Fetched {
    pub url: String,
    /// 0 when the request itself failed.
    pub status: u16,
    pub ms: u64,
}

/// `03:41` from milliseconds, for log times.
pub fn clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}", s / 60, s % 60)
}

/// Lines kept per run; older ones are dropped so a 10,000-page audit stays small.
const LOG_CAP: usize = 2000;

pub struct LiveRun {
    pub url: String,
    pub settings: AuditSettings,
    pub status: RunStatus,
    pub handle: Option<AuditHandle>,
    pub started: Option<Instant>,
    pub started_clock: String,
    pub elapsed_ms: u64,
    pub max_pages: u32,
    pub fetched: Vec<Fetched>,
    /// When the engine last sent anything, to tell a slow step from a stuck one.
    pub last_event: Option<Instant>,
    pub log: Vec<LogLine>,
    /// Pages whose request failed or answered 4xx or 5xx.
    pub failed_fetches: u32,
    /// Chrome render progress between the crawl and the checks: (done, total).
    pub rendered: Option<(u32, u32)>,
    /// Newest last: (url, score).
    pub completed: Vec<(String, u8)>,
    pub pages_total: Option<u32>,
    pub checks_done: u32,
    pub checks_total: u32,
    pub pass: u32,
    pub warn: u32,
    pub fail: u32,
    pub report: Option<Arc<AuditReport>>,
    /// The site's run count and last score before this run, when it has history.
    pub prev: Option<(usize, u8)>,
}

impl LiveRun {
    pub fn active(&self) -> bool {
        matches!(
            self.status,
            RunStatus::Queued | RunStatus::Running | RunStatus::Saving
        )
    }

    pub fn elapsed(&self) -> u64 {
        match (self.started, &self.status) {
            (Some(t), RunStatus::Running | RunStatus::Saving) => t.elapsed().as_millis() as u64,
            _ => self.elapsed_ms,
        }
    }

    /// Pages done out of the expected total: the crawl's count once known, else the cap.
    pub fn pages(&self) -> (u32, u32) {
        match self.pages_total {
            Some(total) => (self.completed.len() as u32, total),
            None => (self.fetched.len() as u32, self.max_pages),
        }
    }

    pub fn fraction(&self) -> f32 {
        match self.status {
            RunStatus::Done | RunStatus::Saving => 1.,
            RunStatus::Queued => 0.,
            _ if self.checks_total > 0 => self.checks_done as f32 / self.checks_total as f32,
            _ if self.rendered.is_some() => {
                let (done, total) = self.rendered.unwrap_or((0, 1));
                done as f32 / total.max(1) as f32
            }
            // ponytail: during the crawl the total is unknown; pages fetched against the cap
            // is the best guess and jumps once the crawl completes.
            _ => {
                let (done, total) = self.pages();
                done as f32 / total.max(1) as f32
            }
        }
    }

    pub fn log(&mut self, level: LogLevel, text: impl Into<String>) {
        let at_ms = self.started.map_or(0, |t| t.elapsed().as_millis() as u64);
        if self.log.len() >= LOG_CAP {
            self.log.drain(..LOG_CAP / 10);
        }
        self.log.push(LogLine {
            at_ms,
            level,
            text: text.into(),
        });
    }

    /// How long the engine has been silent, while it is running.
    pub fn quiet_ms(&self) -> Option<u64> {
        match self.status {
            RunStatus::Running => self
                .last_event
                .or(self.started)
                .map(|t| t.elapsed().as_millis() as u64),
            _ => None,
        }
    }

    /// The log as plain text, for the clipboard.
    pub fn log_text(&self) -> String {
        let mut out = format!("SiteVet {} · {}\n", env!("CARGO_PKG_VERSION"), self.url);
        for l in &self.log {
            let tag = match l.level {
                LogLevel::Info => "     ",
                LogLevel::Warn => "WARN ",
                LogLevel::Fail => "FAIL ",
            };
            out.push_str(&format!("{} {tag}{}\n", clock(l.at_ms), l.text));
        }
        out
    }

    pub fn run_number(&self) -> usize {
        self.prev.map_or(1, |(n, _)| n + 1)
    }
}

/// What the confirm dialog will delete from history.
#[derive(Clone)]
pub enum Delete {
    Run { id: RunId, origin: String },
    Site { origin: String },
}

pub struct Batch {
    pub runs: Vec<usize>,
    pub parallel: usize,
    pub started: Instant,
    pub started_clock: String,
}

pub struct Form {
    pub several: bool,
    pub start_url: Entity<InputState>,
    pub paste: Entity<InputState>,
    pub sites: Vec<String>,
    /// Starts from the saved defaults; edits here apply to the next audit only.
    pub editor: OptionsEditor,
    pub error: Option<String>,
}

pub struct AppView {
    pub focus: FocusHandle,
    pub sites: Vec<Site>,
    pub sites_error: Option<String>,
    pub selected: Option<String>,
    pub screen: Screen,
    pub tab: Tab,
    pub sidebar: bool,
    pub theme_overridden: bool,
    pub site_search: Entity<InputState>,
    pub category_search: Entity<InputState>,
    pub page_search: Entity<InputState>,
    // Pages tab.
    pub category: Option<String>,
    pub page_sort: PageSort,
    pub page: Option<String>,
    pub detail: DetailTab,
    // History tab.
    pub reports: HashMap<RunId, Arc<AuditReport>>,
    pub loading: HashSet<RunId>,
    /// Runs whose stored report could not be read, with the reason.
    pub load_failed: HashMap<RunId, String>,
    pub range: Range,
    pub metric: Option<String>,
    pub compare: (Option<RunId>, Option<RunId>),
    pub compare_cache: Option<((RunId, RunId), Arc<ReportDiff>)>,
    pub open_select: OpenSelect,
    /// The running screen shows the activity log instead of the page list.
    pub run_log: bool,
    /// Waiting for the user to confirm a delete.
    pub pending_delete: Option<Delete>,
    pub delete_error: Option<String>,
    pub changes_from_history: bool,
    // Audits.
    pub form: Form,
    // Settings.
    /// The saved defaults every new audit starts from.
    pub defaults: AuditSettings,
    pub defaults_editor: OptionsEditor,
    pub settings_note: Option<String>,
    pub settings_error: Option<String>,
    pub runs: Vec<LiveRun>,
    pub batch: Option<Batch>,
    ticking: bool,
    /// A newer release, until the popup is dismissed.
    pub update: Option<crate::update::Update>,
    /// The last export, until its card is closed.
    pub export_status: Option<crate::export::ExportStatus>,
}

pub(crate) fn input(
    window: &mut Window,
    cx: &mut Context<AppView>,
    placeholder: &str,
    value: String,
) -> Entity<InputState> {
    let placeholder = placeholder.to_string();
    let e = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder(placeholder)
            .default_value(value)
    });
    cx.subscribe(&e, |_, _, _: &InputEvent, cx| cx.notify())
        .detach();
    e
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let defaults = data::load_defaults();
        let form = Form {
            several: false,
            start_url: input(window, cx, "https://example.com", String::new()),
            paste: input(window, cx, "Paste URLs, one per line", String::new()),
            sites: Vec::new(),
            editor: OptionsEditor::new(&defaults, window, cx),
            error: None,
        };
        let defaults_editor = OptionsEditor::new(&defaults, window, cx);
        cx.subscribe_in(
            &form.start_url,
            window,
            |this, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    this.run_from_form(window, cx);
                }
            },
        )
        .detach();
        cx.subscribe_in(
            &form.paste,
            window,
            |this, _, ev: &InputEvent, window, cx| {
                if matches!(ev, InputEvent::PressEnter { .. }) {
                    this.add_pasted(window, cx);
                }
            },
        )
        .detach();
        cx.observe_window_appearance(window, |this, window, cx| {
            if !this.theme_overridden {
                theme::set_dark(is_dark(window.appearance()), cx);
            }
        })
        .detach();

        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let mut this = Self {
            focus,
            sites: Vec::new(),
            sites_error: None,
            selected: None,
            screen: Screen::NewAudit,
            tab: Tab::Pages,
            sidebar: true,
            theme_overridden: false,
            site_search: input(window, cx, "Search pages", String::new()),
            category_search: input(window, cx, "Search categories", String::new()),
            page_search: input(window, cx, "Search pages", String::new()),
            category: None,
            page_sort: PageSort::Lowest,
            page: None,
            detail: DetailTab::Findings,
            reports: HashMap::new(),
            loading: HashSet::new(),
            load_failed: HashMap::new(),
            range: Range::All,
            metric: None,
            compare: (None, None),
            compare_cache: None,
            open_select: OpenSelect::None,
            run_log: false,
            pending_delete: None,
            delete_error: None,
            changes_from_history: false,
            form,
            defaults,
            defaults_editor,
            settings_note: None,
            settings_error: None,
            runs: Vec::new(),
            batch: None,
            ticking: false,
            update: None,
            export_status: None,
        };
        this.reload_sites(None, cx);
        let check = cx.background_spawn(async { crate::update::check() });
        cx.spawn(async move |this, cx| {
            if let Some(update) = check.await {
                let _ = this.update(cx, |app, cx| {
                    app.update = Some(update);
                    cx.notify();
                });
            }
        })
        .detach();
        this
    }

    // ---- Sites and selection ----

    pub fn reload_sites(&mut self, select: Option<String>, cx: &mut Context<Self>) {
        let task = cx.background_spawn(async { data::load_sites() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| {
                match result {
                    Ok(sites) => {
                        app.sites = sites;
                        app.sites_error = None;
                        for s in &app.sites {
                            app.reports.insert(s.latest_run().id, s.latest.clone());
                        }
                        // Only on a fresh start: never pull the user off a screen they moved to.
                        let first = app.selected.is_none()
                            && select.is_none()
                            && app.screen == Screen::NewAudit
                            && app.runs.is_empty();
                        if let Some(origin) = select {
                            app.open_site(&origin, cx);
                        } else if first
                            && let Some(origin) = app.sites.first().map(|s| s.origin.clone())
                        {
                            app.open_site(&origin, cx);
                        }
                    }
                    Err(e) => app.sites_error = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn site(&self) -> Option<&Site> {
        let origin = self.selected.as_deref()?;
        self.sites.iter().find(|s| s.origin == origin)
    }

    pub fn open_site(&mut self, origin: &str, cx: &mut Context<Self>) {
        if self.selected.as_deref() != Some(origin) {
            self.category = None;
            self.page = None;
            self.detail = DetailTab::Findings;
            self.compare = (None, None);
            self.compare_cache = None;
            self.changes_from_history = false;
            self.metric = None;
        }
        self.selected = Some(origin.to_string());
        self.screen = Screen::Report;
        if self.tab == Tab::History {
            self.load_history(cx);
        }
        cx.notify();
    }

    /// Loads every stored report of the selected site, for the history chart and compare.
    pub fn load_history(&mut self, cx: &mut Context<Self>) {
        let Some(site) = self.site() else { return };
        let missing: Vec<RunId> = site
            .runs
            .iter()
            .map(|r| r.id)
            .filter(|id| {
                !self.reports.contains_key(id)
                    && !self.loading.contains(id)
                    && !self.load_failed.contains_key(id)
            })
            .collect();
        if missing.is_empty() {
            return;
        }
        self.loading.extend(missing.iter().copied());
        let task = cx.background_spawn(async move {
            missing
                .into_iter()
                .map(|id| (id, data::load_report(id)))
                .collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = this.update(cx, |app, cx| {
                for (id, result) in loaded {
                    app.loading.remove(&id);
                    match result {
                        Ok(r) => {
                            app.reports.insert(id, Arc::new(r));
                        }
                        Err(e) => {
                            app.load_failed.insert(id, e);
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn set_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        self.open_select = OpenSelect::None;
        if tab == Tab::History {
            // Runs that failed to load get another try each time History opens.
            self.load_failed.clear();
            self.load_history(cx);
        }
        if tab != Tab::Changes {
            self.changes_from_history = false;
        }
        cx.notify();
    }

    /// The active run for a site (by origin), if any.
    pub fn run_for(&self, origin: &str) -> Option<usize> {
        self.runs
            .iter()
            .rposition(|r| r.active() && data::origin_of(&r.url) == origin)
    }

    /// The history site a URL belongs to.
    pub fn site_for(&self, url: &str) -> Option<&Site> {
        let origin = data::origin_of(url);
        self.sites.iter().find(|s| s.origin == origin)
    }

    pub fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme_overridden = true;
        let dark = !cx.global::<theme::Appearance>().dark;
        theme::set_dark(dark, cx);
        cx.refresh_windows();
    }

    // ---- New audit form ----

    pub fn new_audit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.screen = Screen::NewAudit;
        self.form.error = None;
        if !self.form.several && self.form.start_url.read(cx).value().is_empty() {
            let url = self
                .site()
                .map(|s| s.latest.start_url.clone())
                .unwrap_or_default();
            self.form
                .start_url
                .update(cx, |s, cx| s.set_value(url, window, cx));
        }
        cx.notify();
    }

    pub fn add_pasted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.form.paste.read(cx).value().to_string();
        for url in data::parse_urls(&text) {
            if !self.form.sites.contains(&url) {
                self.form.sites.push(url);
            }
        }
        self.form
            .paste
            .update(cx, |s, cx| s.set_value("", window, cx));
        cx.notify();
    }

    /// Puts the saved defaults back into the new audit form.
    pub fn reset_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let defaults = self.defaults.clone();
        self.form.editor.load(&defaults, window, cx);
        self.form.error = None;
        cx.notify();
    }

    pub fn run_from_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = match self.form.editor.read_with(self.form.several, cx) {
            Ok(s) => s,
            Err(e) => {
                self.form.error = Some(e);
                cx.notify();
                return;
            }
        };
        self.form.error = None;
        let started = if self.form.several {
            let urls = self.form.sites.clone();
            if urls.is_empty() {
                self.form.error = Some("Add at least one site".into());
                false
            } else {
                self.start_batch(urls, settings, cx);
                true
            }
        } else {
            let url = self.form.start_url.read(cx).value().trim().to_string();
            match data::parse_urls(&url).into_iter().next() {
                Some(url) => {
                    self.start_one(url, settings, cx);
                    true
                }
                None => {
                    self.form.error = Some("Enter a start URL".into());
                    false
                }
            }
        };
        // Values changed for this audit do not carry over to the next one.
        if started {
            self.reset_form(window, cx);
        }
        cx.notify();
    }

    /// "Run audit again" from history: the saved defaults on the site's start URL.
    pub fn rerun(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.site().map(|s| s.latest.start_url.clone()) else {
            return;
        };
        self.start_one(url, self.defaults.clone(), cx);
    }

    // ---- Deleting from history ----

    pub fn ask_delete(&mut self, what: Delete, cx: &mut Context<Self>) {
        self.delete_error = None;
        self.pending_delete = Some(what);
        cx.notify();
    }

    /// The runs a pending delete covers.
    pub fn delete_ids(&self, what: &Delete) -> Vec<RunId> {
        match what {
            Delete::Run { id, .. } => vec![*id],
            Delete::Site { origin } => self
                .sites
                .iter()
                .find(|s| &s.origin == origin)
                .map(|s| s.runs.iter().map(|r| r.id).collect())
                .unwrap_or_default(),
        }
    }

    pub fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let Some(what) = self.pending_delete.clone() else {
            return;
        };
        let ids = self.delete_ids(&what);
        let task = cx.background_spawn({
            let ids = ids.clone();
            async move { data::delete_runs(&ids) }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| match result {
                Ok(_) => {
                    app.pending_delete = None;
                    app.forget_runs(&ids, &what);
                    app.reload_sites(None, cx);
                }
                Err(e) => {
                    app.delete_error = Some(e);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Drops everything cached about deleted runs, and leaves a deleted site's screen.
    fn forget_runs(&mut self, ids: &[RunId], what: &Delete) {
        for id in ids {
            self.reports.remove(id);
            self.load_failed.remove(id);
        }
        let gone = |id: &Option<RunId>| id.is_some_and(|id| ids.contains(&id));
        if gone(&self.compare.0) {
            self.compare.0 = None;
        }
        if gone(&self.compare.1) {
            self.compare.1 = None;
        }
        self.compare_cache = None;
        self.changes_from_history = false;
        let site_gone = match what {
            Delete::Site { origin } => Some(origin.clone()),
            // The last run of a site takes the site with it.
            Delete::Run { origin, .. } => self
                .sites
                .iter()
                .find(|s| &s.origin == origin)
                .filter(|s| s.runs.iter().all(|r| ids.contains(&r.id)))
                .map(|s| s.origin.clone()),
        };
        if let Some(origin) = site_gone {
            self.sites.retain(|s| s.origin != origin);
            if self.selected.as_deref() == Some(origin.as_str()) {
                self.selected = None;
                match self.sites.first().map(|s| s.origin.clone()) {
                    Some(next) => {
                        self.selected = Some(next);
                        self.screen = Screen::Report;
                    }
                    None => self.screen = Screen::NewAudit,
                }
            }
        }
    }

    // ---- Settings ----

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Show what is saved, not edits left from an earlier visit.
        let defaults = self.defaults.clone();
        self.defaults_editor.load(&defaults, window, cx);
        self.settings_note = None;
        self.settings_error = None;
        self.screen = Screen::Settings;
        cx.notify();
    }

    pub fn save_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.defaults_editor.read(cx) {
            Ok(s) => match data::save_defaults(&s) {
                Ok(()) => {
                    // The form follows unless it holds values changed for one audit.
                    let form_custom = self.form.editor.differs_from(&self.defaults, cx);
                    self.defaults = s;
                    if !form_custom {
                        self.reset_form(window, cx);
                    }
                    self.settings_error = None;
                    self.settings_note = Some("SAVED · NEW AUDITS START FROM THESE".into());
                    self.form.error = None;
                }
                Err(e) => self.settings_error = Some(format!("Could not save: {e}")),
            },
            // The settings screen shows field errors live from the fields themselves.
            Err(_) => self.settings_error = None,
        }
        cx.notify();
    }

    /// Fills the defaults editor with sitevet's own defaults. Nothing is saved until Save.
    pub fn sdk_defaults(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.defaults_editor
            .load(&AuditSettings::default(), window, cx);
        self.settings_note = None;
        self.settings_error = None;
        cx.notify();
    }

    // ---- The audit engine ----

    fn new_run(&self, url: String, settings: AuditSettings) -> LiveRun {
        let prev = self.site_for(&url).map(|s| (s.runs.len(), s.latest.score));
        LiveRun {
            max_pages: settings.options.max_pages,
            url,
            settings,
            status: RunStatus::Queued,
            handle: None,
            started: None,
            started_clock: String::new(),
            elapsed_ms: 0,
            fetched: Vec::new(),
            last_event: None,
            log: Vec::new(),
            failed_fetches: 0,
            rendered: None,
            completed: Vec::new(),
            pages_total: None,
            checks_done: 0,
            checks_total: 0,
            pass: 0,
            warn: 0,
            fail: 0,
            report: None,
            prev,
        }
    }

    fn start_one(&mut self, url: String, settings: AuditSettings, cx: &mut Context<Self>) {
        let idx = self.runs.len();
        self.runs.push(self.new_run(url, settings));
        self.start_run(idx, cx);
        self.screen = Screen::Running(idx);
        cx.notify();
    }

    fn start_batch(&mut self, urls: Vec<String>, settings: AuditSettings, cx: &mut Context<Self>) {
        let parallel = settings.sites_at_a_time;
        let first = self.runs.len();
        for url in urls {
            let run = self.new_run(url, settings.clone());
            self.runs.push(run);
        }
        let new: Vec<usize> = (first..self.runs.len()).collect();
        match &mut self.batch {
            // Sites still queued in the earlier batch would otherwise never start.
            Some(b) if b.runs.iter().any(|&i| self.runs[i].active()) => {
                b.runs.extend(new);
                b.parallel = parallel;
            }
            _ => {
                self.batch = Some(Batch {
                    runs: new,
                    parallel,
                    started: Instant::now(),
                    started_clock: time::now_clock(),
                })
            }
        }
        self.screen = Screen::Batch;
        self.pump_batch(cx);
    }

    /// Starts queued batch runs until `parallel` are in flight.
    fn pump_batch(&mut self, cx: &mut Context<Self>) {
        let Some(batch) = &self.batch else { return };
        let running = batch
            .runs
            .iter()
            .filter(|&&i| matches!(self.runs[i].status, RunStatus::Running))
            .count();
        let next: Vec<usize> = batch
            .runs
            .iter()
            .copied()
            .filter(|&i| self.runs[i].status == RunStatus::Queued)
            .take(batch.parallel.saturating_sub(running))
            .collect();
        for i in next {
            self.start_run(i, cx);
        }
    }

    pub fn retry(&mut self, idx: usize, cx: &mut Context<Self>) {
        let run = self.new_run(self.runs[idx].url.clone(), self.runs[idx].settings.clone());
        self.runs[idx] = run;
        self.pump_batch(cx);
        cx.notify();
    }

    pub fn remove_queued(&mut self, idx: usize, cx: &mut Context<Self>) {
        if let Some(b) = &mut self.batch {
            b.runs.retain(|&i| i != idx);
        }
        self.runs[idx].status = RunStatus::Cancelled;
        cx.notify();
    }

    pub fn cancel(&mut self, idx: usize, cx: &mut Context<Self>) {
        let run = &mut self.runs[idx];
        // A run that is saving has finished; its report is going into history either way.
        if matches!(run.status, RunStatus::Queued | RunStatus::Running) {
            if let Some(h) = run.handle.take() {
                h.cancel();
            }
            run.elapsed_ms = run.elapsed();
            run.status = RunStatus::Cancelled;
            run.log(LogLevel::Warn, "Cancelled");
        }
        self.pump_batch(cx);
        cx.notify();
    }

    pub fn cancel_all(&mut self, cx: &mut Context<Self>) {
        let ids = self
            .batch
            .as_ref()
            .map(|b| b.runs.clone())
            .unwrap_or_default();
        for i in ids {
            self.cancel(i, cx);
        }
    }

    fn start_run(&mut self, idx: usize, cx: &mut Context<Self>) {
        let run = &mut self.runs[idx];
        let handle = sitevet::spawn(&run.url, run.settings.options_for_run());
        let rx = handle.events();
        run.handle = Some(handle);
        run.status = RunStatus::Running;
        run.started = Some(Instant::now());
        let o = &run.settings.options;
        let rendered = if o.browser { "on" } else { "off" };
        let line = format!(
            "Options: up to {} pages, depth {}, {} requests at a time, rendered checks {rendered}",
            o.max_pages, o.max_depth, o.concurrency
        );
        run.log(LogLevel::Info, line);
        run.started_clock = time::now_clock();
        cx.spawn(async move |this, cx| {
            while let Ok(first) = rx.recv().await {
                // Drain what is already queued so thousands of check events cost one update.
                let mut events = vec![first];
                while events.len() < 512
                    && let Ok(ev) = rx.try_recv()
                {
                    events.push(ev);
                }
                let alive = this.update(cx, |app, cx| {
                    for ev in events {
                        app.on_event(idx, ev, cx);
                    }
                    cx.notify();
                });
                if alive.is_err() {
                    return;
                }
            }
            let _ = this.update(cx, |app, cx| app.on_events_closed(idx, cx));
        })
        .detach();
        self.ensure_ticker(cx);
    }

    fn on_event(&mut self, idx: usize, ev: AuditEvent, cx: &mut Context<Self>) {
        let run = &mut self.runs[idx];
        if run.status != RunStatus::Running {
            return;
        }
        run.last_event = Some(Instant::now());
        match ev {
            AuditEvent::AuditStart { url, max_pages } => {
                run.max_pages = max_pages;
                run.log(
                    LogLevel::Info,
                    format!("Started {url}, up to {max_pages} pages"),
                );
            }
            AuditEvent::PageFetched {
                url,
                status,
                depth,
                ms,
                ..
            } => {
                let level = match status {
                    0 => LogLevel::Fail,
                    400.. => LogLevel::Warn,
                    _ => LogLevel::Info,
                };
                if level != LogLevel::Info {
                    run.failed_fetches += 1;
                }
                let code = if status == 0 {
                    "ERR".to_string()
                } else {
                    status.to_string()
                };
                run.log(
                    level,
                    format!(
                        "Fetched {code} {} ({ms} ms, depth {depth})",
                        data::path_of(&url)
                    ),
                );
                run.fetched.push(Fetched { url, status, ms });
            }
            AuditEvent::PageRendered {
                url,
                ok,
                index,
                pages,
            } => {
                run.rendered = Some((index, pages));
                let level = if ok { LogLevel::Info } else { LogLevel::Warn };
                let what = if ok { "Rendered" } else { "Could not render" };
                run.log(
                    level,
                    format!("{what} {} in Chrome ({index}/{pages})", data::path_of(&url)),
                );
            }
            AuditEvent::RenderComplete {
                provider,
                rendered,
                pages,
                ms,
            } => {
                run.rendered = Some((rendered, pages));
                run.log(
                    LogLevel::Info,
                    format!(
                        "Rendering done with {provider}: {rendered} of {pages} pages in {}",
                        data::duration_label(ms)
                    ),
                );
            }
            AuditEvent::CrawlComplete {
                pages,
                checks_total,
                ms,
            } => {
                run.pages_total = Some(pages);
                run.checks_total = checks_total;
                run.log(
                    LogLevel::Info,
                    format!(
                        "Crawl done: {pages} pages in {}. Running {} checks",
                        data::duration_label(ms),
                        data::thousands(checks_total.into())
                    ),
                );
            }
            AuditEvent::CheckComplete {
                status,
                checks_done,
                checks_total,
                ..
            } => {
                run.checks_done = checks_done;
                run.checks_total = checks_total;
                match status {
                    Status::Pass => run.pass += 1,
                    Status::Warn => run.warn += 1,
                    Status::Fail => run.fail += 1,
                    _ => {}
                }
            }
            AuditEvent::PageComplete {
                url,
                score,
                findings,
                ms,
                ..
            } => {
                run.log(
                    LogLevel::Info,
                    format!(
                        "Checked {}: score {score}, {findings} findings ({ms} ms)",
                        data::path_of(&url)
                    ),
                );
                run.completed.push((url, score));
            }
            AuditEvent::AuditComplete { report, score, .. } => {
                run.log(
                    LogLevel::Info,
                    format!("Audit done: score {score}. Saving to history"),
                );
                run.status = RunStatus::Saving;
                run.elapsed_ms = run.elapsed();
                run.handle = None;
                let compare = run.settings.compare_with_last;
                let task = cx.background_spawn(async move {
                    let mut report = *report;
                    let saved = data::save_report(&mut report, compare);
                    (report, saved)
                });
                cx.spawn(async move |this, cx| {
                    let (report, saved) = task.await;
                    let _ = this.update(cx, |app, cx| app.on_saved(idx, report, saved, cx));
                })
                .detach();
            }
            _ => {}
        }
    }

    /// The event channel closed. Without an `AuditComplete` that means the audit failed;
    /// the handle says why.
    fn on_events_closed(&mut self, idx: usize, cx: &mut Context<Self>) {
        if self.runs[idx].status != RunStatus::Running {
            return;
        }
        let Some(handle) = self.runs[idx].handle.take() else {
            return;
        };
        cx.spawn(async move |this, cx| {
            let err = handle
                .report()
                .await
                .err()
                .map_or_else(|| "The audit stopped".to_string(), |e| e.to_string());
            let _ = this.update(cx, |app, cx| {
                let run = &mut app.runs[idx];
                if run.status == RunStatus::Running {
                    run.elapsed_ms = run.elapsed();
                    run.log(LogLevel::Fail, format!("Stopped: {err}"));
                    run.status = RunStatus::Failed(err);
                }
                app.pump_batch(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn on_saved(
        &mut self,
        idx: usize,
        report: AuditReport,
        saved: Result<RunId, String>,
        cx: &mut Context<Self>,
    ) {
        let origin = report.start_url.clone();
        let run = &mut self.runs[idx];
        run.report = Some(Arc::new(report));
        run.status = match saved {
            Ok(_) => {
                run.log(LogLevel::Info, "Saved to history");
                RunStatus::Done
            }
            Err(e) => {
                run.log(LogLevel::Fail, format!("Saving to history failed: {e}"));
                RunStatus::Failed(format!("Finished, but saving to history failed: {e}"))
            }
        };
        let open = self.screen == Screen::Running(idx);
        if open {
            self.tab = Tab::Pages;
        }
        // Select the site once history reloads, so the left pane and report show the new run.
        let select = open.then(|| data::origin_of(&origin));
        let task = cx.background_spawn(async { data::load_sites() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |app, cx| {
                if let Ok(sites) = result {
                    app.sites = sites;
                    for s in &app.sites {
                        app.reports.insert(s.latest_run().id, s.latest.clone());
                    }
                }
                if let Some(origin) = select
                    && app.sites.iter().any(|s| s.origin == origin)
                {
                    // Fresh selection so the report opens on its worst page.
                    app.selected = None;
                    app.open_site(&origin, cx);
                }
                cx.notify();
            });
        })
        .detach();
        self.pump_batch(cx);
        cx.notify();
    }

    /// Repaints a few times a second while audits run, so elapsed times tick.
    fn ensure_ticker(&mut self, cx: &mut Context<Self>) {
        if self.ticking {
            return;
        }
        self.ticking = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let keep = this
                    .update(cx, |app, cx| {
                        cx.notify();
                        app.runs
                            .iter()
                            .any(|r| matches!(r.status, RunStatus::Running | RunStatus::Saving))
                    })
                    .unwrap_or(false);
                if !keep {
                    let _ = this.update(cx, |app, _| app.ticking = false);
                    return;
                }
            }
        })
        .detach();
    }

    // ---- Shell ----

    fn on_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // Shortcuts only when no field has focus.
        if !self.focus.is_focused(window) {
            if ev.keystroke.key == "escape" {
                window.focus(&self.focus, cx);
                self.open_select = OpenSelect::None;
                self.pending_delete = None;
                cx.notify();
            }
            return;
        }
        let m = &ev.keystroke.modifiers;
        if m.platform || m.control || m.alt || m.function {
            return;
        }
        if ev.keystroke.key == "escape" && self.pending_delete.is_some() {
            self.pending_delete = None;
            cx.notify();
            return;
        }
        let target = match ev.keystroke.key.as_str() {
            "p" if self.sidebar => Some(&self.site_search),
            "c" if self.screen == Screen::Report && self.tab == Tab::Pages => {
                Some(&self.category_search)
            }
            "/" if self.screen == Screen::Report && self.tab == Tab::Pages => {
                Some(&self.page_search)
            }
            _ => None,
        };
        if let Some(input) = target {
            input.update(cx, |s, cx| s.focus(window, cx));
            cx.stop_propagation();
        }
    }

    fn site_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal();
        let query = self.site_search.read(cx).value().to_lowercase();
        let new_audit = self.screen == Screen::NewAudit;
        let button = if new_audit {
            ui::button_primary("new-audit", "+  New audit", 32., &p)
        } else {
            ui::button_secondary("new-audit", "+  New audit", 32., &p)
        };

        // History sites, then sites being audited for the first time.
        let mut rows: Vec<AnyElement> = Vec::new();
        for (i, site) in self.sites.iter().enumerate() {
            let host = site.host().to_string();
            if !query.is_empty() && !host.contains(&query) {
                continue;
            }
            let run = self.run_for(&site.origin);
            let selected = match self.screen {
                Screen::Running(r) => data::origin_of(&self.runs[r].url) == site.origin,
                Screen::Report => self.selected.as_deref() == Some(site.origin.as_str()),
                _ => false,
            };
            let origin = site.origin.clone();
            let group = SharedString::from(format!("site-row-{i}"));
            let row = match run {
                Some(r) => self.running_row(r, &host, &p),
                None => {
                    let issues = data::site_issues(&site.latest);
                    let del = site.origin.clone();
                    let right = div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(6.))
                        .child(
                            ui::trash_button(("del-site", i), group.clone(), false, &p).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    cx.stop_propagation();
                                    this.ask_delete(
                                        Delete::Site {
                                            origin: del.clone(),
                                        },
                                        cx,
                                    );
                                }),
                            ),
                        )
                        .child(ui::score_chip(site.latest.score, &p));
                    site_row(&host, data::issues_label(&issues), p.muted, right, &p)
                }
            };
            rows.push(
                row.group(group)
                    .id(("site", i))
                    .when(selected, |d| d.bg(p.bg_raised).child(ui::edge(&p)))
                    .on_click(
                        cx.listener(move |this, _, _, cx| match this.run_for(&origin) {
                            Some(r) => {
                                this.screen = Screen::Running(r);
                                cx.notify();
                            }
                            None => this.open_site(&origin, cx),
                        }),
                    )
                    .into_any_element(),
            );
        }
        let known: HashSet<&str> = self.sites.iter().map(|s| s.origin.as_str()).collect();
        let mut shown = HashSet::new();
        for (i, run) in self.runs.iter().enumerate().rev() {
            let origin = data::origin_of(&run.url);
            let host = data::host_of(&run.url).to_string();
            if !run.active() || known.contains(origin.as_str()) || !shown.insert(origin) {
                continue;
            }
            if !query.is_empty() && !host.contains(&query) {
                continue;
            }
            let selected = self.screen == Screen::Running(i);
            rows.push(
                self.running_row(i, &host, &p)
                    .id(("run", i))
                    .when(selected, |d| d.bg(p.bg_raised).child(ui::edge(&p)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.screen = Screen::Running(i);
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(261.))
            .h_full()
            .border_r_1()
            .border_color(p.border)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(10.))
                    .h(px(53.))
                    .px(px(20.))
                    .bg(p.bg_sunken)
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .font_family(theme::BRAND)
                            .text_size(px(15.))
                            .line_height(px(20.))
                            .text_color(p.text)
                            .child("SITEVET"),
                    )
                    .child(ui::spacer())
                    .child(
                        div()
                            .mono_small()
                            .text_color(p.muted)
                            .child(format!("V{}", env!("CARGO_PKG_VERSION"))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .p(px(12.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(ui::search_box(&self.site_search, "p", window, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .p(px(12.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        button.flex_1().on_click(
                            cx.listener(|this, _, window, cx| this.new_audit(window, cx)),
                        ),
                    ),
            )
            .child(
                div()
                    .id("site-list")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(rows)
                    .when_some(self.sites_error.clone(), |d, e| {
                        d.child(
                            div()
                                .p(px(16.))
                                .small()
                                .text_color(p.fail)
                                .child(format!("History could not be read: {e}")),
                        )
                    }),
            )
    }

    fn running_row(&self, idx: usize, host: &str, p: &theme::Palette) -> Div {
        let run = &self.runs[idx];
        let (done, total) = run.pages();
        let (sub, right) = match run.status {
            RunStatus::Queued => ("QUEUED".to_string(), String::new()),
            _ => (
                format!("RUNNING · {done} / {total}"),
                format!("{:.0}%", run.fraction() * 100.),
            ),
        };
        site_row(
            host,
            sub,
            p.accent_text,
            div().mono_small().text_color(p.accent_text).child(right),
            p,
        )
    }

    fn top_bar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal();
        let dark = cx.global::<theme::Appearance>().dark;
        let mut bar = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .h(px(53.))
            .px(px(20.))
            .bg(p.bg_sunken)
            .border_b_1()
            .border_color(p.border)
            .child(
                div()
                    .id("sidebar")
                    .flex_none()
                    .cursor_pointer()
                    .child(ui::icon("icons/layout-sidebar.svg", 28., 28., p.muted))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.sidebar = !this.sidebar;
                        cx.notify();
                    })),
            );
        let title: String;
        let mut right: Option<String> = None;
        match self.screen {
            Screen::NewAudit => title = "New audit".into(),
            Screen::Settings => title = "Settings".into(),
            Screen::Running(i) => {
                let run = &self.runs[i];
                title = run.url.clone();
                right = Some(format!(
                    "RUN {} · STARTED {}",
                    run.run_number(),
                    run.started_clock
                ));
            }
            Screen::Batch => {
                let b = self.batch.as_ref();
                title = format!("{} sites", b.map_or(0, |b| b.runs.len()));
                right = b.map(|b| format!("STARTED {}", b.started_clock));
            }
            Screen::Report => {
                let site = self.site();
                title = site.map(|s| s.latest.start_url.clone()).unwrap_or_default();
                right = site.map(|s| match self.tab {
                    Tab::History => {
                        format!(
                            "{} RUNS · SINCE {}",
                            s.runs.len(),
                            time::date(&s.runs[0].started_at)
                        )
                    }
                    _ => format!(
                        "RUN {} · {}",
                        s.runs.len(),
                        time::date_time(&s.latest_run().started_at)
                    ),
                });
            }
        }
        bar = bar
            .child(
                div()
                    .mono_small()
                    .text_color(p.muted)
                    .whitespace_nowrap()
                    .child(title),
            )
            .child(ui::spacer());
        if self.screen == Screen::Report && self.site().is_some() {
            let tabs = [
                (Tab::Overview, "Overview"),
                (Tab::Pages, "Pages"),
                (Tab::SiteWide, "Site-wide"),
                (Tab::Changes, "Changes"),
                (Tab::History, "History"),
            ];
            bar = bar.child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(16.))
                    .children(tabs.into_iter().map(|(tab, text)| {
                        ui::top_tab(text, text, self.tab == tab, &p)
                            .on_click(cx.listener(move |this, _, _, cx| this.set_tab(tab, cx)))
                    }))
                    .children(right.take().map(|r| ui::micro(r, p.muted))),
            );
            bar = bar.child(self.export_button(cx));
        }
        let on_settings = self.screen == Screen::Settings;
        bar.children(right.map(|r| ui::micro(r, p.muted)))
            .child(
                div()
                    .id("settings")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .cursor_pointer()
                    .when(on_settings, |d| d.bg(p.bg_raised))
                    .hover(|s| s.bg(p.bg_raised))
                    .child(ui::icon(
                        "icons/settings.svg",
                        16.,
                        16.,
                        if on_settings { p.text } else { p.muted },
                    ))
                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
            )
            .child(
                div()
                    .id("theme")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .cursor_pointer()
                    .hover(|s| s.bg(p.bg_raised))
                    .child(ui::icon(
                        if dark {
                            "icons/sun.svg"
                        } else {
                            "icons/moon.svg"
                        },
                        16.,
                        16.,
                        p.muted,
                    ))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_theme(cx))),
            )
    }
}

fn site_row(host: &str, sub: String, sub_color: Hsla, right: Div, p: &theme::Palette) -> Div {
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.))
        .px(px(16.))
        .py(px(10.))
        .border_b_1()
        .border_color(p.border)
        .cursor_pointer()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .overflow_hidden()
                .child(
                    div()
                        .mono_small()
                        .text_color(p.text)
                        .truncate()
                        .child(host.to_string()),
                )
                .child(
                    div()
                        .mono_micro()
                        .text_color(sub_color)
                        .truncate()
                        .child(sub),
                ),
        )
        .child(right)
}

pub fn is_dark(a: WindowAppearance) -> bool {
    matches!(a, WindowAppearance::Dark | WindowAppearance::VibrantDark)
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal();
        let body: AnyElement = match self.screen {
            Screen::NewAudit => self.render_new_audit(window, cx).into_any_element(),
            Screen::Settings => self.render_settings(window, cx).into_any_element(),
            Screen::Running(i) => self.render_running(i, cx).into_any_element(),
            Screen::Batch => self.render_batch(cx).into_any_element(),
            Screen::Report => match self.site().is_some() {
                true => self.render_report(window, cx),
                false => div().into_any_element(),
            },
        };
        div()
            .id("app")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    // Clicking empty space leaves any field, so the shortcut keys work again.
                    if this.open_select != OpenSelect::None {
                        this.open_select = OpenSelect::None;
                        cx.notify();
                    }
                    let _ = window;
                }),
            )
            .flex()
            .size_full()
            .bg(p.bg_app)
            .text_color(p.text)
            .font_family(theme::SANS)
            .relative()
            .when(self.sidebar, |d| d.child(self.site_pane(window, cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .child(self.top_bar(cx))
                    .child(div().flex().flex_col().flex_1().min_h_0().child(body)),
            )
            // Cards in the bottom-right corner, newest on top.
            .child(
                div()
                    .absolute()
                    .right(px(16.))
                    .bottom(px(16.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .children(self.export_card(cx))
                    .children(self.update_popup(cx)),
            )
            .children(self.delete_dialog(cx))
    }
}

impl AppView {
    /// Confirms a delete from history. The CLI shares this history, so it says so.
    fn delete_dialog(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let what = self.pending_delete.as_ref()?;
        let p = cx.pal();
        let ids = self.delete_ids(what);
        let n = ids.len();
        let (title, body, button) = match what {
            Delete::Site { origin } => (
                format!("Delete {}?", data::host_of(origin)),
                format!(
                    "This removes all {n} run{} of this site from history, with their reports.",
                    if n == 1 { "" } else { "s" }
                ),
                if n == 1 {
                    "Delete 1 run".to_string()
                } else {
                    format!("Delete {n} runs")
                },
            ),
            Delete::Run { id, origin } => {
                let site = self.sites.iter().find(|s| &s.origin == origin);
                let number = site.and_then(|s| s.run_number(*id)).unwrap_or(0);
                let summary = site.and_then(|s| s.runs.iter().find(|r| r.id == *id));
                let when = summary.map_or(String::new(), |r| {
                    format!(" ({}, score {})", time::date_time(&r.started_at), r.score)
                });
                let last = site.is_some_and(|s| s.runs.len() == 1);
                (
                    format!("Delete run {number} of {}?", data::host_of(origin)),
                    if last {
                        format!(
                            "This removes run {number}{when} and its report. It is the site's only run, so the site leaves the list too."
                        )
                    } else {
                        format!(
                            "This removes run {number}{when} and its report from history. The site's other runs stay."
                        )
                    },
                    "Delete run".to_string(),
                )
            }
        };
        let scrim = if cx.global::<theme::Appearance>().dark {
            hsla(0., 0., 0., 0.6)
        } else {
            hsla(240. / 360., 0.05, 0.04, 0.3)
        };
        Some(
            div()
                .id("delete-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(scrim)
                .occlude()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.pending_delete = None;
                    cx.notify();
                }))
                .child(
                    div()
                        .id("delete-dialog")
                        .w(px(460.))
                        .flex()
                        .flex_col()
                        .bg(p.bg_surface)
                        .border_1()
                        .border_color(p.border_strong)
                        // Clicks inside the dialog do not reach the scrim.
                        .on_click(|_, _, cx| cx.stop_propagation())
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(10.))
                                .p(px(20.))
                                .child(div().h2().text_color(p.text).child(title))
                                .child(div().body().text_color(p.text).child(body))
                                .child(div().small().text_color(p.muted).child(
                                    "The sitevet command-line tool reads the same history, so the runs are removed there too. This cannot be undone.",
                                ))
                                .when_some(self.delete_error.clone(), |d, e| {
                                    d.child(div().small().text_color(p.fail).child(format!("Could not delete: {e}")))
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(10.))
                                .px(px(20.))
                                .py(px(14.))
                                .bg(p.bg_sunken)
                                .border_t_1()
                                .border_color(p.border)
                                .child(ui::micro("ESC TO CANCEL", p.muted))
                                .child(ui::spacer())
                                .child(ui::button_secondary("delete-cancel", "Cancel", 36., &p).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.pending_delete = None;
                                        cx.notify();
                                    },
                                )))
                                .child(ui::button_danger("delete-confirm", button, 36., &p).on_click(cx.listener(
                                    |this, _, _, cx| this.confirm_delete(cx),
                                ))),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Bottom-right card when GitHub has a newer release. Download opens the installer for
    /// this OS in the browser; Later hides the card until the next start.
    fn update_popup(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let update = self.update.clone()?;
        let p = cx.pal();
        let url = update.url.clone();
        Some(
            ui::toast_card(&p)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::marker(p.accent_bg, 6.))
                        .child(ui::label("UPDATE", &p))
                        .child(ui::spacer())
                        .child(
                            div()
                                .id("update-close")
                                .px(px(4.))
                                .small()
                                .text_color(p.muted)
                                .cursor_pointer()
                                .hover(|s| s.text_color(p.text))
                                .child("×")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.update = None;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .title()
                        .text_color(p.text)
                        .child(format!("SiteVet {} is out", update.version)),
                )
                .child(div().small().text_color(p.muted).child(format!(
                    "You have {}. The download opens in your browser.",
                    env!("CARGO_PKG_VERSION")
                )))
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .pt(px(4.))
                        .child(
                            ui::button_primary("update-download", "Download", 32., &p).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    cx.open_url(&url);
                                    this.update = None;
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(
                            ui::button_secondary("update-later", "Later", 32., &p).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.update = None;
                                    cx.notify();
                                }),
                            ),
                        ),
                ),
        )
    }
}
