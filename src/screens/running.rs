//! Progress screens: one audit, and several sites at a time.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{AppView, LiveRun, LogLevel, RunStatus, Screen, clock};
use crate::data;
use crate::theme::{ActivePalette, Palette, TextStyles};
use crate::ui;

/// A tab over the running screen's list: label over a 2px accent underline.
fn list_tab(id: &'static str, text: String, active: bool, p: &Palette) -> Stateful<Div> {
    let label = div()
        .mono_label()
        .whitespace_nowrap()
        .child(text.to_uppercase());
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(12.))
        .pt(px(14.))
        .cursor_pointer()
        .child(if active {
            label.text_color(p.text)
        } else {
            label.text_color(p.muted)
        })
        .child(div().h(px(2.)).w_full().when(active, |d| d.bg(p.accent_bg)))
}

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

/// Seconds of silence before the activity line turns amber.
const QUIET_WARN_MS: u64 = 30_000;

/// What the engine is doing while it says nothing, in words.
fn quiet_hint(run: &LiveRun) -> String {
    match (run.pages_total, run.rendered) {
        (None, Some((done, total))) => format!(
            "Chrome is rendering page {} of {total}. A slow page can take up to a minute.",
            (done + 1).min(total)
        ),
        (None, None) => match run.fetched.last() {
            Some(f) => format!(
                "It is reading the pages it fetched. The last one was {} ({} in {} ms).",
                data::path_of(&f.url),
                if f.status == 0 { "no response".to_string() } else { f.status.to_string() },
                f.ms
            ),
            None => "It is fetching the start page, robots.txt and the sitemap.".into(),
        },
        (Some(_), _) => "It is checking links on the remaining pages. Sites with many links to other sites take longer.".into(),
    }
}

