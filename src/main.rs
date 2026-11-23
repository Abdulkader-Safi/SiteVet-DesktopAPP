mod data;
mod theme;

use gpui_kit::*;
use theme::{ActivePalette, TextStyles};

struct Shell;

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_2()
            .p_5()
            .bg(p.bg_app)
            .text_color(p.text)
            .child(div().font_family(theme::BRAND).text_size(px(15.)).child("SITEVET"))
            .child(div().h2().child("New audit"))
            .child(div().body().text_color(p.muted).child("Audit one site, or several one after another."))
            .child(div().mono_label().text_color(p.muted).child("START URL"))
            .child(div().mono().child("https://dsrpt.com.au"))
            .child(div().score_text().child("91"))
            .child(div().mono_micro().text_color(p.accent_text).child("RUNNING · 42 / 100"))
    }
}

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        let dark = matches!(cx.window_appearance(), WindowAppearance::Dark | WindowAppearance::VibrantDark);
        theme::init(dark, cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(None, size(px(1512.), px(982.)), cx)),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Shell)).expect("failed to open window");
    });
}
