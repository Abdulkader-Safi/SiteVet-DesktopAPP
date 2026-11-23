//! Everything the app reads from or writes to the sitevet SDK, kept free of UI code.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use sitevet::{AuditReport, History, Options, PageResult, RunSummary, Status};

/// One audited site in the left pane: its runs (oldest first) and the latest report.
#[derive(Clone)]
pub struct Site {
    pub origin: String,
    /// Oldest first, so `runs[i]` is "Run i+1".
    pub runs: Vec<RunSummary>,
    pub latest: Arc<AuditReport>,
}

impl Site {
    pub fn latest_run(&self) -> &RunSummary {
        self.runs.last().expect("a site has at least one run")
    }

    /// "Run n" label for a history ID, counted per site.
    pub fn run_number(&self, id: sitevet::RunId) -> Option<usize> {
        self.runs.iter().position(|r| r.id == id).map(|i| i + 1)
    }

    pub fn host(&self) -> &str {
        host_of(&self.origin)
    }
}

/// Fail and warn counts for one page: "9 ISSUES · 2 FAIL".
pub struct Issues {
    pub issues: usize,
    pub fail: usize,
}

pub fn page_issues(page: &PageResult) -> Issues {
    let issues = page.findings.iter().filter(|f| matches!(f.status, Status::Fail | Status::Warn)).count();
    let fail = page.findings.iter().filter(|f| f.status == Status::Fail).count();
    Issues { issues, fail }
}

/// For a whole site, issues are distinct failing or warning rules and fail counts every
/// failing finding, so "9 ISSUES · 29 FAIL" reads as nine problems found 29 times.
pub fn site_issues(report: &AuditReport) -> Issues {
    let findings = report.pages.iter().flat_map(|p| &p.findings);
    let mut rules = BTreeSet::new();
    let mut fail = 0;
    for f in findings {
        if matches!(f.status, Status::Fail | Status::Warn) {
            rules.insert(f.rule_id.as_str());
        }
        if f.status == Status::Fail {
            fail += 1;
        }
    }
    Issues { issues: rules.len(), fail }
}

/// Totals across categories: (pass, warn, fail).
pub fn totals(report: &AuditReport) -> (u32, u32, u32) {
    report.categories.iter().fold((0, 0, 0), |(p, w, f), c| (p + c.pass, w + c.warn, f + c.fail))
}

pub fn issues_label(i: &Issues) -> String {
    let noun = if i.issues == 1 { "ISSUE" } else { "ISSUES" };
    match i.fail {
        0 => format!("{} {noun}", i.issues),
        n => format!("{} {noun} · {n} FAIL", i.issues),
    }
}

/// `abdulkadersafi.com` from `https://abdulkadersafi.com/blog`.
pub fn host_of(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

/// `/work/vouch` from a full page URL, `/` for the home page.
pub fn path_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    match rest.find('/') {
        Some(i) => rest[i..].to_string(),
        None => "/".into(),
    }
}

/// Loads every site in history with its newest report. Blocking: call it off the UI thread.
pub fn load_sites() -> Result<Vec<Site>, String> {
    let history = History::open(None).map_err(|e| e.to_string())?;
    load_sites_from(&history)
}

pub fn load_sites_from(history: &History) -> Result<Vec<Site>, String> {
    let all = history.list(None, usize::MAX >> 1).map_err(|e| e.to_string())?;
    // `list` is newest first; group by origin keeping that order for the site list.
    let mut origins: Vec<String> = Vec::new();
    for r in &all {
        if !origins.contains(&r.origin) {
            origins.push(r.origin.clone());
        }
    }
    let mut sites = Vec::new();
    for origin in origins {
        let mut runs: Vec<RunSummary> = all.iter().filter(|r| r.origin == origin).cloned().collect();
        runs.reverse();
        let latest_id = runs.last().map(|r| r.id).unwrap_or_default();
        // A run saved by a newer sitevet can't be read; skip the site rather than fail the list.
        let Ok(report) = history.load(latest_id) else { continue };
        sites.push(Site { origin, runs, latest: Arc::new(report) });
    }
    Ok(sites)
}

/// Reads one stored report. Blocking.
pub fn load_report(id: sitevet::RunId) -> Result<AuditReport, String> {
    History::open(None).and_then(|h| h.load(id)).map_err(|e| e.to_string())
}

/// Saves a finished report, first attaching a diff against the site's previous run when
/// `compare` is on. Returns the new run ID. Blocking.
pub fn save_report(report: &mut AuditReport, compare: bool) -> Result<sitevet::RunId, String> {
    let history = History::open(None).map_err(|e| e.to_string())?;
    if compare
        && let Some(prev) = history.latest_for(&report.start_url).map_err(|e| e.to_string())?
        && let Ok(baseline) = history.load(prev)
    {
        let mut diff = sitevet::diff(&baseline, report);
        diff.baseline.run_id = Some(prev);
        report.changes = Some(diff);
    }
    history.save(report).map_err(|e| e.to_string())
}