fn activity_line(run: &LiveRun, p: &Palette) -> Option<Div> {
    let quiet = run.quiet_ms()?;
    let warn = quiet >= QUIET_WARN_MS;
    let ago = data::duration_label(quiet);
    let row = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .px(px(20.))
        .py(px(10.))
        .border_b_1()
        .border_color(p.border);
    Some(if warn {
        row.bg(p.bg_sunken)
            .child(ui::marker(p.warn, 8.))
            .child(div().small().text_color(p.text).child(format!(
            "No update from the audit for {ago}. {} Open the activity log to see the last step.",
            quiet_hint(run)
        )))
    } else {
        row.child(ui::marker(p.pass, 6.)).child(ui::micro(
            format!("WORKING · LAST UPDATE {} AGO", ago.to_uppercase()),
            p.muted,
        ))
    })
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
                .child("Saving to history"),
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
        let concurrency = run.settings.options.concurrency;
        let (title, sub) = match (run.pages_total, run.rendered) {
            (Some(n), _) => (
                "Checking pages",
                format!(
                    "{} checks per page · {concurrency} requests at a time",
                    run.checks_total / n.max(1)
                ),
            ),
            (None, Some((done, total))) => (
                "Rendering pages in Chrome",
                format!("{done} of {total} pages · axe-core and Core Web Vitals"),
            ),
            (None, None) => (
                "Crawling pages",
                format!(
                    "{} pages found · {concurrency} requests at a time",
                    run.fetched.len()
                ),
            ),
        };
        let failed = run.failed_fetches;

        // Pages still in flight: fetched but not finished, newest first, as many as run at once.
        let finished: std::collections::HashSet<&str> =
            run.completed.iter().map(|(u, _)| u.as_str()).collect();
        let in_flight: Vec<&crate::app::Fetched> = if run.status == RunStatus::Running {
            run.fetched
                .iter()
                .rev()
                .filter(|f| !finished.contains(f.url.as_str()))
                .take(concurrency as usize)
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
        let checking = in_flight.into_iter().map(|f| {
            let (color, word) = match (crawling, f.status) {
                (_, 0) => (p.fail, "NO RESPONSE".to_string()),
                (true, s) => (
                    if s >= 400 { p.warn } else { p.info },
                    format!("FETCHED {s} · {} MS", f.ms),
                ),
                (false, _) => (p.info, "CHECKING".to_string()),
            };
            row(
                "—".into(),
                data::path_of(&f.url),
                true,
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(ui::marker(color, 6.))
                    .child(ui::micro(word, p.muted)),
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

        let show_log = self.run_log;
        let log_len = run.log.len();
        let log_rows: Vec<Div> = if show_log {
            run.log
                .iter()
                .rev()
                .map(|l| {
                    let color = match l.level {
                        LogLevel::Info => p.muted,
                        LogLevel::Warn => p.warn,
                        LogLevel::Fail => p.fail,
                    };
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(16.))
                        .px(px(20.))
                        .py(px(6.))
                        .border_b_1()
                        .border_color(p.border)
                        .child(
                            div()
                                .w(px(52.))
                                .flex_none()
                                .mono_micro()
                                .text_color(p.muted)
                                .child(clock(l.at_ms)),
                        )
                        .child(ui::marker(color, 6.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .mono_small()
                                .text_color(if l.level == LogLevel::Info {
                                    p.text
                                } else {
                                    color
                                })
                                .truncate()
                                .child(l.text.clone()),
                        )
                })
                .collect()
        } else {
            Vec::new()
        };
        let cancellable = matches!(run.status, RunStatus::Queued | RunStatus::Running);
        let saving = run.status == RunStatus::Saving;
        let finished_report =
            matches!(run.status, RunStatus::Done).then(|| data::origin_of(&run.url));

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
                        if saving {
                            d
                        } else if cancellable {
                            d.child(
                                ui::button_secondary("cancel", "Cancel", 36., &p).on_click(
                                    cx.listener(move |this, _, _, cx| this.cancel(idx, cx)),
                                ),
                            )
                        } else if let Some(origin) = finished_report.clone() {
                            d.child(ui::button_primary("open", "Open report", 36., &p).on_click(
                                cx.listener(move |this, _, _, cx| {
                                    if this.sites.iter().any(|s| s.origin == origin) {
                                        this.open_site(&origin, cx);
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
            .children(activity_line(run, &p))
            .children(status_line(run, &p))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_end()
                    .gap(px(20.))
                    .px(px(20.))
                    .border_b_1()
                    .border_color(p.border)
                    .child(
                        list_tab("tab-pages", "Pages as they finish".into(), !show_log, &p)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run_log = false;
                                cx.notify();
                            })),
                    )
                    .child(
                        list_tab("tab-log", format!("Activity log {}", log_len), show_log, &p)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.run_log = true;
                                cx.notify();
                            })),
                    )
                    .child(ui::spacer())
                    .when(failed > 0, |d| {
                        d.child(div().pb(px(14.)).child(ui::micro(
                            format!(
                                "{failed} FAILED REQUEST{}",
                                if failed == 1 { "" } else { "S" }
                            ),
                            p.warn,
                        )))
                    })
                    .child(div().pb(px(14.)).map(|d| {
                        if show_log {
                            d.child(ui::link("copy-log", "Copy log", &p).on_click(cx.listener(
                                move |this, _, _, cx| {
                                    let text = this.runs[idx].log_text();
                                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                                },
                            )))
                        } else {
                            d.child(ui::micro("NEWEST FIRST", p.muted))
                        }
                    })),
            )
            .child(
                div()
                    .id("live-log")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .map(|d| {
                        if show_log {
                            d.children(log_rows)
                        } else {
                            d.children(checking).children(finished_rows)
                        }
                    }),
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
            let quiet = r.quiet_ms().filter(|&q| q >= QUIET_WARN_MS);
            let (word, color) = match r.status {
                RunStatus::Done => ("DONE", p.pass),
                // A long silence turns the marker amber, as on the single-audit screen.
                RunStatus::Running if quiet.is_some() => ("NO UPDATE", p.warn),
                RunStatus::Running if r.pages_total.is_none() && r.rendered.is_some() => {
                    ("RENDERING", p.info)
                }
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
            let origin = data::origin_of(&r.url);
            let host_label = data::host_of(&r.url).to_string();
            let action = match r.status {
                RunStatus::Done => Some(ui::link(("open", i), "Open report", &p).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if this.sites.iter().any(|s| s.origin == origin) {
                            this.open_site(&origin, cx);
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
