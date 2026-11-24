//! History tab: score by run (gpui-component's LineChart), compare two runs, and the run list.

use std::collections::BTreeSet;
use std::sync::Arc;

use gpui_kit::component::chart::LineChart;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sitevet::{AuditReport, RunId};

use crate::app::{AppView, OpenSelect, Range, Tab};
use crate::data::{self, category_name, time};
use crate::screens::report::{card, change_list};
use crate::theme::{ActivePalette, Palette, TextStyles};
use crate::ui;

#[derive(Clone)]
struct Point {
    label: SharedString,
    score: f64,
    head: String,
    delta: String,
    counts: String,
}

impl AppView {
    fn score_in(&self, report: &AuditReport) -> Option<u8> {
        match &self.metric {
            None => Some(report.score),
            Some(cat) => report
                .categories
                .iter()
                .find(|c| &c.category == cat)
                .map(|c| c.score),
        }
    }

    pub fn render_history(&mut self, cx: &mut Context<Self>) -> Div {
        let p = cx.pal();
        let Some(site) = self.site().cloned() else {
            return div();
        };
        let runs = &site.runs;
        let first = &runs[0];
        let last = site.latest_run();
        let a = self.compare.0.unwrap_or(first.id);
        let b = self.compare.1.unwrap_or(last.id);
        let n = |id: RunId| site.run_number(id).unwrap_or(0);

        // Diff between the picked runs, kept until the pick changes.
        let diff = match (self.reports.get(&a), self.reports.get(&b)) {
            (Some(ra), Some(rb)) => {
                if self
                    .compare_cache
                    .as_ref()
                    .is_none_or(|(k, _)| *k != (a, b))
                {
                    self.compare_cache = Some(((a, b), Arc::new(sitevet::diff(ra, rb))));
                }
                self.compare_cache.as_ref().map(|(_, d)| d.clone())
            }
            _ => None,
        };

        let kpis = div()
            .flex()
            .flex_none()
            .h(px(66.))
            .border_b_1()
            .border_color(p.border)
            .child(ui::kpi(
                "LATEST",
                None,
                ui::kpi_value_unit(last.score.to_string(), format!("RUN {}", runs.len()), &p),
                &p,
            ))
            .child(ui::kpi(
                "FIRST RUN",
                None,
                ui::kpi_value_unit(first.score.to_string(), "RUN 1", &p),
                &p,
            ))
            .child(ui::kpi(
                "CHANGE",
                None,
                ui::kpi_value_unit(
                    data::signed(last.score as i64 - first.score as i64),
                    "POINTS",
                    &p,
                ),
                &p,
            ))
            .child(ui::kpi(
                "RUNS",
                None,
                ui::kpi_value_unit(
                    runs.len().to_string(),
                    format!(
                        "{} DAYS",
                        time::days_between(&first.started_at, &last.started_at)
                    ),
                    &p,
                ),
                &p,
            ))
            .child(ui::kpi(
                "FIXED",
                Some(p.pass),
                ui::kpi_value_unit(
                    diff.as_ref()
                        .map_or("—".into(), |d| d.fixed.len().to_string()),
                    "ISSUES",
                    &p,
                ),
                &p,
            ))
            .child(ui::kpi(
                "NEW",
                Some(p.fail),
                ui::kpi_value_unit(
                    diff.as_ref()
                        .map_or("—".into(), |d| d.new_failures.len().to_string()),
                    "ISSUES",
                    &p,
                ),
                &p,
            ));

        let charts = div()
            .id("history-charts")
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap(px(20.))
            .px(px(24.))
            .py(px(20.))
            .overflow_y_scroll()
            .child(self.score_card(&site, cx))
            .child(self.compare_card(&site, a, b, cx));

        // Runs, newest first.
        let mut prev_score: Option<u8> = None;
        let mut rows: Vec<AnyElement> = Vec::new();
        for r in runs.iter() {
            let delta = prev_score.map_or("FIRST".to_string(), |s| {
                data::signed(r.score as i64 - s as i64)
            });
            prev_score = Some(r.score);
            let (tag_bg, tag) = if r.id == b {
                (Some(p.accent_bg), "B")
            } else if r.id == a {
                (Some(p.muted), "A")
            } else {
                (None, "")
            };
            let id = r.id;
            rows.push(
                div()
                    .id(("run-row", id as u64))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(12.))
                    .px(px(16.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(p.border)
                    .cursor_pointer()
                    .when(tag_bg.is_some(), |d| d.bg(p.bg_raised))
                    .hover(|s| s.bg(p.bg_raised))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.compare.1 = Some(id);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id(("run-tag", id as u64))
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(18.))
                            .map(|d| match tag_bg {
                                Some(bg) => {
                                    d.bg(bg).mono_micro().text_color(p.accent_on).child(tag)
                                }
                                None => d.border_1().border_color(p.border),
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.compare.0 = Some(id);
                                cx.stop_propagation();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap(px(2.))
                            .child(
                                div()
                                    .mono_small()
                                    .text_color(p.text)
                                    .child(format!("Run {}", n(r.id))),
                            )
                            .child(ui::micro(
                                format!("{} · {} PAGES", time::date_time(&r.started_at), r.pages),
                                p.muted,
                            )),
                    )
                    .child(ui::micro(delta, p.muted))
                    .child(ui::score_chip(r.score, &p))
                    .into_any_element(),
            );
        }
        rows.reverse();

        let changes = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(16.))
            .child(ui::label(format!("RUN {} TO RUN {}", n(a), n(b)), &p))
            .children(
                diff.as_ref()
                    .map(|d| change_list("fixed", &d.fixed, p.pass, Some(3), &p)),
            )
            .children(
                diff.as_ref()
                    .map(|d| change_list("new", &d.new_failures, p.fail, Some(3), &p)),
            )
            .when(diff.is_none(), |d| {
                d.child(self.compare_note(&site, a, b, &p))
            })
            .child(
                ui::link("full-compare", "Open the full comparison", &p).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.set_tab(Tab::Changes, cx);
                        this.changes_from_history = true;
                    },
                )),
            );

        let runs_pane = div()
            .id("runs-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(380.))
            .h_full()
            .bg(p.bg_sunken)
            .border_l_1()
            .border_color(p.border)
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .px(px(16.))
                    .py(px(14.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(ui::label("RUNS", &p))
                    .child(ui::spacer())
                    .child(
                        ui::link("rerun", "Run audit again", &p)
                            .on_click(cx.listener(|this, _, _, cx| this.rerun(cx))),
                    ),
            )
            .children(rows)
            .child(changes);

        div().flex().flex_col().size_full().child(kpis).child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .child(charts)
                .child(runs_pane),
        )
    }

    fn score_card(&mut self, site: &data::Site, cx: &mut Context<Self>) -> Div {
        let p = cx.pal();
        let runs: Vec<_> = site
            .runs
            .iter()
            .enumerate()
            .filter(|(_, r)| match self.range {
                Range::All => true,
                Range::Days30 => time::age_days(&r.started_at) <= 30,
                Range::Days90 => time::age_days(&r.started_at) <= 90,
            })
            .collect();
        let mut points = Vec::new();
        let mut prev: Option<(usize, u8)> = None;
        for (i, r) in &runs {
            let report = self.reports.get(&r.id);
            let score = match (&self.metric, report) {
                (None, _) => Some(r.score),
                (Some(_), Some(rep)) => self.score_in(rep),
                _ => None,
            };
            let Some(score) = score else { continue };
            let counts = report.map_or(format!("{} PAGES", r.pages), |rep| {
                let (_, warn, fail) = data::totals(rep);
                format!("{fail} FAIL · {warn} WARN · {} PAGES", r.pages)
            });
            points.push(Point {
                label: format!("RUN {}", i + 1).into(),
                score: score as f64,
                head: format!("RUN {} · {}", i + 1, time::date(&r.started_at)),
                delta: prev.map_or("FIRST RUN".into(), |(n, s)| {
                    format!("{} VS RUN {n}", data::signed(score as i64 - s as i64))
                }),
                counts,
            });
            prev = Some((i + 1, score));
        }
        let span = match (runs.first(), runs.last()) {
            (Some((_, a)), Some((_, b))) => {
                format!(
                    "{} runs from {} to {}. Hover a run for its numbers.",
                    runs.len(),
                    time::date(&a.started_at),
                    time::date(&b.started_at)
                )
            }
            _ => "No runs in this range.".into(),
        };
        // Same 60 to 100 axis as the design unless a score falls below it.
        let low = points.iter().map(|pt| pt.score).fold(60., f64::min);
        let low = (low / 10.).floor() * 10.;

        let metric_label = self
            .metric
            .as_deref()
            .map_or("Overall".to_string(), category_name);
        let categories: BTreeSet<String> = site
            .latest
            .categories
            .iter()
            .map(|c| c.category.clone())
            .collect();
        let open = self.open_select == OpenSelect::Metric;
        let metric_menu = open.then(|| {
            let mut items = vec![
                ui::select_item("m-all", "Overall", self.metric.is_none(), &p)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.metric = None;
                            this.open_select = OpenSelect::None;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .into_any_element(),
            ];
            for c in categories {
                let active = self.metric.as_deref() == Some(c.as_str());
                let name = category_name(&c);
                items.push(
                    ui::select_item(SharedString::from(format!("m-{c}")), name, active, &p)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.metric = Some(c.clone());
                                this.open_select = OpenSelect::None;
                                this.load_history(cx);
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        )
                        .into_any_element(),
                );
            }
            ui::select_menu(items, &p)
        });

        let range_chip = |id: &'static str,
                          text: &'static str,
                          r: Range,
                          cx: &mut Context<Self>| {
            ui::chip(id, text, self.range == r, &p).on_click(cx.listener(move |this, _, _, cx| {
                this.range = r;
                cx.notify();
            }))
        };

        let pal = p;
        let chart = LineChart::new(points.clone())
            .x(|pt: &Point| pt.label.clone())
            .y(|pt: &Point| pt.score)
            .linear()
            .dot()
            .stroke(p.accent_text)
            .y_domain(low, 100.)
            .y_padding(0., 0.)
            .y_axis(true)
            .y_tick_count(((100. - low) / 10.) as usize + 1)
            .y_tick_format(|v| format!("{v:.0}"))
            // The design's "PASS 90+" guide.
            .reference_line(90.)
            .tooltip_content(move |pt: &Point, _, _| tooltip(pt, &pal));

        card(&p)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .title()
                                    .text_color(p.text)
                                    .child("Overall score by run"),
                            )
                            .child(div().small().text_color(p.muted).child(span)),
                    )
                    .child(ui::spacer())
                    .child(
                        div()
                            .child(ui::select_box("metric", metric_label, &p).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, _, cx| {
                                    this.open_select = if this.open_select == OpenSelect::Metric {
                                        OpenSelect::None
                                    } else {
                                        OpenSelect::Metric
                                    };
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            ))
                            .children(metric_menu),
                    )
                    .child(range_chip("r-all", "All runs", Range::All, cx))
                    .child(range_chip("r-30", "30 days", Range::Days30, cx))
                    .child(range_chip("r-90", "90 days", Range::Days90, cx)),
            )
            .child(
                div()
                    .h(px(230.))
                    .w_full()
                    .when(!points.is_empty(), |d| d.child(chart)),
            )
    }

    fn compare_card(
        &mut self,
        site: &data::Site,
        a: RunId,
        b: RunId,
        cx: &mut Context<Self>,
    ) -> Div {
        let p = cx.pal();
        let n = |id: RunId| site.run_number(id).unwrap_or(0);
        let label = |id: RunId| {
            let r = site.runs.iter().find(|r| r.id == id);
            format!(
                "Run {} · {}",
                n(id),
                r.map_or(String::new(), |r| time::date(&r.started_at))
            )
        };
        let ra = self.reports.get(&a).cloned();
        let rb = self.reports.get(&b).cloned();

        let menu = |which: OpenSelect, current: RunId, cx: &mut Context<Self>| {
            (self.open_select == which).then(|| {
                let items = site
                    .runs
                    .iter()
                    .rev()
                    .map(|r| {
                        let id = r.id;
                        ui::select_item(("cmp", id as u64), label(id), id == current, &p)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    match which {
                                        OpenSelect::CompareA => this.compare.0 = Some(id),
                                        _ => this.compare.1 = Some(id),
                                    }
                                    this.open_select = OpenSelect::None;
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                            .into_any_element()
                    })
                    .collect();
                ui::select_menu(items, &p)
            })
        };
        let select = |id: &'static str, which: OpenSelect, run: RunId, cx: &mut Context<Self>| {
            let m = menu(which, run, cx);
            div()
                .child(ui::select_box(id, label(run), &p).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        this.open_select = if this.open_select == which {
                            OpenSelect::None
                        } else {
                            which
                        };
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ))
                .children(m)
        };

        let col = |text: String, w: f32, right: bool| {
            div()
                .w(px(w))
                .flex_none()
                .flex()
                .when(right, |d| d.justify_end())
                .child(ui::label(text, &p))
        };
        let mut body = div().flex().flex_col().gap(px(8.)).child(
            div()
                .flex()
                .gap(px(16.))
                .child(col("CATEGORY".into(), 140., false))
                .child(div().flex_1().child(ui::label("SCORE", &p)))
                .child(col(format!("RUN {}", n(a)), 50., true))
                .child(col(format!("RUN {}", n(b)), 50., true))
                .child(col("CHANGE".into(), 60., true)),
        );
        match (ra, rb) {
            (Some(ra), Some(rb)) => {
                let mut cats: Vec<String> = rb
                    .categories
                    .iter()
                    .chain(&ra.categories)
                    .map(|c| c.category.clone())
                    .collect();
                cats.dedup();
                let mut seen = BTreeSet::new();
                cats.retain(|c| seen.insert(c.clone()));
                let score = |r: &AuditReport, c: &str| {
                    r.categories
                        .iter()
                        .find(|x| x.category == c)
                        .map(|x| x.score)
                };
                // Biggest gain first, as in the design.
                let change_of = |c: &str| match (score(&ra, c), score(&rb, c)) {
                    (Some(x), Some(y)) => y as i64 - x as i64,
                    _ => i64::MIN,
                };
                cats.sort_by_key(|c| std::cmp::Reverse(change_of(c)));
                for c in cats {
                    let (sa, sb) = (score(&ra, &c), score(&rb, &c));
                    let num = |s: Option<u8>, color| {
                        div()
                            .w(px(50.))
                            .flex_none()
                            .flex()
                            .justify_end()
                            .mono_small()
                            .text_color(color)
                            .child(s.map_or("—".into(), |s| s.to_string()))
                    };
                    let change = match (sa, sb) {
                        (Some(x), Some(y)) => data::signed(y as i64 - x as i64),
                        _ => "—".into(),
                    };
                    let bar = |s: Option<u8>, color| {
                        div()
                            .h(px(5.))
                            .w(relative(s.unwrap_or(0) as f32 / 100.))
                            .bg(color)
                    };
                    body = body.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(16.))
                            .py(px(4.))
                            .child(
                                div()
                                    .w(px(140.))
                                    .flex_none()
                                    .body()
                                    .text_color(p.text)
                                    .truncate()
                                    .child(category_name(&c)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .gap(px(2.))
                                    .child(bar(sa, p.muted))
                                    .child(bar(sb, p.accent_text)),
                            )
                            .child(num(sa, p.muted))
                            .child(num(sb, p.text))
                            .child(
                                div()
                                    .w(px(60.))
                                    .flex_none()
                                    .flex()
                                    .justify_end()
                                    .mono_small()
                                    .text_color(p.text)
                                    .child(change),
                            ),
                    );
                }
            }
            _ => body = body.child(self.compare_note(site, a, b, &p)),
        }

        card(&p)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(div().title().text_color(p.text).child("Compare"))
                    .child(select("cmp-a", OpenSelect::CompareA, a, cx))
                    .child(div().small().text_color(p.muted).child("to"))
                    .child(select("cmp-b", OpenSelect::CompareB, b, cx))
                    .child(ui::spacer())
                    .child(legend(p.muted, format!("RUN {}", n(a)), &p))
                    .child(legend(p.accent_bg, format!("RUN {}", n(b)), &p)),
            )
            .child(body)
    }
}

