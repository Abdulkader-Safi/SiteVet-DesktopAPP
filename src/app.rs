//! The window: state, the audit engine and the shell (site list and top bar).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sitevet::{AuditEvent, AuditHandle, AuditReport, ReportDiff, RunId, Status};

use crate::data::{self, AuditSettings, Site, time};
use crate::theme::{self, ActivePalette, TextStyles};
use crate::ui;

#[derive(Clone, Copy, PartialEq)]
pub enum Screen {
    Report,
    NewAudit,
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
pub struct LiveRun {
    pub url: String,
    pub settings: AuditSettings,
    pub status: RunStatus,
    pub handle: Option<AuditHandle>,
    pub started: Option<Instant>,
    pub started_clock: String,
    pub elapsed_ms: u64,
    pub max_pages: u32,
    pub fetched: Vec<String>,
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
        matches!(self.status, RunStatus::Queued | RunStatus::Running | RunStatus::Saving)
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
            // ponytail: during the crawl the total is unknown; pages fetched against the cap
            // is the best guess and jumps once the crawl completes.
            _ => {
                let (done, total) = self.pages();
                done as f32 / total.max(1) as f32
            }
        }
    }

    pub fn run_number(&self) -> usize {
        self.prev.map_or(1, |(n, _)| n + 1)
    }
}

pub struct Batch {
    pub runs: Vec<usize>,
    pub parallel: usize,
    pub started: Instant,
    pub started_clock: String,
}

/// A setting from "Show all options": a text field mapped onto one `Options` field.
pub struct Extra {
    pub name: &'static str,
    pub input: Entity<InputState>,
    pub apply: fn(&mut sitevet::Options, &str) -> Result<(), String>,
}

pub struct Form {
    pub several: bool,
    pub start_url: Entity<InputState>,
    pub max_pages: Entity<InputState>,
    pub max_depth: Entity<InputState>,
    pub concurrency: Entity<InputState>,
    pub sites_at_a_time: Entity<InputState>,
    pub paste: Entity<InputState>,
    pub extras: Vec<Extra>,
    pub sites: Vec<String>,
    pub settings: AuditSettings,
    pub show_all: bool,
    pub error: Option<String>,
    pub note: Option<String>,
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
    pub range: Range,
    pub metric: Option<String>,
    pub compare: (Option<RunId>, Option<RunId>),
    pub compare_cache: Option<((RunId, RunId), Arc<ReportDiff>)>,
    pub open_select: OpenSelect,
    pub changes_from_history: bool,
    // Audits.
    pub form: Form,
    pub runs: Vec<LiveRun>,
    pub batch: Option<Batch>,
    ticking: bool,
}

fn input(window: &mut Window, cx: &mut Context<AppView>, placeholder: &str, value: String) -> Entity<InputState> {
    let placeholder = placeholder.to_string();
    let e = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder).default_value(value));
    cx.subscribe(&e, |_, _, _: &InputEvent, cx| cx.notify()).detach();
    e
}

