mod app;
mod assets;
mod data;
mod screens;
mod theme;
mod ui;
mod update;

use gpui_kit::*;

fn main() {
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            theme::init(app::is_dark(cx.window_appearance()), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(None, size(px(1512.), px(982.)), cx)),
                window_min_size: Some(size(px(1100.), px(700.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("SiteVet".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| app::AppView::new(window, cx))
            })
            .expect("failed to open window");
            cx.activate(true);
        });
}
