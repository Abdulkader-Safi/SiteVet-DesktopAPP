//! Components from the Figma file: score chip, status badge, chips, buttons, toggle, KPI cell,
//! tabs and fields. Each takes the active palette.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sitevet::Status;

use crate::theme::{Palette, TextStyles};

pub fn spacer() -> Div {
    div().flex_1().min_w_0()
}

pub fn icon(path: &'static str, w: f32, h: f32, color: Hsla) -> Svg {
    svg()
        .path(path)
        .w(px(w))
        .h(px(h))
        .flex_none()
        .text_color(color)
}

pub fn marker(color: Hsla, size: f32) -> Div {
    div().flex_none().size(px(size)).bg(color)
}

/// "OVERALL", "CATEGORIES": Mono label in muted.
pub fn label(text: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .flex_none()
        .mono_label()
        .text_color(p.muted)
        .whitespace_nowrap()
        .child(text.into())
}

/// "9 ISSUES · 2 FAIL": Mono micro in muted.
pub fn micro(text: impl Into<SharedString>, color: Hsla) -> Div {
    div()
        .flex_none()
        .mono_micro()
        .text_color(color)
        .whitespace_nowrap()
        .child(text.into())
}

/// A score outlined in its band colour: good 90+, fair 50 to 89, poor below 50.
pub fn score_chip(score: u8, p: &Palette) -> Div {
    let c = p.band(score);
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(40.))
        .h(px(24.))
        .border_1()
        .border_color(c)
        .mono_small()
        .text_color(c)
        .child(score.to_string())
}

pub fn status_color(status: Status, p: &Palette) -> Hsla {
    match status {
        Status::Pass => p.pass,
        Status::Fail => p.fail,
        Status::Warn => p.warn,
        _ => p.info,
    }
}

fn status_word(status: Status) -> &'static str {
    match status {
        Status::Pass => "PASS",
        Status::Fail => "FAIL",
        Status::Warn => "WARN",
        _ => "INFO",
    }
}

/// Outline badge: colour lives only in the 1px outline, the 6px square and the label.
pub fn status_badge(status: Status, p: &Palette) -> Div {
    badge(status_word(status), status_color(status, p))
}

pub fn badge(word: &'static str, c: Hsla) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .h(px(18.))
        .px(px(6.))
        .border_1()
        .border_color(c)
        .child(marker(c, 6.))
        .child(div().mono_micro().text_color(c).child(word))
}

/// Filter chip: "Lowest score", "All runs".
pub fn chip(
    id: impl Into<ElementId>,
    text: &'static str,
    active: bool,
    p: &Palette,
) -> Stateful<Div> {
    let base = div()
        .id(id)
        .flex_none()
        .px(px(8.))
        .py(px(3.))
        .small()
        .cursor_pointer()
        .child(text);
    if active {
        base.bg(p.accent_bg)
            .text_color(p.accent_on)
            .border_1()
            .border_color(p.accent_bg)
    } else {
        base.border_1()
            .border_color(p.border)
            .text_color(p.muted)
            .hover(|s| s.text_color(p.text))
    }
}

/// Segmented control segment: "One site" / "Several sites".
pub fn segment(
    id: impl Into<ElementId>,
    text: &'static str,
    active: bool,
    p: &Palette,
) -> Stateful<Div> {
    let base = div()
        .id(id)
        .flex_none()
        .px(px(14.))
        .py(px(6.))
        .small()
        .cursor_pointer()
        .child(text);
    if active {
        base.bg(p.accent_bg)
            .text_color(p.accent_on)
            .border_1()
            .border_color(p.accent_bg)
    } else {
        base.border_1()
            .border_color(p.border)
            .text_color(p.muted)
            .hover(|s| s.text_color(p.text))
    }
}

pub fn button_primary(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    h: f32,
    p: &Palette,
) -> Stateful<Div> {
    let big = h >= 40.;
    let base = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(h))
        .px(px(if big { 20. } else { 12. }))
        .bg(p.accent_bg)
        .text_color(p.accent_on)
        .cursor_pointer()
        .hover(|s| s.opacity(0.9))
        .child(text.into());
    if big {
        base.body_medium()
    } else {
        base.small_medium()
    }
}

