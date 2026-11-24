//! Colour tokens, type scale and fonts from the Figma file.

use std::borrow::Cow;

use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

pub const SANS: &str = "Schibsted Grotesk";
pub const MONO: &str = "JetBrains Mono";
pub const BRAND: &str = "Saira Stencil One";

/// The Figma variables, one set per mode.
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg_app: Hsla,
    pub bg_sunken: Hsla,
    pub bg_surface: Hsla,
    pub bg_raised: Hsla,
    pub bg_input: Hsla,
    pub border: Hsla,
    pub border_strong: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub accent_bg: Hsla,
    pub accent_on: Hsla,
    pub accent_text: Hsla,
    pub pass: Hsla,
    pub warn: Hsla,
    pub fail: Hsla,
    pub info: Hsla,
    pub skip: Hsla,
}

impl Palette {
    pub fn dark() -> Self {
        Self {
            bg_app: rgb(0x0b0b0c).into(),
            bg_sunken: rgb(0x0f0f11).into(),
            bg_surface: rgb(0x141416).into(),
            bg_raised: rgb(0x1e1e22).into(),
            bg_input: rgb(0x0b0b0c).into(),
            border: rgb(0x2a2a2f).into(),
            border_strong: rgb(0x3a3a41).into(),
            text: rgb(0xf2f2ee).into(),
            muted: rgb(0x9b9ba3).into(),
            accent_bg: rgb(0xc8f031).into(),
            accent_on: rgb(0x0b0b0c).into(),
            accent_text: rgb(0xc8f031).into(),
            pass: rgb(0x3ddc84).into(),
            warn: rgb(0xffb020).into(),
            fail: rgb(0xff6b63).into(),
            info: rgb(0x6fa8ff).into(),
            skip: rgb(0x8a8a93).into(),
        }
    }

    pub fn light() -> Self {
        Self {
            bg_app: rgb(0xf6f5f1).into(),
            bg_sunken: rgb(0xefeee8).into(),
            bg_surface: rgb(0xffffff).into(),
            bg_raised: rgb(0xe8e6de).into(),
            bg_input: rgb(0xffffff).into(),
            border: rgb(0xdcdad2).into(),
            border_strong: rgb(0xb9b7ae).into(),
            text: rgb(0x0b0b0c).into(),
            muted: rgb(0x5f5f66).into(),
            accent_bg: rgb(0xc8f031).into(),
            accent_on: rgb(0x0b0b0c).into(),
            accent_text: rgb(0x3f6b00).into(),
            pass: rgb(0x16803f).into(),
            warn: rgb(0xa84b00).into(),
            fail: rgb(0xc2241c).into(),
            info: rgb(0x1f55c4).into(),
            skip: rgb(0x6b6b74).into(),
        }
    }

    /// Score band colour: good 90+, fair 50 to 89, poor below 50.
    pub fn band(&self, score: u8) -> Hsla {
        match score {
            90.. => self.pass,
            50..=89 => self.warn,
            _ => self.fail,
        }
    }
}

/// Current mode. Follows the system until the theme switch is clicked.
pub struct Appearance {
    pub dark: bool,
    pub pal: Palette,
}

impl Global for Appearance {}

pub trait ActivePalette {
    fn pal(&self) -> Palette;
}

impl ActivePalette for App {
    fn pal(&self) -> Palette {
        self.global::<Appearance>().pal
    }
}

pub fn init(window_dark: bool, cx: &mut App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/SchibstedGrotesk-Regular.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/SchibstedGrotesk-Medium.ttf"
        )),
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/SchibstedGrotesk-SemiBold.ttf"
        )),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Medium.ttf")),
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/SairaStencilOne-Regular.ttf"
        )),
    ];
    cx.text_system()
        .add_fonts(fonts)
        .expect("bundled fonts load");
    set_dark(window_dark, cx);
}

pub fn set_dark(dark: bool, cx: &mut App) {
    let pal = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    cx.set_global(Appearance { dark, pal });
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        None,
        cx,
    );
    // Map the tokens onto gpui-component so its inputs, menus and charts match the design.
    Theme::update(cx, |t| {
        t.font_family = SANS.into();
        t.mono_font_family = MONO.into();
        t.font_size = px(14.);
        t.mono_font_size = px(13.);
        t.radius = px(0.);
        t.radius_lg = px(0.);
        t.shadow = false;
        t.focus_ring = false;
        t.background = pal.bg_app;
        t.foreground = pal.text;
        t.border = pal.border;
        t.input = pal.border;
        t.ring = pal.accent_text;
        t.caret = pal.text;
        t.muted = pal.bg_raised;
        t.muted_foreground = pal.muted;
        t.popover = pal.bg_surface;
        t.popover_foreground = pal.text;
        t.accent = pal.bg_raised;
        t.accent_foreground = pal.text;
        t.primary = pal.accent_bg;
        t.primary_hover = pal.accent_bg.opacity(0.9);
        t.primary_active = pal.accent_bg;
        t.primary_foreground = pal.accent_on;
        t.list_hover = pal.bg_raised;
        t.list_active = pal.bg_raised;
        t.list_active_border = pal.accent_bg;
        t.chart_grid = pal.border;
        t.scrollbar = pal.bg_app.opacity(0.);
    });
}

/// Text styles from the Figma type scale. Letter spacing (Mono label, Score) is left out:
/// GPUI has no tracking setting.
pub trait TextStyles: Styled + Sized {
    fn mono_micro(self) -> Self {
        self.font_family(MONO)
            .text_size(px(10.))
            .line_height(px(14.))
    }
    fn mono_small(self) -> Self {
        self.font_family(MONO)
            .text_size(px(11.))
            .line_height(px(16.))
    }
    fn mono_label(self) -> Self {
        self.font_family(MONO)
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(11.))
            .line_height(px(14.))
    }
    fn mono(self) -> Self {
        self.font_family(MONO)
            .text_size(px(13.))
            .line_height(px(20.))
    }
    fn score_text(self) -> Self {
        self.font_family(MONO)
            .font_weight(FontWeight::MEDIUM)
            .text_size(px(28.))
            .line_height(px(32.))
    }
    fn small(self) -> Self {
        self.font_family(SANS)
            .text_size(px(12.))
            .line_height(px(18.))
    }
    fn small_medium(self) -> Self {
        self.small().font_weight(FontWeight::MEDIUM)
    }
    fn body(self) -> Self {
        self.font_family(SANS)
            .text_size(px(14.))
            .line_height(px(20.))
    }
    fn body_medium(self) -> Self {
        self.body().font_weight(FontWeight::MEDIUM)
    }
    fn title(self) -> Self {
        self.font_family(SANS)
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(15.))
            .line_height(px(20.))
    }
    fn h2(self) -> Self {
        self.font_family(SANS)
            .font_weight(FontWeight::SEMIBOLD)
            .text_size(px(20.))
            .line_height(px(26.))
    }
}

impl<T: Styled> TextStyles for T {}

#[cfg(test)]
mod tests {
    use super::Palette;

    #[test]
    fn score_bands_follow_the_figma_rule() {
        let p = Palette::dark();
        assert_eq!(p.band(100), p.pass);
        assert_eq!(p.band(90), p.pass);
        assert_eq!(p.band(89), p.warn);
        assert_eq!(p.band(50), p.warn);
        assert_eq!(p.band(49), p.fail);
    }
}
