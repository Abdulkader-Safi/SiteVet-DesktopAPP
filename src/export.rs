//! Export the open report as HTML, Markdown or JSON, through the system save dialog.

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use sitevet::{AuditReport, Format};

use crate::app::{AppView, OpenSelect};
use crate::data;
use crate::theme::{ActivePalette, TextStyles};
use crate::ui;

/// The three formats in menu order: format, menu label, what it is for, file extension.
pub const FORMATS: [(Format, &str, &str, &str); 3] = [
    (
        Format::Html,
        "HTML report",
        "One file that opens in any browser. Print it to PDF from there.",
        "html",
    ),
    (
        Format::Markdown,
        "Markdown",
        "Paste into Word, Google Docs, Notion or an email.",
        "md",
    ),
    (
        Format::Json,
        "JSON",
        "Every finding and metric, for scripts and other tools.",
        "json",
    ),
];

/// How the last export went, shown in the bottom-right corner until closed.
pub enum ExportStatus {
    Saved { path: PathBuf, label: &'static str },
    Failed(String),
}

/// `sitevet-dsrpt.com.au-2026-11-24.html`
pub fn file_name(report: &AuditReport, ext: &str) -> String {
    let host: String = data::host_of(&report.start_url)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!(
        "sitevet-{host}-{}.{ext}",
        data::time::date(&report.generated_at)
    )
}

/// Downloads, else home, else the current folder.
fn start_dir() -> PathBuf {
    directories::UserDirs::new()
        .map(|d| d.download_dir().unwrap_or(d.home_dir()).to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

impl AppView {
    /// Asks where to save, then renders and writes the report off the UI thread.
    fn export(&mut self, index: usize, cx: &mut Context<Self>) {
        self.open_select = OpenSelect::None;
        let Some(report) = self.site().map(|s| s.latest.clone()) else {
            return;
        };
        let (format, label, _, ext) = FORMATS[index];
        let dialog = cx.prompt_for_new_path(&start_dir(), Some(&file_name(&report, ext)));
        cx.spawn(async move |this, cx| {
            let path = match dialog.await {
                Ok(Ok(Some(path))) => path,
                // Cancelled.
                Ok(Ok(None)) | Err(_) => return,
                Ok(Err(e)) => {
                    let _ = this.update(cx, |app, cx| {
                        app.export_status = Some(ExportStatus::Failed(e.to_string()));
                        cx.notify();
                    });
                    return;
                }
            };
            let write = cx.background_spawn(write_report(report, format, path.clone()));
            let status = match write.await {
                Ok(()) => ExportStatus::Saved { path, label },
                Err(e) => ExportStatus::Failed(format!("Could not write {}: {e}", path.display())),
            };
            let _ = this.update(cx, |app, cx| {
                app.export_status = Some(status);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    /// The export icon in the top bar and, when open, its menu of formats.
    pub fn export_button(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.pal();
        let open = self.open_select == OpenSelect::Export;
        let menu = open.then(|| {
            let items: Vec<AnyElement> = FORMATS
                .iter()
                .enumerate()
                .map(|(i, (_, label, desc, ext))| {
                    div()
                        .id(("export", i))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .w(px(300.))
                        .px(px(12.))
                        .py(px(10.))
                        .cursor_pointer()
                        .when(i + 1 < FORMATS.len(), |d| {
                            d.border_b_1().border_color(p.border)
                        })
                        .hover(|s| s.bg(p.bg_raised))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(div().body_medium().text_color(p.text).child(*label))
                                .child(ui::spacer())
                                .child(ui::micro(format!(".{ext}").to_uppercase(), p.muted)),
                        )
                        .child(div().small().text_color(p.muted).child(*desc))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.export(i, cx);
                            }),
                        )
                        .into_any_element()
                })
                .collect();
            deferred(
                anchored()
                    .anchor(Anchor::TopRight)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .id("export-menu")
                            .mt(px(30.))
                            .bg(p.bg_surface)
                            .border_1()
                            .border_color(p.border_strong)
                            .occlude()
                            .child(
                                div()
                                    .px(px(12.))
                                    .pt(px(10.))
                                    .pb(px(6.))
                                    .border_b_1()
                                    .border_color(p.border)
                                    .child(ui::label("EXPORT THIS RUN", &p)),
                            )
                            .children(items),
                    ),
            )
            .with_priority(1)
        });
        // The menu is laid out after the icon, so its right edge lines up with the icon's.
        div()
            .flex()
            .flex_none()
            .child(
                div()
                    .id("export-button")
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(28.))
                    .px(px(2.))
                    .cursor_pointer()
                    .when(open, |d| d.bg(p.bg_raised))
                    .hover(|s| s.bg(p.bg_raised))
                    .child(ui::icon(
                        "icons/file-export.svg",
                        25.,
                        22.,
                        if open { p.text } else { p.muted },
                    ))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            cx.stop_propagation();
                            this.open_select = if this.open_select == OpenSelect::Export {
                                OpenSelect::None
                            } else {
                                OpenSelect::Export
                            };
                            cx.notify();
                        }),
                    ),
            )
            .children(menu)
    }

    /// Bottom-right card after an export: where the file went, with Open and Show buttons.
    pub fn export_card(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let status = self.export_status.as_ref()?;
        let p = cx.pal();
        let close = div()
            .id("export-close")
            .px(px(4.))
            .small()
            .text_color(p.muted)
            .cursor_pointer()
            .hover(|s| s.text_color(p.text))
            .child("×")
            .on_click(cx.listener(|this, _, _, cx| {
                this.export_status = None;
                cx.notify();
            }));
        let card = ui::toast_card(&p);
        Some(match status {
            ExportStatus::Saved { path, label } => {
                let (open, show) = (path.clone(), path.clone());
                let reveal = if cfg!(target_os = "macos") {
                    "Show in Finder"
                } else {
                    "Show in folder"
                };
                card.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::marker(p.pass, 6.))
                        .child(ui::label("EXPORTED", &p))
                        .child(ui::spacer())
                        .child(close),
                )
                .child(
                    div()
                        .title()
                        .text_color(p.text)
                        .child(format!("{label} saved")),
                )
                .child(
                    div()
                        .mono_small()
                        .text_color(p.muted)
                        .child(path.display().to_string()),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(8.))
                        .pt(px(4.))
                        .child(
                            ui::button_primary("export-open", "Open", 32., &p).on_click(
                                cx.listener(move |_, _, _, cx| cx.open_with_system(&open)),
                            ),
                        )
                        .child(
                            ui::button_secondary("export-show", reveal, 32., &p)
                                .on_click(cx.listener(move |_, _, _, cx| cx.reveal_path(&show))),
                        ),
                )
                .into_any_element()
            }
            ExportStatus::Failed(e) => card
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(ui::marker(p.fail, 6.))
                        .child(ui::label("EXPORT FAILED", &p))
                        .child(ui::spacer())
                        .child(close),
                )
                .child(div().small().text_color(p.fail).child(e.clone()))
                .into_any_element(),
        })
    }
}

