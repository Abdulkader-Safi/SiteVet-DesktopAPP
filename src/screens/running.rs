//! Progress screens: one audit, and several sites at a time.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{AppView, LiveRun, RunStatus, Screen};
use crate::data;
use crate::theme::{ActivePalette, Palette, TextStyles};
use crate::ui;

fn strip(p: &Palette) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(66.))
        .border_b_1()
        .border_color(p.border)
}

fn progress(title: String, sub: String, fraction: f32, p: &Palette) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap(px(10.))
        .px(px(20.))
        .py(px(16.))
        .border_b_1()
        .border_color(p.border)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .body_medium()
                        .text_color(p.text)
                        .whitespace_nowrap()
                        .child(title),
                )
                .child(
                    div()
                        .small()
                        .text_color(p.muted)
                        .whitespace_nowrap()
                        .child(sub),
                )
                .child(ui::spacer())
                .child(
                    div()
                        .mono_small()
                        .text_color(p.text)
                        .child(format!("{:.0}%", fraction * 100.)),
                ),
        )
        .child(ui::progress_bar(fraction, None, p))
}

fn status_line(run: &LiveRun, p: &Palette) -> Option<Div> {
    match &run.status {
        RunStatus::Failed(e) => Some(
            div()
                .px(px(20.))
                .py(px(12.))
                .small()
                .text_color(p.fail)
                .child(e.clone()),
        ),
        RunStatus::Cancelled => Some(
            div()
                .px(px(20.))
                .py(px(12.))
                .small()
                .text_color(p.muted)
                .child("Cancelled."),
        ),
        RunStatus::Saving => Some(
            div()
                .px(px(20.))
                .py(px(12.))
                .small()
                .text_color(p.muted)
                .child("Saving to history…"),
        ),
        _ => None,
    }
}

impl AppView {
    pub fn render_running(&mut self, idx: usize, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal();
        let run = &self.runs[idx];
        let (done, total) = run.pages();
        let crawling = run.pages_total.is_none();
        let checks = if run.checks_total > 0 {
            format!(
                "{} / {}",
                data::thousands(run.checks_done.into()),
                data::thousands(run.checks_total.into())
            )
        } else {
            "—".into()
        };
        let per_page = match run.pages_total {
            Some(n) if n > 0 => format!("{} checks per page · ", run.checks_total / n),
            _ => format!("{} pages found · ", run.fetched.len()),
        };
        let sub = format!(
            "{per_page}{} requests at a time",
            run.settings.options.concurrency
        );
        let title = if crawling {
            "Crawling pages"
        } else {
            "Crawling and checking pages"
        };

        // Pages still in flight: fetched but not finished, newest first, as many as run at once.
        let finished: std::collections::HashSet<&str> =
            run.completed.iter().map(|(u, _)| u.as_str()).collect();
        let in_flight: Vec<&String> = if run.status == RunStatus::Running {
            run.fetched
                .iter()
                .rev()
                .filter(|u| !finished.contains(u.as_str()))
                .take(run.settings.options.concurrency as usize)
                .collect()
        } else {
            Vec::new()
        };
        let row = |left: String, path: String, dim: bool, right: Div| {
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(16.))
                .px(px(20.))
                .py(px(10.))
                .border_b_1()
                .border_color(p.border)
                .child(
                    div()
                        .w(px(52.))
                        .flex_none()
                        .mono_micro()
                        .text_color(p.muted)
                        .child(left),
                )
                .child(
                    div()
                        .mono_small()
                        .text_color(if dim { p.muted } else { p.text })
                        .truncate()
                        .child(path),
                )
                .child(ui::spacer())
                .child(right)
        };
        let checking = in_flight.into_iter().map(|u| {
            row(
                "—".into(),
                data::path_of(u),
                true,
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(ui::marker(p.info, 6.))
                    .child(ui::micro("CHECKING", p.muted)),
            )
        });
        let total_pages = run.pages_total.unwrap_or(run.max_pages);
        let finished_rows = run
            .completed
            .iter()
            .enumerate()
            .rev()
            .map(|(i, (u, score))| {
                row(
                    format!("{}/{total_pages}", i + 1),
                    data::path_of(u),
                    false,
                    ui::score_chip(*score, &p),
                )
            });