pub fn button_secondary(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    h: f32,
    p: &Palette,
) -> Stateful<Div> {
    let base = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(h))
        .px(px(if h >= 36. { 16. } else { 12. }))
        .border_1()
        .border_color(p.border_strong)
        .text_color(p.text)
        .cursor_pointer()
        .hover(|s| s.bg(p.bg_raised))
        .child(text.into());
    if h >= 36. {
        base.body()
    } else {
        base.small_medium()
    }
}

/// Text link in accent: "Show all options", "Open report".
pub fn link(id: impl Into<ElementId>, text: impl Into<SharedString>, p: &Palette) -> Stateful<Div> {
    div()
        .id(id)
        .flex_none()
        .small_medium()
        .text_color(p.accent_text)
        .cursor_pointer()
        .whitespace_nowrap()
        .hover(|s| s.underline())
        .child(text.into())
}

/// Square 34x18 switch.
pub fn toggle(id: impl Into<ElementId>, on: bool, p: &Palette) -> Stateful<Div> {
    let base = div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .w(px(34.))
        .h(px(18.))
        .px(px(3.))
        .cursor_pointer();
    if on {
        base.bg(p.accent_bg)
            .justify_end()
            .child(marker(p.accent_on, 12.))
    } else {
        base.bg(p.bg_raised)
            .border_1()
            .border_color(p.border_strong)
            .child(marker(p.muted, 12.))
    }
}

/// One cell of a KPI strip: label (with an optional marker) over a value.
pub fn kpi(
    name: impl Into<SharedString>,
    dot: Option<Hsla>,
    value: impl IntoElement,
    p: &Palette,
) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .h_full()
        .justify_center()
        .gap(px(4.))
        .px(px(20.))
        .py(px(14.))
        .border_r_1()
        .border_color(p.border)
        .overflow_hidden()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .children(dot.map(|c| marker(c, 6.)))
                .child(label(name, p)),
        )
        .child(value)
}

pub fn kpi_value(text: impl Into<SharedString>, p: &Palette) -> Div {
    div()
        .title()
        .text_color(p.text)
        .whitespace_nowrap()
        .child(text.into())
}

/// A value with a unit beside it: "91 RUN 6", "41 ISSUES".
pub fn kpi_value_unit(
    value: impl Into<SharedString>,
    unit: impl Into<SharedString>,
    p: &Palette,
) -> Div {
    div()
        .flex()
        .items_baseline()
        .gap(px(8.))
        .child(kpi_value(value, p))
        .child(micro(unit.into().to_uppercase(), p.muted))
}

/// Top bar tab: label over a 2px underline that is accent when active.
pub fn top_tab(
    id: impl Into<ElementId>,
    text: &'static str,
    active: bool,
    p: &Palette,
) -> Stateful<Div> {
    let label = div().whitespace_nowrap().child(text);
    let label = if active {
        label.small_medium().text_color(p.text)
    } else {
        label.small().text_color(p.muted)
    };
    div()
        .id(id)
        .flex()
        .flex_col()
        .justify_end()
        .gap(px(14.))
        .h(px(52.))
        .pt(px(16.))
        .cursor_pointer()
        .child(label)
        .child(div().h(px(2.)).w_full().when(active, |d| d.bg(p.accent_bg)))
}

/// Detail tab: "Findings 9".
pub fn detail_tab(
    id: impl Into<ElementId>,
    text: String,
    active: bool,
    p: &Palette,
) -> Stateful<Div> {
    let label = div().whitespace_nowrap().child(text);
    let label = if active {
        label.small_medium().text_color(p.text)
    } else {
        label.small().text_color(p.muted)
    };
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(8.))
        .cursor_pointer()
        .child(label)
        .child(div().h(px(2.)).w_full().when(active, |d| d.bg(p.accent_bg)))
}

/// 2px accent edge on the left of a selected row.
pub fn edge(p: &Palette) -> Div {
    div()
        .absolute()
        .left_0()
        .top_0()
        .bottom_0()
        .w(px(2.))
        .bg(p.accent_bg)
}

/// Search field: icon, input and the key that focuses it. Border turns accent when focused.
pub fn search_box(input: &Entity<InputState>, key: &'static str, window: &Window, cx: &App) -> Div {
    let p = crate::theme::ActivePalette::pal(cx);
    let focused = input.read(cx).focus_handle(cx).is_focused(window);
    div()
        .flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap(px(8.))
        .h(px(32.))
        .px(px(8.))
        .bg(p.bg_input)
        .border_1()
        .border_color(if focused { p.accent_text } else { p.border })
        .child(icon("icons/search.svg", 14., 14., p.muted))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .body()
                .child(Input::new(input).appearance(false).px_0()),
        )
        .child(
            div()
                .flex_none()
                .px(px(4.))
                .border_1()
                .border_color(p.border)
                .mono_micro()
                .text_color(p.muted)
                .child(key.to_uppercase()),
        )
}

