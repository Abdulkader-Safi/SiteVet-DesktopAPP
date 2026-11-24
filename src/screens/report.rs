//! The report: Pages (designed), plus Overview, Site-wide and Changes built from the same parts.

use std::collections::BTreeMap;
use std::sync::Arc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sitevet::{AuditReport, Finding, FindingRef, Metric, MetricBody, MetricCell, PageResult, Status, ValueStatus};

use crate::app::{AppView, DetailTab, PageSort, Tab};
use crate::data::{self, category_name};
use crate::theme::{ActivePalette, Palette, TextStyles};
use crate::ui;

impl AppView {
    pub fn render_report(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(site) = self.site() else { return div().into_any_element() };
        let report = site.latest.clone();
        match self.tab {
            Tab::Pages => self.render_pages(&report, window, cx).into_any_element(),
            Tab::Overview => self.render_overview(&report, cx).into_any_element(),
            Tab::SiteWide => self.render_site_wide(&report, cx).into_any_element(),
            Tab::Changes => self.render_changes(&report, cx).into_any_element(),
            Tab::History => self.render_history(cx).into_any_element(),
        }
    }

    fn report_kpis(&self, report: &AuditReport, cx: &App) -> Div {
        let p = cx.pal();
        let (pass, warn, fail) = data::totals(report);
        let site = self.site();
        let (since_label, since_value) = match &report.changes {
            Some(c) => {
                let n = site.and_then(|s| c.baseline.run_id.and_then(|id| s.run_number(id)));
                (
                    n.map_or("SINCE LAST RUN".to_string(), |n| format!("SINCE RUN {n}")),
                    format!("{} · {} FIXED · {} NEW", data::signed(c.score_change.into()), c.fixed.len(), c.new_failures.len()),
                )
            }
            None => ("SINCE LAST RUN".to_string(), "FIRST RUN".to_string()),
        };
        let (word, band) = match report.score {
            90.. => ("PASS", p.pass),
            50..=89 => ("WARN", p.warn),
            _ => ("FAIL", p.fail),
        };
        div()
            .flex()
            .flex_none()
            .h(px(79.))
            .border_b_1()
            .border_color(p.border)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .justify_center()
                    .gap(px(4.))
                    .px(px(20.))
                    .py(px(14.))
                    .border_r_1()
                    .border_color(p.border)
                    .child(ui::label("OVERALL", &p))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(div().score_text().child(report.score.to_string()))
                            .child(ui::badge(word, band)),
                    ),
            )
            .child(ui::kpi("FAIL", Some(p.fail), ui::kpi_value(data::thousands(fail.into()), &p), &p))
            .child(ui::kpi("WARN", Some(p.warn), ui::kpi_value(data::thousands(warn.into()), &p), &p))
            .child(ui::kpi("PASS", Some(p.pass), ui::kpi_value(data::thousands(pass.into()), &p), &p))
            .child(ui::kpi("PAGES", Some(p.skip), ui::kpi_value(report.pages.len().to_string(), &p), &p))
            .child(ui::kpi(
                since_label,
                Some(p.accent_text),
                div().mono_small().text_color(p.text).whitespace_nowrap().child(since_value),
                &p,
            ))
            .child(ui::kpi(
                "DURATION",
                Some(p.skip),
                ui::kpi_value(format!("{:.1} S", report.duration_ms as f64 / 1000.), &p),
                &p,
            ))
    }

    // ---- Pages ----

    fn render_pages(&mut self, report: &Arc<AuditReport>, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = cx.pal();
        let cat_query = self.category_search.read(cx).value().to_lowercase();
        let page_query = self.page_search.read(cx).value().to_lowercase();

        // Categories, worst first.
        let mut cats: Vec<_> = report.categories.iter().collect();
        cats.sort_by_key(|c| (c.score, std::cmp::Reverse(c.fail), std::cmp::Reverse(c.warn)));
        let mut cat_rows: Vec<AnyElement> = vec![
            category_row("all", "All categories".into(), None, report.score, self.category.is_none(), &p)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.category = None;
                    cx.notify();
                }))
                .into_any_element(),
        ];
        for c in cats {
            let name = category_name(&c.category);
            if !cat_query.is_empty() && !name.to_lowercase().contains(&cat_query) {
                continue;
            }
            let count = match (c.fail, c.warn) {
                (0, 0) => ("0".to_string(), p.muted),
                (0, w) => (format!("{w} WARN"), p.warn),
                (f, _) => (format!("{f} FAIL"), p.fail),
            };
            let active = self.category.as_deref() == Some(c.category.as_str());
            let id = c.category.clone();
            cat_rows.push(
                category_row(SharedString::from(c.category.clone()), name, Some(count), c.score, active, &p)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.category = Some(id.clone());
                        this.page = None;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }

        // Pages, filtered by category and search.
        let in_cat = |f: &Finding| self.category.as_deref().is_none_or(|c| f.category == c);
        let mut pages: Vec<&PageResult> = report
            .pages
            .iter()
            .filter(|pg| {
                self.category.is_none()
                    || pg.findings.iter().any(|f| in_cat(f) && matches!(f.status, Status::Fail | Status::Warn))
            })
            .filter(|pg| page_query.is_empty() || pg.url.to_lowercase().contains(&page_query))
            .collect();
        match self.page_sort {
            PageSort::Lowest => pages.sort_by(|a, b| a.score.cmp(&b.score).then_with(|| a.url.cmp(&b.url))),
            PageSort::Path => pages.sort_by(|a, b| data::path_of(&a.url).cmp(&data::path_of(&b.url))),
        }
        let selected = self
            .page
            .as_deref()
            .and_then(|u| pages.iter().find(|pg| pg.url == u))
            .or(pages.first())
            .copied();
        let page_rows: Vec<AnyElement> = pages
            .iter()
            .enumerate()
            .map(|(i, pg)| {
                let issues = data::page_issues(pg);
                let url = pg.url.clone();
                let active = selected.is_some_and(|s| s.url == pg.url);
                list_row(data::path_of(&pg.url), data::issues_label(&issues), ui::score_chip(pg.score, &p), &p)
                    .id(("page", i))
                    .when(active, |d| d.bg(p.bg_raised).child(ui::edge(&p)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.page = Some(url.clone());
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();

        let categories = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(232.))
            .h_full()
            .bg(p.bg_sunken)
            .border_r_1()
            .border_color(p.border)
            .child(div().flex_none().pt(px(16.)).pb(px(8.)).px(px(16.)).child(ui::label("CATEGORIES", &p)))
            .child(div().flex().flex_none().pb(px(12.)).px(px(12.)).child(ui::search_box(&self.category_search, "c", window, cx)))
            .child(div().id("cat-list").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().children(cat_rows));

        let sort = self.page_sort;
        let page_pane = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(326.))
            .h_full()
            .border_r_1()
            .border_color(p.border)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .px(px(16.))
                    .py(px(12.))
                    .child(ui::label(format!("{} PAGES", pages.len()), &p))
                    .child(ui::spacer())
                    .child(ui::chip("sort-score", "Lowest score", sort == PageSort::Lowest, &p).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.page_sort = PageSort::Lowest;
                            cx.notify();
                        },
                    )))
                    .child(ui::chip("sort-path", "Path", sort == PageSort::Path, &p).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.page_sort = PageSort::Path;
                            cx.notify();
                        },
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .pb(px(12.))
                    .px(px(12.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(ui::search_box(&self.page_search, "/", window, cx)),
            )
            .child(div().id("page-list").flex().flex_col().flex_1().min_h_0().overflow_y_scroll().children(page_rows));

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.report_kpis(report, cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(categories)
                    .child(page_pane)
                    .child(self.page_detail(selected, cx)),
            )
    }

    fn page_detail(&self, page: Option<&PageResult>, cx: &mut Context<Self>) -> Div {
        let p = cx.pal();
        let pane = div().flex().flex_col().flex_1().min_w_0().h_full().gap(px(12.)).px(px(24.)).py(px(20.));
        let Some(page) = page else {
            return pane.child(div().body().text_color(p.muted).child("No pages match."));
        };
        let in_cat = |cat: &str| self.category.as_deref().is_none_or(|c| c == cat);
        let issues: Vec<&Finding> =
            page.findings.iter().filter(|f| f.status != Status::Pass && in_cat(&f.category)).collect();
        let passed: Vec<&Finding> =
            page.findings.iter().filter(|f| f.status == Status::Pass && in_cat(&f.category)).collect();
        let metrics: Vec<&Metric> = page.metrics.iter().filter(|m| in_cat(&m.category)).collect();
        let meta = format!(
            "{} · DEPTH {} · {} MS · {} KB",
            page.status,
            page.depth,
            page.response_ms,
            page.html_bytes.div_ceil(1024)
        );
        let tabs = [
            (DetailTab::Findings, format!("Findings {}", issues.len())),
            (DetailTab::Measurements, format!("Measurements {}", metrics.len())),
            (DetailTab::Passed, format!("Passed {}", passed.len())),
        ];
        let items: Vec<AnyElement> = match self.detail {
            DetailTab::Findings => issues.iter().map(|f| finding_card(f, &p).into_any_element()).collect(),
            DetailTab::Passed => passed.iter().map(|f| finding_card(f, &p).into_any_element()).collect(),
            DetailTab::Measurements => metrics.iter().map(|m| metric_card(m, &p).into_any_element()).collect(),
        };
        let empty = items.is_empty();
        pane.child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(10.))
                .child(ui::score_chip(page.score, &p))
                .child(div().mono().text_color(p.text).truncate().child(data::path_of(&page.url)))
                .child(ui::spacer())
                .child(ui::micro(meta, p.muted)),
        )
        .when_some(page.error.clone(), |d, e| d.child(div().small().text_color(p.fail).child(e)))
        .child(
            div().flex().flex_none().gap(px(16.)).border_b_1().border_color(p.border).children(tabs.into_iter().map(
                |(tab, text)| {
                    ui::detail_tab(text.clone(), text, self.detail == tab, &p).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.detail = tab;
                            cx.notify();
                        },
                    ))
                },
            )),
        )
        .child(
            div()
                .id("detail-list")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap(px(12.))
                .overflow_y_scroll()
                .children(items)
                .when(empty, |d| d.child(div().body().text_color(p.muted).child("Nothing here."))),
        )
    }

    // ---- Overview, Site-wide, Changes ----

    fn render_overview(&self, report: &AuditReport, cx: &App) -> Div {
        let p = cx.pal();
        let mut cats: Vec<_> = report.categories.iter().collect();
        cats.sort_by_key(|c| c.score);
        let rows = cats.into_iter().map(|c| {
            div()
                .flex()
                .items_center()
                .gap(px(16.))
                .py(px(8.))
                .border_b_1()
                .border_color(p.border)
                .child(div().w(px(180.)).body().text_color(p.text).child(category_name(&c.category)))
                .child(div().flex_1().child(ui::progress_bar(c.score as f32 / 100., None, &p)))
                .child(ui::micro(format!("{} FAIL · {} WARN · {} PASS", c.fail, c.warn, c.pass), p.muted))
                .child(ui::score_chip(c.score, &p))
        });
        let skipped = report.skipped_rules.iter().map(|s| {
            div()
                .flex()
                .gap(px(8.))
                .child(div().mono_small().text_color(p.text).child(s.rule_id.clone()))
                .child(ui::spacer())
                .child(div().small().text_color(p.muted).child(s.reason.clone()))
        });
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.report_kpis(report, cx))
            .child(
                div()
                    .id("overview")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(24.))
                    .py(px(20.))
                    .gap(px(20.))
                    .child(card(&p).child(div().title().child("Categories")).child(div().flex().flex_col().children(rows)))
                    .when(!report.skipped_rules.is_empty(), |d| {
                        d.child(
                            card(&p)
                                .child(div().title().child("Checks that did not run"))
                                .child(div().flex().flex_col().gap(px(6.)).children(skipped)),
                        )
                    }),
            )
    }

    fn render_site_wide(&self, report: &AuditReport, cx: &App) -> Div {
        let p = cx.pal();
        let mut findings: Vec<_> = report.site.findings.iter().collect();
        findings.sort_by_key(|f| match f.finding.status {
            Status::Fail => 0,
            Status::Warn => 1,
            Status::Pass => 3,
            _ => 2,
        });
        let cards = findings.into_iter().map(|sf| {
            let urls = sf.urls.iter().take(8).map(|u| div().mono_small().text_color(p.muted).child(data::path_of(u)));
            let more = sf.urls.len().saturating_sub(8);
            finding_card(&sf.finding, &p)
                .children(urls)
                .when(more > 0, |d| d.child(ui::micro(format!("AND {more} MORE"), p.muted)))
        });
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.report_kpis(report, cx))
            .child(
                div()
                    .id("site-wide")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(24.))
                    .py(px(20.))
                    .gap(px(12.))
                    .children(cards)
                    .when(report.site.findings.is_empty(), |d| {
                        d.child(div().body().text_color(p.muted).child("This report has no site-wide checks."))
                    }),
            )
    }

    fn render_changes(&self, report: &AuditReport, cx: &mut Context<Self>) -> Div {
        let p = cx.pal();
        // From History's "Open the full comparison": the two runs picked there.
        let (title, diff) = match (self.changes_from_history, &self.compare_cache) {
            (true, Some(((a, b), d))) => {
                let site = self.site();
                let n = |id| site.and_then(|s| s.run_number(id)).unwrap_or(0);
                (format!("Run {} to run {}", n(*a), n(*b)), Some((**d).clone()))
            }
            _ => ("Since the last run".to_string(), report.changes.clone()),
        };
        let body = match diff {
            None => div().body().text_color(p.muted).child(
                "This run has nothing to compare with. Turn on \"Compare with the last run\" and audit the site again.",
            ),
            Some(d) => div()
                .flex()
                .flex_col()
                .gap(px(20.))
                .child(
                    div()
                        .flex()
                        .gap(px(24.))
                        .child(ui::kpi_value_unit(data::signed(d.score_change.into()), "points", &p))
                        .child(ui::kpi_value_unit(d.fixed.len().to_string(), "fixed", &p))
                        .child(ui::kpi_value_unit(d.new_failures.len().to_string(), "new", &p))
                        .child(ui::kpi_value_unit(d.pages_added.len().to_string(), "pages added", &p))
                        .child(ui::kpi_value_unit(d.pages_removed.len().to_string(), "pages removed", &p)),
                )
                .child(change_list("fixed", &d.fixed, p.pass, None, &p))
                .child(change_list("new", &d.new_failures, p.fail, None, &p)),
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.report_kpis(report, cx))
            .child(
                div()
                    .id("changes")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(px(24.))
                    .py(px(20.))
                    .child(card(&p).child(div().title().child(title)).child(body)),
            )
    }
}

