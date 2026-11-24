//! The design's own icons, with the kit's icon set behind them for its components.

use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString};

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let bytes: &'static [u8] = match path {
            "icons/search.svg" => include_bytes!("../assets/icons/search.svg"),
            "icons/sun.svg" => include_bytes!("../assets/icons/sun.svg"),
            "icons/moon.svg" => include_bytes!("../assets/icons/moon.svg"),
            "icons/layout-sidebar.svg" => include_bytes!("../assets/icons/layout-sidebar.svg"),
            "icons/file-export.svg" => include_bytes!("../assets/icons/file-export.svg"),
            "icons/settings.svg" => include_bytes!("../assets/icons/settings.svg"),
            _ => return gpui_kit::assets::Assets.load(path),
        };
        Ok(Some(Cow::Borrowed(bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
