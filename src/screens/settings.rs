//! Settings: about the app, and the defaults every new audit starts from.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::AppView;
use crate::editor::Which;
use crate::theme::{self, ActivePalette, Palette, TextStyles};
use crate::ui;

const GITHUB: &str = "https://github.com/Abdulkader-Safi/SiteVet-DesktopAPP";
const PORTFOLIO: &str = "https://abdulkadersafi.com";
const ENGINE: &str = "https://crates.io/crates/sitevet";

/// "GITHUB   github.com/..." with the address as a link that opens the browser.
fn link_row(
    id: &'static str,
    name: &'static str,
    url: &'static str,
    last: bool,
    p: &Palette,
    cx: &mut Context<AppView>,
) -> Div {
    let shown = url.trim_start_matches("https://");
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .px(px(16.))
        .py(px(12.))
        .when(!last, |d| d.border_b_1().border_color(p.border))
        .child(div().w(px(120.)).flex_none().child(ui::label(name, p)))
        .child(
            div()
                .id(id)
                .mono_small()
                .text_color(p.accent_text)
                .cursor_pointer()
                .hover(|s| s.underline())
                .child(shown)
                .on_click(cx.listener(move |_, _, _, cx| cx.open_url(url))),
        )
}

impl AppView {
    pub fn render_settings(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = cx.pal();
        let options = self.render_options(
            Which::Defaults,
            true,
            "Each new audit is compared with the site's last run, when it has one.".into(),
            window,
            cx,
        );
        let unsaved = self.defaults_editor.differs_from(&self.defaults, cx);

        let about = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .w_full()
            .child(ui::label("ABOUT", &p))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .bg(p.bg_surface)
                    .border_1()
                    .border_color(p.border)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .px(px(16.))
                            .py(px(16.))
                            .border_b_1()
                            .border_color(p.border)
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .font_family(theme::BRAND)
                                            .text_size(px(20.))
                                            .line_height(px(26.))
                                            .text_color(p.text)
                                            .child("SITEVET"),
                                    )
                                    .child(ui::micro(format!("VERSION {}", env!("CARGO_PKG_VERSION")), p.muted)),
                            )
                            .child(div().body().text_color(p.text).child(
                                "A desktop app for the sitevet website audit engine. It crawls a site, runs SEO, \
                                 accessibility, security and performance checks on every page, and keeps each run \
                                 so you can see the score move over time.",
                            ))
                            .child(div().small().text_color(p.muted).child(
                                "Runs are stored in the same history as the sitevet command-line tool. MIT licensed.",
                            )),
                    )
                    .child(link_row("gh", "GITHUB", GITHUB, false, &p, cx))
                    .child(link_row("engine", "ENGINE", ENGINE, false, &p, cx))
                    .child(link_row("portfolio", "MADE BY", PORTFOLIO, true, &p, cx)),
            );

        let note = match (&self.settings_error, &self.settings_note, unsaved) {
            (Some(e), _, _) => ui::micro(e.to_uppercase(), p.fail),
            (None, Some(n), _) => ui::micro(n.clone(), p.muted),
            (None, None, true) => ui::micro("NOT SAVED YET", p.warn),
            (None, None, false) => ui::micro("", p.muted),
        };

        let defaults = div()
            .flex()
            .flex_col()
            .gap(px(28.))
            .w_full()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ui::label("AUDIT DEFAULTS", &p))
                    .child(div().small().text_color(p.muted).child(
                        "Every new audit starts from these values. Change a value on the new audit form to use \
                         it for that audit only.",
                    )),
            )
            .child(options)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(4.))
                    .child(
                        ui::button_primary("save-defaults", "Save defaults", 40., &p)
                            .on_click(cx.listener(|this, _, window, cx| this.save_settings(window, cx))),
                    )
                    .child(
                        ui::button_secondary("sdk-defaults", "Use sitevet's defaults", 40., &p)
                            .on_click(cx.listener(|this, _, window, cx| this.sdk_defaults(window, cx))),
                    )
                    .child(ui::spacer())
                    .child(note),
            );

        div()
            .id("settings-screen")
            .flex()
            .flex_col()
            .items_center()
            .size_full()
            .py(px(40.))
            .bg(p.bg_app)
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(36.))
                    .w(px(720.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(div().h2().text_color(p.text).child("Settings"))
                            .child(div().body().text_color(p.muted).child(
                                "About SiteVet, and the values every new audit starts from.",
                            )),
                    )
                    .child(about)
                    .child(defaults),
            )
    }
}
