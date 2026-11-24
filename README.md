# SiteVet desktop

A desktop app for [sitevet](https://crates.io/crates/sitevet), the Rust website audit engine. It runs audits on your machine, shows the results page by page, and keeps every run in the same history database the `sitevet` CLI uses, so runs from either one show up in both.

Built with GPUI through [GPUI Kit](https://gpui-kit.com) (`gpui-kit` 0.11). The score chart is GPUI Kit's own `LineChart`.

## What works

- New audit for one site, or a list of sites run two (or any number) at a time.
- Live progress: pages as they finish with their scores, running check counts, elapsed time, and a cancel button. A batch gets its own table with a row per site.
- The report for each site, in five tabs. Pages lists every page worst first, with its findings and measurements. Overview shows each category's score. Site-wide holds the findings that compare pages (duplicates, orphans). Changes lists what was fixed and what is new since the last run.
- History: score by run on a chart, a side-by-side compare of any two runs per category, and the rules fixed or broken between them.
- Dark and light themes. The app follows the system until you click the switch in the top bar.
- Save as defaults keeps the form's settings in `desktop-defaults.toml`, next to the history database.
- Keyboard: `P` jumps to the site search. On the Pages tab, `C` searches categories and `/` searches pages.

Export is planned. Its icon sits in the top bar but does nothing yet.

## Run it

You need Rust 1.100 or newer. `rust-toolchain.toml` pins it, so rustup installs it on the first build. GPUI's macOS crate uses `Option::map_or_default`, which older compilers reject.

```sh
cargo run --release
```

Rendered checks (axe-core and Core Web Vitals) need Chrome installed. Turn them on under Checks in the new audit form.

## Where your data lives

History is `history.db` in the OS data folder: `~/Library/Application Support/sitevet` on macOS, `~/.local/share/sitevet` on Linux, `%APPDATA%\sitevet` on Windows. Set `SITEVET_HOME` to use another folder, which is handy for trying the app without touching your real runs.

## Known gaps

The "Check external links" switch removes the `links/broken-external` rule from the run, so those results no longer appear. The SDK's link checker still sends its HEAD requests to external links either way, because `sitevet::Options` has no setting to skip them.

## Layout

```text
src/
  main.rs        window setup
  app.rs         state, the audit engine, site list and top bar
  data.rs        history, settings and counts over the SDK (no UI code; has tests)
  theme.rs       colour tokens, type scale and fonts from the Figma file
  ui.rs          shared components: score chip, status badge, buttons, fields
  screens/       report, history, new audit, progress
assets/
  fonts/         Schibsted Grotesk, JetBrains Mono, Saira Stencil One (OFL)
  icons/         the design's SVG icons
```

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

## Licence

The fonts in `assets/fonts` are under the SIL Open Font License; their licence files sit beside them.
