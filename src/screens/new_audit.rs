//! New audit: one site, or several one after another.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppView;
use crate::data;
use crate::theme::{ActivePalette, Palette, TextStyles};
use crate::ui;

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
    pub fn render_new_audit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = cx.pal();
        let several = self.form.several;
        let s = &self.form.settings;

        // What the one-site footer and compare line can promise.
        let one_url = self.form.start_url.read(cx).value().to_string();
        let known = data::parse_urls(&one_url)
            .first()
            .and_then(|u| self.sites.iter().find(|s| s.host() == data::host_of(u)))
            .map(|s| s.runs.len());
        let compare_desc = if several {
            "Each site is compared with its own last run, when it has one.".to_string()
        } else {
            match known {
                Some(n) => format!("Adds a changes view: fixed and new issues since run {n}."),
                None => "Adds a changes view once the site has an earlier run.".to_string(),
            }
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
                    .child(
                        check(
                            "browser",
                            "Rendered checks in Chrome",
                            "axe-core accessibility and Core Web Vitals. Uses your installed Chrome.".into(),
                            s.options.browser,
                            false,
                            &p,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            let o = &mut this.form.settings.options;
                            o.browser = !o.browser;
                            if !o.browser {
                                o.render_crawl = false;
                                o.screenshots = false;
                            }
                            cx.notify();
                        })),
                    )
                    .child(
                        check(
                            "robots",
                            "Obey robots.txt",
                            "Skip pages the site asks crawlers to leave alone.".into(),
                            !s.options.ignore_robots,
                            false,
                            &p,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.form.settings.options.ignore_robots ^= true;
                            cx.notify();
                        })),
                    )
                    .child(
                        check(
                            "external",
                            "Check external links",
                            "One HEAD request per external link, shared across pages.".into(),
                            s.check_external_links,
                            false,
                            &p,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.form.settings.check_external_links ^= true;
                            cx.notify();
                        })),
                    )
                    .child(
                        check("compare", "Compare with the last run", compare_desc, s.compare_with_last, !self.form.show_all, &p)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.form.settings.compare_with_last ^= true;
                                cx.notify();
                            })),
                    )
                    .when(self.form.show_all, |d| {
                        d.child(
                            check(
                                "sitemap",
                                "Seed from the sitemap",
                                "Start the crawl from sitemap.xml as well as the start page.".into(),
                                s.options.crawl_sitemap,
                                false,
                                &p,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.form.settings.options.crawl_sitemap ^= true;
                                cx.notify();
                            })),
                        )
                        .child(
                            check(
                                "render-crawl",
                                "Find links after scripts run",
                                "Reads links from the rendered page. Needs rendered checks.".into(),
                                s.options.render_crawl,
                                false,
                                &p,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                let o = &mut this.form.settings.options;
                                o.render_crawl = !o.render_crawl;
                                o.browser |= o.render_crawl;
                                cx.notify();
                            })),
                        )
                        .child(
                            check(
                                "screenshots",
                                "Screenshots",
                                "Saves a screenshot of every page. Needs rendered checks.".into(),
                                s.options.screenshots,
                                true,
                                &p,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                let o = &mut this.form.settings.options;
                                o.screenshots = !o.screenshots;
                                o.browser |= o.screenshots;
                                cx.notify();
                            })),
                        )
                    }),
            );

        // Every control the form offers once expanded.
        let option_count = 4 + 3 + self.form.extras.len() + 3 + usize::from(several);
        let extras = self.form.show_all.then(|| {
            let fields: Vec<Div> = self
                .form
                .extras
                .iter()
                .map(|e| ui::field(e.name, &e.input, window, cx))
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

        let limits = div()
            .flex()
            .gap(px(16.))
            .w_full()
            .child(ui::field("MAX PAGES", &self.form.max_pages, window, cx))
            .child(ui::field("MAX DEPTH", &self.form.max_depth, window, cx))
            .child(ui::field("CONCURRENCY", &self.form.concurrency, window, cx))
            .when(several, |d| {
                d.child(ui::field(
                    "SITES AT A TIME",
                    &self.form.sites_at_a_time,
                    window,
                    cx,
                ))
            });

        let footer_note = match (&self.form.note, several) {
            (Some(n), _) => n.clone(),
            (None, false) => match (known, s.compare_with_last) {
                (Some(n), true) => format!("SAVED AS RUN {} · COMPARED WITH RUN {n}", n + 1),
                (Some(n), false) => format!("SAVED AS RUN {}", n + 1),
                (None, _) => "SAVED AS RUN 1".into(),
            },
            (None, true) => self.batch_estimate(cx),
        };
        let run_label = if several {
            format!(
                "Run {} audit{}",
                self.form.sites.len(),
                if self.form.sites.len() == 1 { "" } else { "s" }
            )
        } else {
            "Run audit".into()
        };

        let target = if several {
            self.sites_list(cx).into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .w_full()
                .child(ui::label("START URL", &p))
                .child(ui::text_field(&self.form.start_url, window, cx))
                .child(
                    div()
                        .small()
                        .text_color(p.muted)
                        .child("Same-origin pages only. robots.txt and sitemap.xml are read once."),
                )
                .into_any_element()
        };

        let form = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(28.))
            .w(px(720.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(div().h2().text_color(p.text).child("New audit"))
                    .child(
                        div()
                            .body()
                            .text_color(p.muted)
                            .child("Audit one site, or several one after another. Every run is saved to history."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .child(ui::segment("one", "One site", !several, &p).on_click(cx.listener(|this, _, _, cx| {
                        this.form.several = false;
                        cx.notify();
                    })))
                    .child(ui::segment("several", "Several sites", several, &p).on_click(cx.listener(|this, _, window, cx| {
                        if !this.form.several && this.form.sites.is_empty() {
                            // Start the list with the one-site URL, if any.
                            let url = this.form.start_url.read(cx).value().to_string();
                            this.form.sites.extend(data::parse_urls(&url));
                            let _ = window;
                        }
                        this.form.several = true;
                        cx.notify();
                    }))),
            )
            .child(target)
            .child(limits)
            .child(checks)
            .children(extras)
            .child(
                ui::link(
                    "show-all",
                    if self.form.show_all { "Show fewer options".to_string() } else { format!("Show all {option_count} options") },
                    &p,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.form.show_all ^= true;
                    cx.notify();
                })),
            )
            .when_some(self.form.error.clone(), |d, e| d.child(div().small().text_color(p.fail).child(e)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(4.))
                    .child(
                        ui::button_primary("run", run_label, 40., &p)
                            .on_click(cx.listener(|this, _, _, cx| this.run_from_form(cx))),
                    )
                    .child(
                        ui::button_secondary("defaults", "Save as defaults", 40., &p)
                            .on_click(cx.listener(|this, _, _, cx| this.save_defaults(cx))),
                    )
                    .child(ui::spacer())
                    .child(ui::micro(footer_note, p.muted)),
            );

        div()
            .id("new-audit")
            .flex()
            .flex_col()
            .items_center()
            .size_full()
            .py(px(if several { 26. } else { 40. }))
            .bg(p.bg_app)
            .overflow_y_scroll()
            .child(form)
    }

    fn sites_list(&mut self, cx: &mut Context<Self>) -> Div {
        let p = cx.pal();
        let rows: Vec<AnyElement> = self
            .form
            .sites
            .iter()
            .enumerate()
            .map(|(i, url)| {
                let last = self
                    .sites
                    .iter()
                    .find(|s| s.host() == data::host_of(url))
                    .map(|s| s.latest.score);
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .h(px(34.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(ui::micro(format!("{:02}", i + 1), p.muted))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .mono()
                            .text_color(p.text)
                            .truncate()
                            .child(url.clone()),
                    )
                    .map(|d| match last {
                        Some(score) => d
                            .child(ui::micro("LAST", p.muted))
                            .child(ui::score_chip(score, &p)),
                        None => d.child(ui::micro("NEW", p.muted)),
                    })
                    .child(
                        div()
                            .id(("remove-site", i))
                            .px(px(4.))
                            .small()
                            .text_color(p.muted)
                            .cursor_pointer()
                            .hover(|s| s.text_color(p.text))
                            .child("×")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.form.sites.remove(i);
                                cx.notify();
                            })),
                    )
                    .into_any_element()
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(ui::label(format!("SITES · {}", self.form.sites.len()), &p))
                    .child(ui::spacer())
                    .child(
                        div()
                            .id("clear-all")
                            .small()
                            .text_color(p.muted)
                            .cursor_pointer()
                            .hover(|s| s.text_color(p.text))
                            .child("Clear all")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.form.sites.clear();
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .bg(p.bg_surface)
                    .border_1()
                    .border_color(p.border)
                    .children(rows)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .px(px(12.))
                            .py(px(8.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .mono()
                                    .child(gpui_kit::component::input::Input::new(&self.form.paste).appearance(false).px_0()),
                            )
                            .child(
                                ui::button_secondary("add", "Add", 28., &p)
                                    .on_click(cx.listener(|this, _, window, cx| this.add_pasted(window, cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .small()
                    .text_color(p.muted)
                    .child("Each site runs with the limits and checks below, and is saved to history as its own run."),
            )
    }

    /// "5 SITES · 2 AT A TIME · ABOUT 6 MIN", from each site's last run time.
    fn batch_estimate(&self, cx: &App) -> String {
        let n = self.form.sites.len();
        let parallel: usize = self
            .form
            .sites_at_a_time
            .read(cx)
            .value()
            .trim()
            .parse()
            .unwrap_or(1)
            .max(1);
        let known: Vec<u64> = self.sites.iter().map(|s| s.latest.duration_ms).collect();
        let fallback = if known.is_empty() {
            60_000
        } else {
            known.iter().sum::<u64>() / known.len() as u64
        };
        let total: u64 = self
            .form
            .sites
            .iter()
            .map(|u| {
                self.sites
                    .iter()
                    .find(|s| s.host() == data::host_of(u))
                    .map_or(fallback, |s| s.latest.duration_ms)
            })
            .sum();
        let minutes = (total / parallel as u64).div_ceil(60_000).max(1);
        format!(
            "{n} SITE{} · {parallel} AT A TIME · ABOUT {minutes} MIN",
            if n == 1 { "" } else { "S" }
        )
    }
}