async fn write_report(
    report: Arc<AuditReport>,
    format: Format,
    path: PathBuf,
) -> std::io::Result<()> {
    std::fs::write(path, sitevet::render(&report, format))
}

#[cfg(test)]
mod tests {
    use super::{FORMATS, file_name, write_report};

    #[test]
    fn names_the_file_after_the_site_and_day() {
        let json = r#"{"schema_version":1,"sitevet_version":"0.7.0","start_url":"https://dsrpt.com.au/x","final_url":"https://dsrpt.com.au",
            "generated_at":"2026-11-24T08:51:00Z","duration_ms":1,"options":{},"score":90,"categories":[],"pages":[],"blocked":[]}"#;
        let report: sitevet::AuditReport = serde_json::from_str(json).unwrap();
        let name = file_name(&report, "md");
        assert!(name.starts_with("sitevet-dsrpt.com.au-2026-11-2"), "{name}");
        assert!(name.ends_with(".md"));
    }

    #[test]
    fn writes_every_format() {
        let json = r#"{"schema_version":1,"sitevet_version":"0.7.0","start_url":"https://a.com","final_url":"https://a.com",
            "generated_at":"2026-11-24T08:51:00Z","duration_ms":1,"options":{},"score":90,"categories":[],"pages":[],"blocked":[]}"#;
        let report =
            std::sync::Arc::new(serde_json::from_str::<sitevet::AuditReport>(json).unwrap());
        let dir = tempfile::tempdir().unwrap();
        for (format, _, _, ext) in FORMATS {
            let path = dir.path().join(file_name(&report, ext));
            futures::executor::block_on(write_report(report.clone(), format, path.clone()))
                .unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            match ext {
                "html" => assert!(text.starts_with("<!doctype html>")),
                "md" => assert!(text.contains("https://a.com")),
                _ => assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&text).unwrap()["score"],
                    90
                ),
            }
        }
    }
}