/// What the new audit form edits. `options` holds every SDK setting; the two flags below
/// are app-level switches with no field of their own in `Options`.
#[derive(Clone)]
pub struct AuditSettings {
    pub options: Options,
    pub check_external_links: bool,
    pub compare_with_last: bool,
    pub sites_at_a_time: usize,
}

impl Default for AuditSettings {
    fn default() -> Self {
        Self { options: Options::default(), check_external_links: true, compare_with_last: true, sites_at_a_time: 2 }
    }
}

const EXTERNAL_LINKS_RULE: &str = "links/broken-external";

impl AuditSettings {
    /// The options one audit runs with.
    // ponytail: the link checker still sends HEAD requests to external links when this is
    // off; only the rule's verdict is dropped. Skipping the requests needs an SDK option.
    pub fn options_for_run(&self) -> Options {
        let mut options = self.options.clone();
        if !self.check_external_links {
            let rules = sitevet::all_rules().into_iter().filter(|r| r.meta().id != EXTERNAL_LINKS_RULE).collect();
            options.rules = Some(rules);
        }
        options
    }
}

/// `defaults.toml` next to the history database: the SDK's own `sitevet.toml` keys plus a
/// `[desktop]` table for the app-level switches.
fn defaults_path() -> Option<PathBuf> {
    History::default_path().and_then(|p| p.parent().map(|d| d.join("desktop-defaults.toml")))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DesktopKeys {
    check_external_links: bool,
    compare_with_last: bool,
    sites_at_a_time: usize,
}

pub fn settings_to_toml(s: &AuditSettings) -> Result<String, String> {
    let mut table = toml::Table::try_from(&s.options).map_err(|e| e.to_string())?;
    let desktop = DesktopKeys {
        check_external_links: s.check_external_links,
        compare_with_last: s.compare_with_last,
        sites_at_a_time: s.sites_at_a_time,
    };
    table.insert("desktop".into(), toml::Value::try_from(desktop).map_err(|e| e.to_string())?);
    toml::to_string(&table).map_err(|e| e.to_string())
}

pub fn settings_from_toml(text: &str) -> Result<AuditSettings, String> {
    let mut table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let desktop = table.remove("desktop");
    let options = Options::from_toml_str(&toml::to_string(&table).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let mut s = AuditSettings { options, ..Default::default() };
    if let Some(d) = desktop.and_then(|d| d.try_into::<DesktopKeys>().ok()) {
        s.check_external_links = d.check_external_links;
        s.compare_with_last = d.compare_with_last;
        s.sites_at_a_time = d.sites_at_a_time.max(1);
    }
    Ok(s)
}

/// Saved defaults, or the SDK defaults when there are none or the file is unreadable.
pub fn load_defaults() -> AuditSettings {
    defaults_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| settings_from_toml(&t).ok())
        .unwrap_or_default()
}

pub fn save_defaults(s: &AuditSettings) -> Result<(), String> {
    let path = defaults_path().ok_or("no home folder found")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, settings_to_toml(s)?).map_err(|e| e.to_string())
}

/// Splits pasted text into URLs: one per line, blanks dropped, `https://` added when the
/// scheme is missing.
pub fn parse_urls(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| if l.contains("://") { l.to_string() } else { format!("https://{l}") })
        .collect()
}

/// Local-time helpers over the SDK's RFC 3339 UTC timestamps.
pub mod time {
    use jiff::{Timestamp, tz::TimeZone};

    fn local(rfc3339: &str) -> Option<jiff::ZonedDateTime> {
        rfc3339.parse::<Timestamp>().ok().map(|t| t.to_zoned_datetime(TimeZone::system()))
    }

    /// `2026-11-13 14:39`
    pub fn date_time(rfc3339: &str) -> String {
        local(rfc3339).map_or_else(|| rfc3339.to_string(), |z| z.strftime("%Y-%m-%d %H:%M").to_string())
    }

    /// `2026-11-13`
    pub fn date(rfc3339: &str) -> String {
        local(rfc3339).map_or_else(|| rfc3339.to_string(), |z| z.strftime("%Y-%m-%d").to_string())
    }

    /// `11-13`
    pub fn month_day(rfc3339: &str) -> String {
        local(rfc3339).map_or_else(|| rfc3339.to_string(), |z| z.strftime("%m-%d").to_string())
    }

    /// `14:52` for now.
    pub fn now_clock() -> String {
        jiff::ZonedDateTime::now().strftime("%H:%M").to_string()
    }

    /// Whole days between two timestamps.
    pub fn days_between(a: &str, b: &str) -> i64 {
        match (a.parse::<Timestamp>(), b.parse::<Timestamp>()) {
            (Ok(a), Ok(b)) => (b.as_second() - a.as_second()).abs() / 86_400,
            _ => 0,
        }
    }