impl AppView {
    /// What to show while the picked runs are not both loaded.
    fn compare_note(&self, site: &data::Site, a: RunId, b: RunId, p: &Palette) -> Div {
        let failed = [a, b]
            .into_iter()
            .find_map(|id| self.load_failed.get(&id).map(|e| (id, e)));
        match failed {
            Some((id, e)) => div().small().text_color(p.fail).child(format!(
                "Run {} could not be read: {e}",
                site.run_number(id).unwrap_or(0)
            )),
            None => div().small().text_color(p.muted).child("Loading runs"),
        }
    }
}

fn legend(color: Hsla, text: String, p: &Palette) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .child(ui::marker(color, 10.))
        .child(ui::micro(text, p.muted))
}

fn tooltip(pt: &Point, p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .px(px(12.))
        .py(px(10.))
        .bg(p.bg_raised)
        .border_1()
        .border_color(p.border_strong)
        .child(ui::micro(pt.head.clone(), p.muted))
        .child(
            div()
                .flex()
                .items_baseline()
                .gap(px(8.))
                .child(
                    div()
                        .title()
                        .text_color(p.text)
                        .child(format!("{:.0}", pt.score)),
                )
                .child(ui::micro(pt.delta.clone(), p.muted)),
        )
        .child(ui::micro(pt.counts.clone(), p.muted))
}