pub fn card(p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(16.))
        .w_full()
        .px(px(20.))
        .py(px(18.))
        .bg(p.bg_surface)
        .border_1()
        .border_color(p.border)
}

/// A row in a list pane: mono title over a micro subtitle, something on the right.
fn list_row(title: String, sub: String, right: Div, p: &Palette) -> Div {
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
                .child(div().mono_small().text_color(p.text).truncate().child(title))
                .child(div().mono_micro().text_color(p.muted).truncate().child(sub)),
        )
        .child(right)
}

fn category_row(
    id: impl Into<ElementId>,
    name: String,
    count: Option<(String, Hsla)>,
    score: u8,
    active: bool,
    p: &Palette,
) -> Stateful<Div> {
    let name = div().whitespace_nowrap().truncate().child(name);
    let name = if active { name.body_medium().text_color(p.text) } else { name.body().text_color(p.muted) };
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.))
        .px(px(16.))
        .py(px(8.))
        .cursor_pointer()
        .when(active, |d| d.bg(p.bg_raised).child(ui::edge(p)))
        .hover(|s| s.bg(p.bg_raised))
        .child(ui::marker(p.band(score), 8.))
        .child(name)
        .child(ui::spacer())
        .children(count.map(|(t, c)| ui::micro(t, c)))
        .child(div().mono_small().text_color(p.text).child(score.to_string()))
}

