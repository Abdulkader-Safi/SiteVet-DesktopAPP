//! New audit: one site, or several one after another.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppView;
use crate::data;
use crate::editor::Which;
use crate::theme::{ActivePalette, TextStyles};
use crate::ui;

impl AppView {
    pub fn render_new_audit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = cx.pal();
        let several = self.form.several;
        let compare_on = self.form.editor.settings.compare_with_last;

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
        let options = self.render_options(Which::Form, several, compare_desc, window, cx);
        let custom = self.form.editor.differs_from(&self.defaults, cx);

        let footer_note = if several {
            self.batch_estimate(cx)
        } else {
            match (known, compare_on) {
                (Some(n), true) => format!("SAVED AS RUN {} · COMPARED WITH RUN {n}", n + 1),
                (Some(n), false) => format!("SAVED AS RUN {}", n + 1),
                (None, _) => "SAVED AS RUN 1".into(),
            }
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

        // Says where the values come from, and links to where the defaults are kept.
        let source = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .small()
            .text_color(p.muted)
            .child(if custom {
                "Changed for this audit only. The next audit starts from your defaults again."
            } else {
                "Using your defaults."
            })
            .child(
                ui::link("open-settings", "Edit defaults in Settings", &p)
                    .on_click(cx.listener(|this, _, window, cx| this.open_settings(window, cx))),
            );

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
                    .child(ui::segment("several", "Several sites", several, &p).on_click(cx.listener(|this, _, _, cx| {
                        if !this.form.several && this.form.sites.is_empty() {
                            // Start the list with the one-site URL, if any.
                            let url = this.form.start_url.read(cx).value().to_string();
                            this.form.sites.extend(data::parse_urls(&url));
                        }
                        this.form.several = true;
                        cx.notify();
                    }))),
            )
            .child(target)
            .child(options)
            .child(source)
            .when_some(self.form.error.clone(), |d, e| d.child(div().small().text_color(p.fail).child(e)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(4.))
                    .child(
                        ui::button_primary("run", run_label, 40., &p)
                            .on_click(cx.listener(|this, _, window, cx| this.run_from_form(window, cx))),
                    )
                    .when(custom, |d| {
                        d.child(
                            ui::button_secondary("reset", "Reset to defaults", 40., &p)
                                .on_click(cx.listener(|this, _, window, cx| this.reset_form(window, cx))),
                        )
                    })
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
            .editor
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
