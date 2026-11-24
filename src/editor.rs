//! The audit options editor: limits, checks and "all options". The new audit form and the
//! defaults in Settings each own one.

use gpui_kit::component::input::InputState;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sitevet::Options;

use crate::app::{AppView, input};
use crate::data::AuditSettings;
use crate::theme::{ActivePalette, Palette, TextStyles};
use crate::ui;

/// Which editor a control belongs to.
#[derive(Clone, Copy, PartialEq)]
pub enum Which {
    Form,
    Defaults,
}

/// A setting from "Show all options": a text field mapped onto one `Options` field.
struct ExtraSpec {
    name: &'static str,
    get: fn(&Options) -> String,
    apply: fn(&mut Options, &str) -> Result<(), String>,
}

fn opt_str(s: &str) -> Option<String> {
    let s = s.trim();
    (!s.is_empty()).then(|| s.to_string())
}

fn list(s: &str) -> Vec<String> {
    s.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn num<T: std::str::FromStr>(name: &str, s: &str) -> Result<T, String> {
    s.trim()
        .parse()
        .map_err(|_| format!("{name} must be a whole number"))
}

const EXTRAS: [ExtraSpec; 10] = [
    ExtraSpec {
        name: "DELAY MS",
        get: |o| o.delay_ms.to_string(),
        apply: |o, v| {
            o.delay_ms = num("Delay", v)?;
            Ok(())
        },
    },
    ExtraSpec {
        name: "TIMEOUT MS",
        get: |o| o.timeout_ms.to_string(),
        apply: |o, v| {
            o.timeout_ms = num("Timeout", v)?;
            Ok(())
        },
    },
    ExtraSpec {
        name: "USER AGENT",
        get: |o| o.user_agent.clone().unwrap_or_default(),
        apply: |o, v| {
            o.user_agent = opt_str(v);
            Ok(())
        },
    },
    ExtraSpec {
        name: "ONLY CATEGORIES",
        get: |o| o.only.join(", "),
        apply: |o, v| {
            o.only = list(v);
            Ok(())
        },
    },
    ExtraSpec {
        name: "SKIP CATEGORIES",
        get: |o| o.skip.join(", "),
        apply: |o, v| {
            o.skip = list(v);
            Ok(())
        },
    },
    ExtraSpec {
        name: "CLICK DEPTH WARN",
        get: |o| o.click_depth_warn.to_string(),
        apply: |o, v| {
            o.click_depth_warn = num("Click depth warn", v)?;
            Ok(())
        },
    },
    ExtraSpec {
        name: "SLOW RESOURCE MS",
        get: |o| o.slow_resource_ms.to_string(),
        apply: |o, v| {
            o.slow_resource_ms = num("Slow resource", v)?;
            Ok(())
        },
    },
    ExtraSpec {
        name: "CHROME PATH",
        get: |o| {
            o.chrome_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        },
        apply: |o, v| {
            o.chrome_path = opt_str(v).map(Into::into);
            Ok(())
        },
    },
    ExtraSpec {
        name: "PAGESPEED KEY",
        get: |o| o.psi_key.clone().unwrap_or_default(),
        apply: |o, v| {
            o.psi_key = opt_str(v);
            Ok(())
        },
    },
    ExtraSpec {
        name: "PAGESPEED MAX PAGES",
        get: |o| o.psi_max_pages.to_string(),
        apply: |o, v| {
            o.psi_max_pages = num("PageSpeed max pages", v)?;
            Ok(())
        },
    },
];

pub struct OptionsEditor {
    pub max_pages: Entity<InputState>,
    pub max_depth: Entity<InputState>,
    pub concurrency: Entity<InputState>,
    pub sites_at_a_time: Entity<InputState>,
    extras: Vec<Entity<InputState>>,
    /// The switches; the text fields are read into a copy of this by `read`.
    pub settings: AuditSettings,
    pub show_all: bool,
}

impl OptionsEditor {
    pub fn new(s: &AuditSettings, window: &mut Window, cx: &mut Context<AppView>) -> Self {
        let o = &s.options;
        Self {
            max_pages: input(window, cx, "", o.max_pages.to_string()),
            max_depth: input(window, cx, "", o.max_depth.to_string()),
            concurrency: input(window, cx, "", o.concurrency.to_string()),
            sites_at_a_time: input(window, cx, "", s.sites_at_a_time.to_string()),
            extras: EXTRAS
                .iter()
                .map(|e| input(window, cx, "", (e.get)(o)))
                .collect(),
            settings: s.clone(),
            show_all: false,
        }
    }

    /// Puts `s` into every field and switch.
    pub fn load(&mut self, s: &AuditSettings, window: &mut Window, cx: &mut App) {
        let o = &s.options;
        let set = |e: &Entity<InputState>, v: String, window: &mut Window, cx: &mut App| {
            e.update(cx, |state, cx| state.set_value(v, window, cx));
        };
        set(&self.max_pages, o.max_pages.to_string(), window, cx);
        set(&self.max_depth, o.max_depth.to_string(), window, cx);
        set(&self.concurrency, o.concurrency.to_string(), window, cx);
        set(
            &self.sites_at_a_time,
            s.sites_at_a_time.to_string(),
            window,
            cx,
        );
        for (spec, e) in EXTRAS.iter().zip(&self.extras) {
            set(e, (spec.get)(o), window, cx);
        }
        self.settings = s.clone();
    }

    /// The settings as typed, or which field is wrong.
    pub fn read(&self, cx: &App) -> Result<AuditSettings, String> {
        self.read_with(true, cx)
    }

    /// Like `read`, but leaves "Sites at a time" as it was when that field is hidden, so a
    /// value you cannot see never blocks a one-site audit.
    pub fn read_with(&self, sites_field: bool, cx: &App) -> Result<AuditSettings, String> {
        let v = |e: &Entity<InputState>| e.read(cx).value().to_string();
        let mut s = self.settings.clone();
        s.options.max_pages = num("Max pages", &v(&self.max_pages))?;
        s.options.max_depth = num("Max depth", &v(&self.max_depth))?;
        s.options.concurrency = num("Concurrency", &v(&self.concurrency))?;
        if sites_field {
            s.sites_at_a_time = num::<usize>("Sites at a time", &v(&self.sites_at_a_time))?.max(1);
        }
        for (spec, e) in EXTRAS.iter().zip(&self.extras) {
            (spec.apply)(&mut s.options, &v(e))?;
        }
        s.options.validate().map_err(|e| e.to_string())?;
        Ok(s)
    }

    /// Whether the typed values differ from `defaults`. Unreadable fields count as different.
    pub fn differs_from(&self, defaults: &AuditSettings, cx: &App) -> bool {
        let toml = |s: &AuditSettings| crate::data::settings_to_toml(s).ok();
        self.read(cx).ok().and_then(|s| toml(&s)) != toml(defaults)
    }
}

/// A check row: title, description, square switch.
fn check(
    id: &'static str,
    title: &'static str,
    desc: String,
    on: bool,
    last: bool,
    p: &Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(16.))
        .px(px(16.))
        .py(px(12.))
        .cursor_pointer()
        .when(!last, |d| d.border_b_1().border_color(p.border))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .gap(px(2.))
                .child(div().body_medium().text_color(p.text).child(title))
                .child(div().small().text_color(p.muted).child(desc)),
        )
        .child(ui::toggle(
            SharedString::from(format!("{id}-toggle")),
            on,
            p,
        ))
}