/// Status badge, title, rule ID, message, evidence in a sunken mono block, and the fix.
pub fn finding_card(f: &Finding, p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(8.))
        .px(px(16.))
        .py(px(12.))
        .bg(p.bg_surface)
        .border_1()
        .border_color(p.border)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::status_badge(f.status, p))
                .child(div().title().text_color(p.text).whitespace_nowrap().child(f.title.clone()))
                .child(ui::spacer())
                .child(div().mono_small().text_color(p.muted).whitespace_nowrap().child(f.rule_id.clone())),
        )
        .child(div().body().text_color(p.text).child(f.message.clone()))
        .when_some(f.evidence.clone(), |d, e| {
            d.child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .bg(p.bg_app)
                    .border_1()
                    .border_color(p.border)
                    .mono_small()
                    .text_color(p.text)
                    .child(e),
            )
        })
        .when_some(f.fix.clone(), |d, fix| {
            d.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .child(div().mono_label().text_color(p.accent_text).child("FIX"))
                    .child(div().flex_1().min_w_0().small().text_color(p.muted).child(fix)),
            )
        })
}

fn cell_text(c: &MetricCell) -> String {
    match c {
        MetricCell::Number(n) if n.fract() == 0. => format!("{n:.0}"),
        MetricCell::Number(n) => format!("{n:.2}"),
        MetricCell::Text(t) => t.clone(),
    }
}