fn opt_str(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn list(s: &str) -> Vec<String> {
    s.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect()
}

fn num<T: std::str::FromStr>(name: &str, s: &str) -> Result<T, String> {
    s.trim().parse().map_err(|_| format!("{name} must be a whole number"))
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let defaults = data::load_defaults();
        let o = &defaults.options;
        let extra = |window: &mut Window, cx: &mut Context<Self>, name, value: String, apply| Extra {
            name,
            input: input(window, cx, "", value),
            apply,
        };
        let extras = vec![
            extra(window, cx, "DELAY MS", o.delay_ms.to_string(), |o, v| Ok(o.delay_ms = num("Delay", v)?)),
            extra(window, cx, "TIMEOUT MS", o.timeout_ms.to_string(), |o, v| Ok(o.timeout_ms = num("Timeout", v)?)),
            extra(window, cx, "USER AGENT", o.user_agent.clone().unwrap_or_default(), |o, v| {
                Ok(o.user_agent = opt_str(v))
            }),
            extra(window, cx, "ONLY CATEGORIES", o.only.join(", "), |o, v| Ok(o.only = list(v))),
            extra(window, cx, "SKIP CATEGORIES", o.skip.join(", "), |o, v| Ok(o.skip = list(v))),
            extra(window, cx, "CLICK DEPTH WARN", o.click_depth_warn.to_string(), |o, v| {
                Ok(o.click_depth_warn = num("Click depth warn", v)?)
            }),
            extra(window, cx, "SLOW RESOURCE MS", o.slow_resource_ms.to_string(), |o, v| {
                Ok(o.slow_resource_ms = num("Slow resource", v)?)
            }),
            extra(window, cx, "CHROME PATH", o.chrome_path.as_ref().map(|p| p.display().to_string()).unwrap_or_default(), |o, v| {
                Ok(o.chrome_path = opt_str(v).map(Into::into))
            }),
            extra(window, cx, "PAGESPEED KEY", o.psi_key.clone().unwrap_or_default(), |o, v| Ok(o.psi_key = opt_str(v))),
            extra(window, cx, "PAGESPEED MAX PAGES", o.psi_max_pages.to_string(), |o, v| {
                Ok(o.psi_max_pages = num("PageSpeed max pages", v)?)
            }),
        ];
        let form = Form {
            several: false,
            start_url: input(window, cx, "https://example.com", String::new()),
            max_pages: input(window, cx, "", o.max_pages.to_string()),
            max_depth: input(window, cx, "", o.max_depth.to_string()),
            concurrency: input(window, cx, "", o.concurrency.to_string()),
            sites_at_a_time: input(window, cx, "", defaults.sites_at_a_time.to_string()),
            paste: input(window, cx, "Paste URLs, one per line", String::new()),
            extras,
            sites: Vec::new(),
            settings: defaults,
            show_all: false,
            error: None,
            note: None,
        };
        cx.subscribe_in(&form.start_url, window, |this, _, ev: &InputEvent, _, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.run_from_form(cx);
            }
        })
        .detach();
        cx.subscribe_in(&form.paste, window, |this, _, ev: &InputEvent, window, cx| {
            if matches!(ev, InputEvent::PressEnter { .. }) {
                this.add_pasted(window, cx);
            }
        })
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
            range: Range::All,
            metric: None,
            compare: (None, None),
            compare_cache: None,
            open_select: OpenSelect::None,
            changes_from_history: false,
            form,
            runs: Vec::new(),
            batch: None,
            ticking: false,
        };
        this.reload_sites(None, cx);
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
                        let first = app.selected.is_none() && select.is_none();
                        if let Some(origin) = select {
                            app.open_site(&origin, cx);
                        } else if first && let Some(origin) = app.sites.first().map(|s| s.origin.clone()) {
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
        let missing: Vec<RunId> =
            site.runs.iter().map(|r| r.id).filter(|id| !self.reports.contains_key(id) && !self.loading.contains(id)).collect();
        if missing.is_empty() {
            return;
        }
        self.loading.extend(missing.iter().copied());
        let task = cx.background_spawn(async move {
            missing.into_iter().filter_map(|id| data::load_report(id).ok().map(|r| (id, r))).collect::<Vec<_>>()
        });
        cx.spawn(async move |this, cx| {
            let loaded = task.await;
            let _ = this.update(cx, |app, cx| {
                for (id, r) in loaded {
                    app.loading.remove(&id);
                    app.reports.insert(id, Arc::new(r));
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
            self.load_history(cx);
        }
        if tab != Tab::Changes {
            self.changes_from_history = false;
        }
        cx.notify();
    }

    /// The active run for a site, if any.
    pub fn run_for(&self, host: &str) -> Option<usize> {
        self.runs.iter().rposition(|r| r.active() && data::host_of(&r.url) == host)
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
            let url = self.site().map(|s| s.latest.start_url.clone()).unwrap_or_default();
            self.form.start_url.update(cx, |s, cx| s.set_value(url, window, cx));
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
        self.form.paste.update(cx, |s, cx| s.set_value("", window, cx));
        cx.notify();
    }

    /// Reads the form into settings, or says which field is wrong.
    fn read_form(&self, cx: &App) -> Result<AuditSettings, String> {
        let f = &self.form;
        let v = |e: &Entity<InputState>| e.read(cx).value().to_string();
        let mut s = f.settings.clone();
        s.options.max_pages = num("Max pages", &v(&f.max_pages))?;
        s.options.max_depth = num("Max depth", &v(&f.max_depth))?;
        s.options.concurrency = num("Concurrency", &v(&f.concurrency))?;
        s.sites_at_a_time = num::<usize>("Sites at a time", &v(&f.sites_at_a_time))?.max(1);
        for extra in &f.extras {
            (extra.apply)(&mut s.options, &v(&extra.input))?;
        }
        s.options.validate().map_err(|e| e.to_string())?;
        Ok(s)
    }

    pub fn save_defaults(&mut self, cx: &mut Context<Self>) {
        match self.read_form(cx).and_then(|s| data::save_defaults(&s)) {
            Ok(()) => {
                self.form.error = None;
                self.form.note = Some("SAVED AS DEFAULTS".into());
            }
            Err(e) => self.form.error = Some(e),
        }
        cx.notify();
    }

    pub fn run_from_form(&mut self, cx: &mut Context<Self>) {
        let settings = match self.read_form(cx) {
            Ok(s) => s,
            Err(e) => {
                self.form.error = Some(e);
                cx.notify();
                return;
            }
        };
        self.form.error = None;
        self.form.note = None;
        if self.form.several {
            let urls = self.form.sites.clone();
            if urls.is_empty() {
                self.form.error = Some("Add at least one site".into());
            } else {
                self.start_batch(urls, settings, cx);
            }
        } else {
            let url = self.form.start_url.read(cx).value().trim().to_string();
            match data::parse_urls(&url).into_iter().next() {
                Some(url) => self.start_one(url, settings, cx),
                None => self.form.error = Some("Enter a start URL".into()),
            }
        }
        cx.notify();
    }

    /// "Run audit again" from history: the saved defaults on the site's start URL.
    pub fn rerun(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.site().map(|s| s.latest.start_url.clone()) else { return };
        let settings = self.read_form(cx).unwrap_or_else(|_| data::load_defaults());
        self.start_one(url, settings, cx);
    }

    // ---- The audit engine ----

    fn new_run(&self, url: String, settings: AuditSettings) -> LiveRun {
        let host = data::host_of(&url);
        let prev = self.sites.iter().find(|s| s.host() == host).map(|s| (s.runs.len(), s.latest.score));
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
        self.batch = Some(Batch {
            runs: (first..self.runs.len()).collect(),
            parallel,
            started: Instant::now(),
            started_clock: time::now_clock(),
        });
        self.screen = Screen::Batch;
        self.pump_batch(cx);
    }

    /// Starts queued batch runs until `parallel` are in flight.
    fn pump_batch(&mut self, cx: &mut Context<Self>) {
        let Some(batch) = &self.batch else { return };
        let running = batch.runs.iter().filter(|&&i| matches!(self.runs[i].status, RunStatus::Running)).count();
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
        if run.active() {
            if let Some(h) = run.handle.take() {
                h.cancel();
            }
            run.elapsed_ms = run.elapsed();
            run.status = RunStatus::Cancelled;
        }
        cx.notify();
    }

    pub fn cancel_all(&mut self, cx: &mut Context<Self>) {
        let ids = self.batch.as_ref().map(|b| b.runs.clone()).unwrap_or_default();
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
        match ev {
            AuditEvent::AuditStart { max_pages, .. } => run.max_pages = max_pages,
            AuditEvent::PageFetched { url, .. } => run.fetched.push(url),
            AuditEvent::CrawlComplete { pages, checks_total, .. } => {
                run.pages_total = Some(pages);
                run.checks_total = checks_total;
            }
            AuditEvent::CheckComplete { status, checks_done, checks_total, .. } => {
                run.checks_done = checks_done;
                run.checks_total = checks_total;
                match status {
                    Status::Pass => run.pass += 1,
                    Status::Warn => run.warn += 1,
                    Status::Fail => run.fail += 1,
                    _ => {}
                }
            }
            AuditEvent::PageComplete { url, score, .. } => run.completed.push((url, score)),
            AuditEvent::AuditComplete { report, .. } => {
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
        let Some(handle) = self.runs[idx].handle.take() else { return };
        cx.spawn(async move |this, cx| {
            let err = handle.report().await.err().map_or_else(|| "The audit stopped".to_string(), |e| e.to_string());
            let _ = this.update(cx, |app, cx| {
                let run = &mut app.runs[idx];
                if run.status == RunStatus::Running {
                    run.elapsed_ms = run.elapsed();
                    run.status = RunStatus::Failed(err);
                }
                app.pump_batch(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn on_saved(&mut self, idx: usize, report: AuditReport, saved: Result<RunId, String>, cx: &mut Context<Self>) {
        let origin = report.start_url.clone();
        let run = &mut self.runs[idx];
        run.report = Some(Arc::new(report));
        run.status = match saved {
            Ok(_) => RunStatus::Done,
            Err(e) => RunStatus::Failed(format!("Finished, but saving to history failed: {e}")),
        };
        let open = self.screen == Screen::Running(idx);
        if open {
            self.tab = Tab::Pages;
        }
        let host = data::host_of(&origin).to_string();
        // Select the site once history reloads, so the left pane and report show the new run.
        let select = open.then(|| host.clone());
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
                if let Some(host) = select
                    && let Some(origin) = app.sites.iter().find(|s| s.host() == host).map(|s| s.origin.clone())
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
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let keep = this
                    .update(cx, |app, cx| {
                        cx.notify();
                        app.runs.iter().any(|r| matches!(r.status, RunStatus::Running | RunStatus::Saving))
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
                cx.notify();
            }
            return;
        }
        let target = match ev.keystroke.key.as_str() {
            "p" => Some(&self.site_search),
            "c" if self.screen == Screen::Report && self.tab == Tab::Pages => Some(&self.category_search),
            "/" if self.screen == Screen::Report && self.tab == Tab::Pages => Some(&self.page_search),
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
            let run = self.run_for(&host);
            let selected = match self.screen {
                Screen::Running(r) => data::host_of(&self.runs[r].url) == host,
                Screen::Report => self.selected.as_deref() == Some(site.origin.as_str()),
                _ => false,
            };
            let origin = site.origin.clone();
            let row = match run {
                Some(r) => self.running_row(r, &host, &p),
                None => {
                    let issues = data::site_issues(&site.latest);
                    site_row(&host, data::issues_label(&issues), p.muted, ui::score_chip(site.latest.score, &p), &p)
                }
            };
            rows.push(
                row.id(("site", i))
                    .when(selected, |d| d.bg(p.bg_raised).child(ui::edge(&p)))
                    .on_click(cx.listener(move |this, _, _, cx| match this.run_for(data::host_of(&origin)) {
                        Some(r) => {
                            this.screen = Screen::Running(r);
                            cx.notify();
                        }
                        None => this.open_site(&origin, cx),
                    }))
                    .into_any_element(),
            );
        }
        let known: HashSet<String> = self.sites.iter().map(|s| s.host().to_string()).collect();
        let mut shown = HashSet::new();
        for (i, run) in self.runs.iter().enumerate().rev() {
            let host = data::host_of(&run.url).to_string();
            if !run.active() || known.contains(&host) || !shown.insert(host.clone()) {
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
                div().flex().flex_none().p(px(12.)).border_b_1().border_color(p.border).child(
                    button.flex_1().on_click(cx.listener(|this, _, window, cx| this.new_audit(window, cx))),
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
                        d.child(div().p(px(16.)).small().text_color(p.fail).child(format!("History could not be read: {e}")))
                    }),
            )
    }

    fn running_row(&self, idx: usize, host: &str, p: &theme::Palette) -> Div {
        let run = &self.runs[idx];
        let (done, total) = run.pages();
        let (sub, right) = match run.status {
            RunStatus::Queued => ("QUEUED".to_string(), String::new()),
            _ => (format!("RUNNING · {done} / {total}"), format!("{:.0}%", run.fraction() * 100.)),
        };
        site_row(host, sub, p.accent_text, div().mono_small().text_color(p.accent_text).child(right), p)
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
            Screen::Running(i) => {
                let run = &self.runs[i];
                title = run.url.clone();
                right = Some(format!("RUN {} · STARTED {}", run.run_number(), run.started_clock));
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
                        format!("{} RUNS · SINCE {}", s.runs.len(), time::date(&s.runs[0].started_at))
                    }
                    _ => format!("RUN {} · {}", s.runs.len(), time::date_time(&s.latest_run().started_at)),
                });
            }
        }
        bar = bar.child(div().mono_small().text_color(p.muted).whitespace_nowrap().child(title)).child(ui::spacer());
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
            // Export comes later; the icon holds its place.
            bar = bar.child(ui::icon("icons/file-export.svg", 25., 22., p.muted));
        }
        bar.children(right.map(|r| ui::micro(r, p.muted))).child(
            div()
                .id("theme")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(28.))
                .cursor_pointer()
                .hover(|s| s.bg(p.bg_raised))
                .child(ui::icon(if dark { "icons/sun.svg" } else { "icons/moon.svg" }, 16., 16., p.muted))
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
                .child(div().mono_small().text_color(p.text).truncate().child(host.to_string()))
                .child(div().mono_micro().text_color(sub_color).truncate().child(sub)),
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
    }
}
