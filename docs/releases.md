# Releases

## v0.5.0 (2026-11-24)

- Windows: the SiteVet icon now shows on the window, the taskbar, the Start menu and the desktop shortcut. Before, Windows showed its generic icon, because the icon was never built into the .exe (issue #8).

## v0.4.0 (2026-11-24)

See what an audit is doing while it runs, and delete runs you no longer need.

- The running screen names the stage it is in: crawling, rendering pages in Chrome with a count, or checking. Before, an audit with rendered checks on sat on "Crawling pages" for minutes while Chrome worked.
- A line under the progress bar shows how long ago the audit last reported. After 30 quiet seconds it turns amber and says what is in progress.
- An activity log next to the page list records every step with its time. Copy it for a bug report.
- Delete a whole site from the sidebar, or one run from the History tab. A confirm dialog comes first, since the sitevet CLI reads the same history.

## v0.3.0 (2026-11-24)

Settings and export, with fixes from a code review. v0.2.0 was never published; its changes ship here.

- Settings, behind the gear in the top bar, shows the app version and links to GitHub, the sitevet crate and abdulkadersafi.com. It also holds your audit defaults, every option the new audit form has. New audits start from them, and a value changed on the new audit form applies to that audit only.
- Export saves the open run as an HTML report, Markdown or JSON. Choose where in the save dialog, then open the file or show it in Finder from the card that appears.
- When a newer release is out, the app offers it in a card in the bottom-right corner.
- Fixed: cancelling a site in a batch no longer stalls the sites queued after it, and a second batch no longer strands the first one's queue.
- Fixed: runs on http and https versions of a site, or typed with capitals, are matched to the right site, so run numbers and "Open report" are correct.

## v0.1.0 (2026-11-24)

First release. Audit one site or a list of sites, watch pages finish as they are checked, and read each report page by page. History charts the score across runs and compares any two of them. Dark and light themes. Runs are stored in the same history database as the `sitevet` CLI.