fn metric_card(m: &Metric, p: &Palette) -> Div {
    let color = match m.status {
        Some(ValueStatus::Ok) => p.pass,
        Some(ValueStatus::Warn) => p.warn,
        Some(ValueStatus::Bad) => p.fail,
        _ => p.skip,
    };
    let body = match &m.body {
        MetricBody::Value { value, unit } => {
            let v = value.as_ref().map_or("—".to_string(), cell_text);
            div().mono().text_color(p.text).child(format!("{v} {}", unit.clone().unwrap_or_default()))
        }
        MetricBody::Table { columns, rows, total } => {
            let cols = columns.len().max(1);
            let row = |cells: Vec<String>, head: bool| {
                div().flex().gap(px(12.)).children(cells.into_iter().take(cols).map(move |c| {
                    let d = div().flex_1().min_w_0().truncate().child(c);
                    if head { d.mono_label().text_color(p.muted) } else { d.mono_small().text_color(p.text) }
                }))
            };
            let shown = rows.iter().take(20).map(|r| row(r.iter().map(cell_text).collect(), false));
            let more = (*total as usize).saturating_sub(rows.len().min(20));
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(row(columns.clone(), true))
                .children(shown)
                .when(more > 0, |d| d.child(ui::micro(format!("AND {more} MORE"), p.muted)))
        }
        _ => div(),
    };
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(8.))
        .px(px(16.))
        .py(px(12.))
        .bg(p.bg_surface)
        .border_1()
        .border_color(p.border)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(ui::marker(color, 6.))
                .child(div().title().text_color(p.text).child(m.label.clone()))
                .child(ui::spacer())
                .child(div().mono_small().text_color(p.muted).child(m.id.clone())),
        )
        .child(body)
        .when_some(m.note.clone(), |d, n| d.child(div().small().text_color(p.muted).child(n)))
}

/// "41 fixed" with rule IDs and how many pages each touched. `limit` caps the rows.
pub fn change_list(word: &str, refs: &[FindingRef], color: Hsla, limit: Option<usize>, p: &Palette) -> Div {
    let mut by_rule: BTreeMap<&str, usize> = BTreeMap::new();
    for r in refs {
        *by_rule.entry(r.rule_id.as_str()).or_default() += 1;
    }
    let mut rules: Vec<_> = by_rule.into_iter().collect();
    rules.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let rows = rules.into_iter().take(limit.unwrap_or(usize::MAX)).map(|(rule, n)| {
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().mono_small().text_color(p.text).truncate().child(rule.to_string()))
            .child(ui::spacer())
            .child(ui::micro(format!("{n} {}", if n == 1 { "PAGE" } else { "PAGES" }), p.muted))
    });
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(ui::marker(color, 6.))
                .child(div().small_medium().text_color(p.text).child(format!("{} {word}", refs.len()))),
        )
        .children(rows)
}