    /// Days since a timestamp, for the "30 days" and "90 days" filters.
    pub fn age_days(rfc3339: &str) -> i64 {
        rfc3339.parse::<Timestamp>().map_or(i64::MAX, |t| (Timestamp::now().as_second() - t.as_second()) / 86_400)
    }
}

/// `189.4 S`, `2 min 14 s`.
pub fn duration_label(ms: u64) -> String {
    if ms < 60_000 { format!("{:.1} s", ms as f64 / 1000.0) } else { format!("{} min {} s", ms / 60_000, ms / 1000 % 60) }
}

/// `13,156`
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `+17`, `-2`, `±0`
pub fn signed(n: i64) -> String {
    match n {
        0 => "±0".into(),
        n if n > 0 => format!("+{n}"),
        n => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(score: u8, pages: &[(&str, &[(&str, &str)])]) -> AuditReport {
        let pages: Vec<String> = pages
            .iter()
            .map(|(url, findings)| {
                let f: Vec<String> = findings
                    .iter()
                    .map(|(rule, status)| {
                        format!(
                            r#"{{"rule_id":"{rule}","category":"links","title":"t","status":"{status}","severity":"error",
                               "message":"m","details":null,"evidence":null,"fix":null,"key":"{rule}{url}"}}"#
                        )
                    })
                    .collect();
                format!(
                    r#"{{"url":"{url}","final_url":"{url}","status":200,"error":null,"depth":0,"redirect_chain":[],
                       "response_ms":10,"html_bytes":10,"truncated":false,"score":90,"findings":[{}],"metrics":[]}}"#,
                    f.join(",")
                )
            })
            .collect();
        let json = format!(
            r#"{{"schema_version":1,"sitevet_version":"0.7.0","start_url":"https://a.com","final_url":"https://a.com",
               "generated_at":"2026-11-13T10:39:00Z","duration_ms":1200,"options":{{}},"score":{score},
               "categories":[],"pages":[{}],"blocked":[]}}"#,
            pages.join(",")
        );
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn issue_counts() {
        let r = report(
            90,
            &[
                ("https://a.com/", &[("a/x", "fail"), ("a/y", "warn"), ("a/z", "pass")]),
                ("https://a.com/b", &[("a/x", "fail")]),
            ],
        );
        let page = page_issues(&r.pages[0]);
        assert_eq!((page.issues, page.fail), (2, 1));
        assert_eq!(issues_label(&page), "2 ISSUES · 1 FAIL");
        let site = site_issues(&r);
        assert_eq!((site.issues, site.fail), (2, 2));
        assert_eq!(issues_label(&Issues { issues: 1, fail: 0 }), "1 ISSUE");
    }

    #[test]
    fn urls_and_numbers() {
        assert_eq!(host_of("https://dsrpt.com.au/think-tank?x=1"), "dsrpt.com.au");
        assert_eq!(path_of("https://a.com"), "/");
        assert_eq!(path_of("https://a.com/work/vouch"), "/work/vouch");
        assert_eq!(parse_urls("a.com\n\n  https://b.org  \n"), vec!["https://a.com", "https://b.org"]);
        assert_eq!(thousands(13156), "13,156");
        assert_eq!(thousands(100), "100");
        assert_eq!(signed(0), "±0");
        assert_eq!(signed(17), "+17");
        assert_eq!(duration_label(189_400), "3 min 9 s");
        assert_eq!(duration_label(38_200), "38.2 s");
    }

    #[test]
    fn defaults_round_trip() {
        let mut s = AuditSettings::default();
        s.options.max_pages = 42;
        s.check_external_links = false;
        s.sites_at_a_time = 3;
        let back = settings_from_toml(&settings_to_toml(&s).unwrap()).unwrap();
        assert_eq!(back.options.max_pages, 42);
        assert!(!back.check_external_links);
        assert_eq!(back.sites_at_a_time, 3);
    }

    #[test]
    fn external_links_switch_drops_only_that_rule() {
        let s = AuditSettings { check_external_links: false, ..Default::default() };
        let rules = s.options_for_run().rules.unwrap();
        assert_eq!(rules.len(), sitevet::all_rules().len() - 1);
        assert!(rules.iter().all(|r| r.meta().id != EXTERNAL_LINKS_RULE));
        assert!(AuditSettings::default().options_for_run().rules.is_none());
    }

    #[test]
    fn history_groups_runs_by_site() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(Some(dir.path().join("h.db"))).unwrap();
        let mut a = report(74, &[]);
        h.save(&a).unwrap();
        a.score = 91;
        let last = h.save(&a).unwrap();
        let sites = load_sites_from(&h).unwrap();
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].runs.len(), 2);
        assert_eq!(sites[0].run_number(last), Some(2));
        assert_eq!(sites[0].latest.score, 91);
        assert_eq!(sites[0].host(), "a.com");
    }
}