/// A 40px form field in mono. The border is accent while focused.
pub fn text_field(input: &Entity<InputState>, window: &Window, cx: &App) -> Div {
    let p = crate::theme::ActivePalette::pal(cx);
    let focused = input.read(cx).focus_handle(cx).is_focused(window);
    div()
        .flex()
        .items_center()
        .w_full()
        .h(px(40.))
        .px(px(12.))
        .bg(p.bg_input)
        .border_1()
        .border_color(if focused { p.accent_bg } else { p.border })
        .mono()
        .text_color(p.text)
        .child(Input::new(input).appearance(false).px_0())
}

/// "MAX PAGES" over a field.
pub fn field(name: &'static str, input: &Entity<InputState>, window: &Window, cx: &App) -> Div {
    let p = crate::theme::ActivePalette::pal(cx);
    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w_0()
        .gap(px(6.))
        .child(label(name, &p))
        .child(text_field(input, window, cx))
}

/// Closed select: "Overall  v".
pub fn select_box(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    p: &Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.))
        .px(px(10.))
        .py(px(5.))
        .bg(p.bg_input)
        .border_1()
        .border_color(p.border)
        .cursor_pointer()
        .child(
            div()
                .mono_small()
                .text_color(p.text)
                .whitespace_nowrap()
                .child(text.into()),
        )
        .child(micro("V", p.muted))
}

/// The open list under a select, drawn above everything else.
pub fn select_menu(items: Vec<AnyElement>, p: &Palette) -> impl IntoElement {
    deferred(
        anchored().snap_to_window_with_margin(px(8.)).child(
            div()
                .id("select-menu")
                .mt(px(4.))
                .min_w(px(160.))
                .max_h(px(320.))
                .overflow_y_scroll()
                .bg(p.bg_surface)
                .border_1()
                .border_color(p.border_strong)
                .occlude()
                .children(items),
        ),
    )
    .with_priority(1)
}

pub fn select_item(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    active: bool,
    p: &Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(10.))
        .py(px(6.))
        .mono_small()
        .whitespace_nowrap()
        .cursor_pointer()
        .text_color(if active { p.text } else { p.muted })
        .when(active, |d| d.bg(p.bg_raised))
        .hover(|s| s.bg(p.bg_raised).text_color(p.text))
        .child(text.into())
}

/// Thin progress track with the done part in accent.
pub fn progress_bar(fraction: f32, w: Option<f32>, p: &Palette) -> Div {
    let f = fraction.clamp(0., 1.);
    let track = div().flex().h(px(4.)).bg(p.border).overflow_hidden();
    let track = match w {
        Some(w) => track.w(px(w)).flex_none(),
        None => track.w_full(),
    };
    track.child(div().h_full().w(relative(f)).bg(p.accent_text))
}

/// A card for the bottom-right corner: an update offer, an export result.
pub fn toast_card(p: &Palette) -> Div {
    div()
        .w(px(320.))
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(16.))
        .bg(p.bg_surface)
        .border_1()
        .border_color(p.border_strong)
        .occlude()
}

/// Outline button in the danger colour, for deleting.
pub fn button_danger(
    id: impl Into<ElementId>,
    text: impl Into<SharedString>,
    h: f32,
    p: &Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(h))
        .px(px(16.))
        .border_1()
        .border_color(p.fail)
        .text_color(p.fail)
        .body_medium()
        .cursor_pointer()
        .hover(|s| s.bg(p.bg_raised))
        .child(text.into())
}

/// A trash button that shows on hover of its row's `group`, and always on a selected row.
pub fn trash_button(
    id: impl Into<ElementId>,
    group: SharedString,
    always: bool,
    p: &Palette,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(28.))
        .cursor_pointer()
        .when(!always, |d| {
            d.opacity(0.).group_hover(group, |s| s.opacity(1.))
        })
        .hover(|s| s.bg(p.bg_raised))
        .child(icon("icons/trash.svg", 14., 14., p.muted))
}