impl AppView {
    pub fn editor_mut(&mut self, which: Which) -> &mut OptionsEditor {
        match which {
            Which::Form => &mut self.form.editor,
            Which::Defaults => &mut self.defaults_editor,
        }
    }

    fn toggle(&mut self, which: Which, edit: fn(&mut AuditSettings), cx: &mut Context<Self>) {
        edit(&mut self.editor_mut(which).settings);
        cx.notify();
    }

    /// Limits, checks, the extra fields and the "Show all" link, for one editor.
    /// `sites_field` adds "Sites at a time"; `compare_desc` describes the compare switch.
    pub fn render_options(
        &mut self,
        which: Which,
        sites_field: bool,
        compare_desc: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = cx.pal();
        let ed = self.editor_mut(which);
        let s = ed.settings.clone();
        let show_all = ed.show_all;
        let (max_pages, max_depth, concurrency, sites) = (
            ed.max_pages.clone(),
            ed.max_depth.clone(),
            ed.concurrency.clone(),
            ed.sites_at_a_time.clone(),
        );
        let extras: Vec<Entity<InputState>> = ed.extras.clone();

        let toggle_row = |id,
                          title,
                          desc: String,
                          on,
                          last,
                          edit: fn(&mut AuditSettings),
                          cx: &mut Context<Self>| {
            check(id, title, desc, on, last, &p)
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(which, edit, cx)))
        };

        let checks = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .child(ui::label("CHECKS", &p))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .bg(p.bg_surface)
                    .border_1()
                    .border_color(p.border)
                    .child(toggle_row(
                        "browser",
                        "Rendered checks in Chrome",
                        "axe-core accessibility and Core Web Vitals. Uses your installed Chrome."
                            .into(),
                        s.options.browser,
                        false,
                        |s| {
                            let o = &mut s.options;
                            o.browser = !o.browser;
                            if !o.browser {
                                o.render_crawl = false;
                                o.screenshots = false;
                            }
                        },
                        cx,
                    ))
                    .child(toggle_row(
                        "robots",
                        "Obey robots.txt",
                        "Skip pages the site asks crawlers to leave alone.".into(),
                        !s.options.ignore_robots,
                        false,
                        |s| s.options.ignore_robots ^= true,
                        cx,
                    ))
                    .child(toggle_row(
                        "external",
                        "Check external links",
                        "One HEAD request per external link, shared across pages.".into(),
                        s.check_external_links,
                        false,
                        |s| s.check_external_links ^= true,
                        cx,
                    ))
                    .child(toggle_row(
                        "compare",
                        "Compare with the last run",
                        compare_desc,
                        s.compare_with_last,
                        !show_all,
                        |s| s.compare_with_last ^= true,
                        cx,
                    ))
                    .when(show_all, |d| {
                        d.child(toggle_row(
                            "sitemap",
                            "Seed from the sitemap",
                            "Start the crawl from sitemap.xml as well as the start page.".into(),
                            s.options.crawl_sitemap,
                            false,
                            |s| s.options.crawl_sitemap ^= true,
                            cx,
                        ))
                        .child(toggle_row(
                            "render-crawl",
                            "Find links after scripts run",
                            "Reads links from the rendered page. Needs rendered checks.".into(),
                            s.options.render_crawl,
                            false,
                            |s| {
                                let o = &mut s.options;
                                o.render_crawl = !o.render_crawl;
                                o.browser |= o.render_crawl;
                            },
                            cx,
                        ))
                        .child(toggle_row(
                            "screenshots",
                            "Screenshots",
                            "Saves a screenshot of every page. Needs rendered checks.".into(),
                            s.options.screenshots,
                            true,
                            |s| {
                                let o = &mut s.options;
                                o.screenshots = !o.screenshots;
                                o.browser |= o.screenshots;
                            },
                            cx,
                        ))
                    }),
            );

        let limits = div()
            .flex()
            .gap(px(16.))
            .w_full()
            .child(ui::field("MAX PAGES", &max_pages, window, cx))
            .child(ui::field("MAX DEPTH", &max_depth, window, cx))
            .child(ui::field("CONCURRENCY", &concurrency, window, cx))
            .when(sites_field, |d| {
                d.child(ui::field("SITES AT A TIME", &sites, window, cx))
            });

        let more = show_all.then(|| {
            let fields: Vec<Div> = EXTRAS
                .iter()
                .zip(&extras)
                .map(|(spec, e)| ui::field(spec.name, e, window, cx))
                .collect();
            let mut grid = div().flex().flex_col().gap(px(16.)).w_full();
            let mut it = fields.into_iter();
            loop {
                let row: Vec<Div> = it.by_ref().take(2).collect();
                if row.is_empty() {
                    break;
                }
                grid = grid.child(div().flex().gap(px(16.)).w_full().children(row));
            }
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .w_full()
                .child(ui::label("MORE OPTIONS", &p))
                .child(grid)
        });

        // Every control the editor offers once expanded.
        let count = 3 + usize::from(sites_field) + 4 + 3 + EXTRAS.len();
        let link_text = if show_all {
            "Show fewer options".to_string()
        } else {
            format!("Show all {count} options")
        };

        div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .w_full()
            .child(limits)
            .child(checks)
            .children(more)
            .child(
                ui::link(
                    match which {
                        Which::Form => "show-all-form",
                        Which::Defaults => "show-all-defaults",
                    },
                    link_text,
                    &p,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.editor_mut(which).show_all ^= true;
                    cx.notify();
                })),
            )
    }
}