        let active = run.active();
        let finished_report =
            matches!(run.status, RunStatus::Done).then(|| data::host_of(&run.url).to_string());

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                strip(&p)
                    .child(ui::kpi(
                        "PAGES",
                        None,
                        ui::kpi_value(format!("{done} / {total}"), &p),
                        &p,
                    ))
                    .child(ui::kpi(
                        "ELAPSED",
                        None,
                        ui::kpi_value(data::duration_label(run.elapsed()), &p),
                        &p,
                    ))
                    .child(ui::kpi(
                        "FAIL",
                        Some(p.fail),
                        ui::kpi_value(data::thousands(run.fail.into()), &p),
                        &p,
                    ))
                    .child(ui::kpi(
                        "WARN",
                        Some(p.warn),
                        ui::kpi_value(data::thousands(run.warn.into()), &p),
                        &p,
                    ))
                    .child(ui::kpi(
                        "PASS",
                        Some(p.pass),
                        ui::kpi_value(data::thousands(run.pass.into()), &p),
                        &p,
                    ))
                    .child(ui::kpi("CHECKS", None, ui::kpi_value(checks, &p), &p))
                    .child(div().flex_none().px(px(20.)).map(|d| {
                        if active {
                            d.child(
                                ui::button_secondary("cancel", "Cancel", 36., &p).on_click(
                                    cx.listener(move |this, _, _, cx| this.cancel(idx, cx)),
                                ),
                            )
                        } else if let Some(host) = finished_report.clone() {
                            d.child(ui::button_primary("open", "Open report", 36., &p).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    if let Some(o) = this
                                        .sites
                                        .iter()
                                        .find(|s| s.host() == host)
                                        .map(|s| s.origin.clone())
                                    {
                                        this.open_site(&o, cx);
                                    }
                                }),
                            ))
                        } else {
                            d.child(ui::button_secondary("back", "New audit", 36., &p).on_click(
                                cx.listener(|this, _, window, cx| this.new_audit(window, cx)),
                            ))
                        }
                    })),
            )
            .child(progress(title.into(), sub, run.fraction(), &p))
            .children(status_line(run, &p))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .px(px(20.))
                    .py(px(14.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(ui::label("PAGES AS THEY FINISH", &p))
                    .child(ui::spacer())
                    .child(ui::micro("NEWEST FIRST", p.muted)),
            )
            .child(
                div()
                    .id("live-log")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .children(checking)
                    .children(finished_rows),
            )
    }

    pub fn render_batch(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal();
        let Some(batch) = &self.batch else {
            return div();
        };
        let runs: Vec<(usize, &LiveRun)> = batch.runs.iter().map(|&i| (i, &self.runs[i])).collect();
        let n = runs.len();
        let done = runs
            .iter()
            .filter(|(_, r)| r.status == RunStatus::Done)
            .count();
        let (fail, warn, pass, pages) =
            runs.iter()
                .fold((0u64, 0u64, 0u64, 0u64), |(f, w, ps, pg), (_, r)| {
                    (
                        f + r.fail as u64,
                        w + r.warn as u64,
                        ps + r.pass as u64,
                        pg + r.pages().0 as u64,
                    )
                });
        let fraction = if n == 0 {
            0.
        } else {
            runs.iter().map(|(_, r)| r.fraction()).sum::<f32>() / n as f32
        };
        let any_active = runs.iter().any(|(_, r)| r.active());
        let elapsed = batch.started.elapsed().as_millis() as u64;
        let left = if any_active && fraction > 0.02 {
            let rest = (elapsed as f32 / fraction * (1. - fraction)) as u64;
            format!(" · about {} min left", rest.div_ceil(60_000).max(1))
        } else {
            String::new()
        };

        // Column widths from the design.
        const STATUS: f32 = 154.;
        const PROGRESS: f32 = 237.;
        const PAGES: f32 = 98.;
        const SCORE: f32 = 89.;
        const VS: f32 = 150.;
        const ACTION: f32 = 90.;
        let head = |t: &'static str, w: Option<f32>| {
            let d = div().flex_none().child(ui::label(t, &p));
            match w {
                Some(w) => d.w(px(w)),
                None => d.flex_1(),
            }
        };
        let rows = runs.iter().map(|&(i, r)| {
            let (word, color) = match r.status {
                RunStatus::Done => ("DONE", p.pass),
                RunStatus::Running | RunStatus::Saving => ("RUNNING", p.info),
                RunStatus::Queued => ("QUEUED", p.skip),
                RunStatus::Failed(_) => ("FAILED", p.fail),
                RunStatus::Cancelled => ("CANCELLED", p.skip),
            };
            let (pd, pt) = r.pages();
            let pages_text = match r.status {
                RunStatus::Queued => "—".to_string(),
                RunStatus::Done => pt.to_string(),
                RunStatus::Failed(_) | RunStatus::Cancelled => pd.to_string(),
                _ => format!("{pd} / {pt}"),
            };
            let score = r.report.as_ref().map(|rep| rep.score);
            let vs = match (r.prev, score) {
                (Some((runs, last)), Some(s)) => {
                    format!("{} since run {runs}", data::signed(s as i64 - last as i64))
                }
                (None, _) if !matches!(r.status, RunStatus::Failed(_)) => "first run".into(),
                _ => "—".into(),
            };
            let progress_cell = match &r.status {
                RunStatus::Failed(e) => {
                    div().small().text_color(p.fail).truncate().child(e.clone())
                }
                RunStatus::Queued => div(),
                _ => div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(ui::progress_bar(r.fraction(), Some(148.), &p))
                    .child(ui::micro(format!("{:.0}%", r.fraction() * 100.), p.muted)),
            };
            let host = data::host_of(&r.url).to_string();
            let host_label = host.clone();
            let action = match r.status {
                RunStatus::Done => Some(ui::link(("open", i), "Open report", &p).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if let Some(o) = this
                            .sites
                            .iter()
                            .find(|s| s.host() == host)
                            .map(|s| s.origin.clone())
                        {
                            this.open_site(&o, cx);
                        }
                    }),
                )),
                RunStatus::Running | RunStatus::Saving => {
                    Some(ui::link(("watch", i), "Watch", &p).on_click(cx.listener(
                        move |this, _, _, cx| {
                            this.screen = Screen::Running(i);
                            cx.notify();
                        },
                    )))
                }
                RunStatus::Queued => Some(
                    ui::link(("remove", i), "Remove", &p)
                        .text_color(p.muted)
                        .on_click(cx.listener(move |this, _, _, cx| this.remove_queued(i, cx))),
                ),
                RunStatus::Failed(_) | RunStatus::Cancelled => Some(
                    ui::link(("retry", i), "Retry", &p)
                        .on_click(cx.listener(move |this, _, _, cx| this.retry(i, cx))),
                ),
            };
            let running = matches!(r.status, RunStatus::Running | RunStatus::Saving);
            div()
                .flex()
                .flex_none()
                .items_center()
                .h(px(if running { 42. } else { 45. }))
                .px(px(20.))
                .border_b_1()
                .border_color(p.border)
                .when(running, |d| d.bg(p.bg_raised))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .mono()
                        .text_color(p.text)
                        .truncate()
                        .child(host_label),
                )
                .child(
                    div()
                        .w(px(STATUS))
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::marker(color, 6.))
                        .child(ui::micro(word, p.muted)),
                )
                .child(div().w(px(PROGRESS)).flex_none().child(progress_cell))
                .child(
                    div()
                        .w(px(PAGES))
                        .flex_none()
                        .mono_small()
                        .text_color(p.muted)
                        .child(pages_text),
                )
                .child(div().w(px(SCORE)).flex_none().map(|d| match score {
                    Some(s) => d.child(ui::score_chip(s, &p)),
                    None => d.mono_small().text_color(p.muted).child("—"),
                }))
                .child(
                    div()
                        .w(px(VS))
                        .flex_none()
                        .mono_small()
                        .text_color(if r.prev.is_some() && score.is_some() {
                            p.text
                        } else {
                            p.muted
                        })
                        .child(vs),
                )
                .child(
                    div()
                        .w(px(ACTION))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .children(action),
                )
        });

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                strip(&p)
                    .child(ui::kpi("SITES", None, ui::kpi_value(format!("{done} / {n} done"), &p), &p))
                    .child(ui::kpi("ELAPSED", None, ui::kpi_value(data::duration_label(elapsed), &p), &p))
                    .child(ui::kpi("FAIL", Some(p.fail), ui::kpi_value(data::thousands(fail), &p), &p))
                    .child(ui::kpi("WARN", Some(p.warn), ui::kpi_value(data::thousands(warn), &p), &p))
                    .child(ui::kpi("PASS", Some(p.pass), ui::kpi_value(data::thousands(pass), &p), &p))
                    .child(ui::kpi("PAGES", None, ui::kpi_value(data::thousands(pages), &p), &p))
                    .child(div().flex_none().px(px(20.)).when(any_active, |d| {
                        d.child(
                            ui::button_secondary("cancel-all", "Cancel all", 36., &p)
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_all(cx))),
                        )
                    })),
            )
            .child(progress(
                format!("Auditing {n} sites"),
                format!("{} at a time{left}", batch.parallel),
                fraction,
                &p,
            ))
            .child(
                div()
                    .id("batch-table")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .px(px(20.))
                            .py(px(14.))
                            .border_b_1()
                            .border_color(p.border)
                            .child(head("SITE", None))
                            .child(head("STATUS", Some(STATUS)))
                            .child(head("PROGRESS", Some(PROGRESS)))
                            .child(head("PAGES", Some(PAGES)))
                            .child(head("SCORE", Some(SCORE)))
                            .child(head("VS LAST RUN", Some(VS)))
                            .child(div().w(px(ACTION)).flex_none()),
                    )
                    .children(rows)
                    .child(
                        div()
                            .px(px(20.))
                            .py(px(16.))
                            .small()
                            .text_color(p.muted)
                            .child("Finished sites open like any report. A failed site does not stop the others."),
                    ),
            )
    }
}
