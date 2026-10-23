//! The taskrc as a folder of files: what is kept of each, how `include` finds the others, and how a change to a
//! setting is written back as text.
//!
//! The text of every file is kept as it was written, comments and layout included. Only two things are changed on
//! the way in ([`sanitize`]): a sync or credential-like setting is removed, so no secret is ever stored, and a line
//! this app cannot use is commented out, so it is flagged rather than lost.
//!
//! `include <file>` is read as Taskwarrior 3.5.0 reads it: a file is looked for next to the file that names it, the
//! included lines count where the `include` stands (so later lines win), and files may nest to ten levels. Where
//! `task` stops with an error (a missing file, nesting too deep), this reports a problem and goes on.

use crate::taskrc::{self, Config, Parsed, Verdict};
use std::collections::{BTreeMap, BTreeSet};

/// The file everything starts from: what `TASKRC` points at.
pub const ROOT: &str = "taskrc";
/// The most files a folder may hold, and the most text in all of them.
pub const MAX_FILES: usize = 64;
pub const MAX_TOTAL_BYTES: usize = 1024 * 1024;
/// Taskwarrior: "Configuration files may only be nested to 10 levels."
pub const MAX_DEPTH: usize = 10;
const MAX_PATH_BYTES: usize = 200;

/// The marker in front of a line this app cannot use.
const UNSUPPORTED: &str = "# [unsupported] ";

/// The files, by path relative to the folder (`taskrc`, `holidays/en-US.rc`).
pub type Tree = BTreeMap<String, String>;

/// A path inside the folder, written the one way: `/` between parts, no `.` or `..`, nothing absolute.
pub fn clean_path(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    let raw = raw.strip_prefix("./").unwrap_or(raw);
    if raw.is_empty() || raw.len() > MAX_PATH_BYTES {
        return Err(format!("a file name must be 1 to {MAX_PATH_BYTES} bytes"));
    }
    if raw.starts_with('/') || raw.starts_with('~') || raw.contains('\\') || raw.chars().any(char::is_control) {
        return Err(format!("'{raw}' is not a name inside the folder"));
    }
    for part in raw.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(format!("'{raw}' is not a name inside the folder"));
        }
    }
    Ok(raw.to_owned())
}

/// The setting name that makes a line unfit to keep, if there is one. The whole line is looked at, a comment
/// included: comments are kept as written now, and `# sync.encryption_secret=...` is a secret all the same.
fn secret_name(line: &str) -> Option<String> {
    for (at, _) in line.match_indices('=') {
        let before = line[..at].trim_end();
        let start = before
            .rfind(|c: char| c.is_whitespace() || c == '#')
            .map_or(0, |i| i + 1);
        let name = &before[start..];
        if name.is_empty() {
            continue;
        }
        if taskrc::is_sensitive(name) {
            return Some(name.to_owned());
        }
        // A context's own settings: `context.work.rc.sync.x`.
        if name.starts_with("context.") {
            if let Some((_, key)) = name.split_once(".rc.") {
                if taskrc::is_sensitive(key) {
                    return Some(name.to_owned());
                }
            }
        }
    }
    None
}

/// What [`sanitize`] kept, and what it did not.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sanitized {
    pub text: String,
    /// Names removed for being sync or credential-like. Never their values.
    pub blocked: Vec<String>,
    /// The lines (counting from 1) that were commented out because they cannot be used, and why.
    pub commented: Vec<(usize, String)>,
}

/// The text to keep for a file: as written, except that sync and credential lines are removed and lines this app
/// cannot use are commented out. Running it again on its own result changes nothing.
pub fn sanitize(text: &str) -> Sanitized {
    let pieces: Vec<&str> = text.split_inclusive('\n').collect();
    let contents: Vec<&str> = pieces.iter().map(|p| p.trim_end_matches(['\n', '\r'])).collect();
    let (parsed, verdicts) = taskrc::parse_lines(&contents);
    let mut out = Sanitized::default();
    let mut blocked: BTreeSet<String> = parsed.blocked.iter().cloned().collect();
    for (i, piece) in pieces.iter().enumerate() {
        let content = contents[i];
        let ending = &piece[content.len()..];
        if let Some(name) = secret_name(content) {
            blocked.insert(name);
            continue;
        }
        match verdicts[i] {
            // A sync or credential line: not kept.
            Verdict::Blocked => {}
            Verdict::Unsupported => {
                let why = unsupported_reason(content, &parsed);
                out.commented.push((i + 1, why));
                out.text.push_str(UNSUPPORTED);
                out.text.push_str(content.trim());
                out.text.push_str(ending);
            }
            Verdict::Kept | Verdict::Include => out.text.push_str(piece),
        }
    }
    out.blocked = blocked.into_iter().collect();
    out
}

fn unsupported_reason(line: &str, parsed: &Parsed) -> String {
    let line = line.split('#').next().unwrap_or("").trim();
    let Some((name, _)) = line.split_once('=') else {
        return "not a setting".to_owned();
    };
    let name = name.trim();
    if parsed.ignored.iter().any(|n| n == name) {
        "not a setting this app reads".to_owned()
    } else if let Some(w) = parsed.warnings.iter().find(|w| w.starts_with(&format!("{name}:"))) {
        w.clone()
    } else {
        "not usable".to_owned()
    }
}

/// An `include` line and where it points.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Include {
    /// The line it is on, counting from 1.
    pub line: usize,
    /// As written.
    pub target: String,
    /// The file it names, when that is in the folder.
    pub found: Option<String>,
}

/// A trouble spotted while following the includes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Problem {
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl Problem {
    pub fn text(&self) -> String {
        format!("{}:{}: {}", self.file, self.line, self.message)
    }
}

/// The settings of a whole folder, and what was found on the way.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub parsed: Parsed,
    pub problems: Vec<Problem>,
    /// For each file reached from the root, the includes in it.
    pub includes: BTreeMap<String, Vec<Include>>,
}

fn dir_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(d, _)| d)
}

/// `target` next to `from`, with `.` and `..` worked out. `None` if it leaves the folder.
fn join(from: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = dir_of(from).split('/').filter(|p| !p.is_empty()).collect();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

struct Walk<'a> {
    tree: &'a Tree,
    lines: Vec<&'a str>,
    problems: Vec<Problem>,
    includes: BTreeMap<String, Vec<Include>>,
}

impl<'a> Walk<'a> {
    fn file(&mut self, path: &'a str, stack: &mut Vec<&'a str>) {
        let Some(text) = self.tree.get(path) else { return };
        let mut found = Vec::new();
        for (i, line) in text.lines().enumerate() {
            let Some(target) = taskrc::include_target(line) else {
                self.lines.push(line);
                continue;
            };
            let mut include = Include {
                line: i + 1,
                target: target.to_owned(),
                found: None,
            };
            let problem = |message: String| Problem {
                file: path.to_owned(),
                line: i + 1,
                message,
            };
            if target.is_empty() {
                self.problems.push(problem("include names no file".to_owned()));
            } else if target.starts_with(['/', '~', '$']) {
                self.problems.push(problem(format!(
                    "include {target}: only a file in this folder can be included, written relative to this file"
                )));
            } else if let Some(name) = join(path, target) {
                if let Some((key, _)) = self.tree.get_key_value(name.as_str()) {
                    include.found = Some(key.clone());
                    if stack.contains(&key.as_str()) {
                        self.problems
                            .push(problem(format!("include {target}: the file includes itself")));
                    } else if stack.len() >= MAX_DEPTH {
                        self.problems.push(problem(format!(
                            "include {target}: files may only be nested to {MAX_DEPTH} levels"
                        )));
                    } else {
                        stack.push(key.as_str());
                        self.file(key.as_str(), stack);
                        stack.pop();
                    }
                } else {
                    self.problems.push(problem(format!(
                        "include {target}: there is no such file in the folder"
                    )));
                }
            } else {
                self.problems
                    .push(problem(format!("include {target}: that is outside the folder")));
            }
            found.push(include);
        }
        if !found.is_empty() {
            self.includes.entry(path.to_owned()).or_default().extend(found);
        }
    }
}

/// The settings of the folder, read from [`ROOT`] and whatever it includes.
pub fn resolve(tree: &Tree) -> Resolved {
    let mut walk = Walk {
        tree,
        lines: Vec::new(),
        problems: Vec::new(),
        includes: BTreeMap::new(),
    };
    if let Some((root, _)) = tree.get_key_value(ROOT) {
        walk.file(root.as_str(), &mut vec![root.as_str()]);
    }
    let (mut parsed, _) = taskrc::parse_lines(&walk.lines);
    parsed.warnings.extend(walk.problems.iter().map(Problem::text));
    Resolved {
        parsed,
        problems: walk.problems,
        includes: walk.includes,
    }
}

/// Files that no include reaches from the root (kept, but with no effect).
pub fn unreached(tree: &Tree, resolved: &Resolved) -> Vec<String> {
    let mut reached: BTreeSet<&str> = BTreeSet::from([ROOT]);
    for list in resolved.includes.values() {
        reached.extend(list.iter().filter_map(|i| i.found.as_deref()));
    }
    tree.keys().filter(|k| !reached.contains(k.as_str())).cloned().collect()
}

/// What to do with one setting in the root file.
pub type Edit = (String, Option<String>);

/// The changes that turn the settings `old` into `new`, as `config` would make them: a name and its new value, or
/// `None` to remove it.
pub fn edits(old: &Config, new: &Config) -> Vec<Edit> {
    let old: BTreeMap<String, String> = taskrc::render_pairs(old).into_iter().collect();
    let new: BTreeMap<String, String> = taskrc::render_pairs(new).into_iter().collect();
    let mut out = Vec::new();
    for (k, v) in &new {
        if old.get(k) != Some(v) {
            out.push((k.clone(), Some(v.clone())));
        }
    }
    for k in old.keys() {
        if !new.contains_key(k) {
            out.push((k.clone(), None));
        }
    }
    out
}

/// Set (or, with `None`, remove) a setting in a file as `task config` does: every line that starts with
/// `name=` takes the new value and keeps its comment; with no such line a new one goes at the end. A key an included
/// file defines is left alone there; the new line overrides it.
pub fn set_setting(text: &str, name: &str, value: Option<&str>) -> String {
    let mut out = String::with_capacity(text.len() + 32);
    let mut seen = false;
    for piece in text.split_inclusive('\n') {
        let content = piece.trim_end_matches(['\n', '\r']);
        let ending = &piece[content.len()..];
        let Some(rest) = content.strip_prefix(name).and_then(|r| r.strip_prefix('=')) else {
            out.push_str(piece);
            continue;
        };
        seen = true;
        let Some(value) = value else { continue };
        out.push_str(name);
        out.push('=');
        out.push_str(value);
        // What was after the value (spaces and a comment) stays.
        if let Some(h) = rest.find('#') {
            let gap = &rest[..h];
            out.push_str(&gap[gap.trim_end().len()..]);
            out.push_str(&rest[h..]);
        }
        out.push_str(ending);
    }
    if let (false, Some(value)) = (seen, value) {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(name);
        out.push('=');
        out.push_str(value);
        out.push('\n');
    }
    out
}

/// Apply [`edits`] to the root file of the folder.
pub fn apply_edits(tree: &mut Tree, edits: &[Edit]) {
    let mut text = tree.get(ROOT).cloned().unwrap_or_default();
    for (name, value) in edits {
        text = set_setting(&text, name, value.as_deref());
    }
    if !(text.is_empty() && !tree.contains_key(ROOT)) {
        tree.insert(ROOT.to_owned(), text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> Tree {
        files.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn paths_stay_inside_the_folder() {
        assert_eq!(clean_path("./holidays/en-US.rc").as_deref(), Ok("holidays/en-US.rc"));
        for bad in [
            "",
            "/etc/passwd",
            "../x",
            "a/../b",
            "a//b",
            "a/./b",
            "~/x",
            "a\\b",
            "a\nb",
            "a/",
        ] {
            assert!(clean_path(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn text_is_kept_as_written() {
        let text = "# my rc\r\n\r\nbulk=3   # three\r\n  default.project =  P1\r\nweekstart=Monday";
        let s = sanitize(text);
        assert_eq!(s.text, text);
        assert!(s.blocked.is_empty() && s.commented.is_empty());
    }

    #[test]
    fn credentials_are_removed_comments_and_all() {
        let text = "bulk=3\n\
                    sync.encryption_secret=hunter2\n\
                    # sync.server.origin = https://example.com\n\
                    bulk=4 # was sync.server.client_id=abc\n\
                    uda.estimate.sync.token=xyz\n\
                    context.work.rc.sync.encryption_secret=zzz\n\
                    default.project=Home\n";
        let s = sanitize(text);
        assert_eq!(s.text, "bulk=3\ndefault.project=Home\n");
        assert!(!s.text.contains("hunter2") && !s.text.contains("example.com") && !s.text.contains("abc"));
        assert!(!s.text.contains("xyz") && !s.text.contains("zzz"));
        assert!(s.blocked.iter().any(|n| n == "sync.encryption_secret"));
        assert!(s.blocked.iter().all(|n| !n.contains("hunter2")));
    }

    #[test]
    fn what_cannot_be_used_is_commented_out_once() {
        let text = "bulk=3\nsome.unknown.key=1\nthis is not a setting\nlimit=lots\n# note\ninclude other.rc\n";
        let s = sanitize(text);
        assert_eq!(
            s.text,
            "bulk=3\n# [unsupported] some.unknown.key=1\n# [unsupported] this is not a setting\n\
             # [unsupported] limit=lots\n# note\ninclude other.rc\n"
        );
        assert_eq!(s.commented.iter().map(|(l, _)| *l).collect::<Vec<_>>(), [2, 3, 4]);
        assert_eq!(sanitize(&s.text).text, s.text, "stable on a second pass");
        assert!(sanitize(&s.text).commented.is_empty());
    }

    #[test]
    fn include_reads_next_to_the_file_and_later_lines_win() {
        let t = tree(&[
            ("taskrc", "bulk=3\ninclude sub/a.rc\nbulk=9\n"),
            ("sub/a.rc", "bulk=5\ndefault.project=FromA\ninclude b.rc\n"),
            ("sub/b.rc", "default.project=FromB\n"),
        ]);
        let r = resolve(&t);
        assert!(r.problems.is_empty(), "{:?}", r.problems);
        assert_eq!(r.parsed.config.settings["bulk"], "9");
        assert_eq!(r.parsed.config.settings["default.project"], "FromB");
        assert_eq!(r.includes["taskrc"][0].found.as_deref(), Some("sub/a.rc"));
        let t = tree(&[("taskrc", "include sub/a.rc\nbulk=2\n"), ("sub/a.rc", "bulk=5\n")]);
        assert_eq!(resolve(&t).parsed.config.settings["bulk"], "2");
        let t = tree(&[("taskrc", "bulk=2\ninclude sub/a.rc\n"), ("sub/a.rc", "bulk=5\n")]);
        assert_eq!(resolve(&t).parsed.config.settings["bulk"], "5");
    }

    #[test]
    fn include_trouble_is_reported_and_the_rest_still_read() {
        let t = tree(&[
            (
                "taskrc",
                "include nothere.rc\ninclude /etc/x.rc\ninclude ../up.rc\ninclude c1.rc\nbulk=4\n",
            ),
            ("c1.rc", "include c2.rc\n"),
            ("c2.rc", "include c1.rc\ndefault.project=Deep\n"),
        ]);
        let r = resolve(&t);
        assert_eq!(r.parsed.config.settings["bulk"], "4");
        assert_eq!(r.parsed.config.settings["default.project"], "Deep");
        let text: Vec<String> = r.problems.iter().map(Problem::text).collect();
        assert!(
            text.iter()
                .any(|p| p.starts_with("taskrc:1:") && p.contains("no such file")),
            "{text:?}"
        );
        assert!(text
            .iter()
            .any(|p| p.starts_with("taskrc:2:") && p.contains("only a file in this folder")));
        assert!(text
            .iter()
            .any(|p| p.starts_with("taskrc:3:") && p.contains("outside the folder")));
        assert!(text
            .iter()
            .any(|p| p.starts_with("c2.rc:1:") && p.contains("includes itself")));
        assert!(r.parsed.warnings.iter().any(|w| w.contains("no such file")));
    }

    #[test]
    fn nesting_stops_at_ten_levels() {
        let mut files = vec![("taskrc".to_owned(), "include f1\n".to_owned())];
        for i in 1..=12 {
            files.push((format!("f{i}"), format!("include f{}\nbulk={i}\n", i + 1)));
        }
        let t: Tree = files.into_iter().collect();
        let r = resolve(&t);
        assert!(r.problems.iter().any(|p| p.message.contains("nested to 10 levels")));
    }

    #[test]
    fn files_nothing_includes_are_found() {
        let t = tree(&[("taskrc", "include a.rc\n"), ("a.rc", ""), ("lonely.rc", "bulk=1\n")]);
        assert_eq!(unreached(&t, &resolve(&t)), ["lonely.rc"]);
    }

    #[test]
    fn config_edits_the_root_only_as_task_config_does() {
        // Measured on `task` 3.5.0: lines that start with `name=` change in place and keep their comment, a name
        // with spaces round it is not matched, a missing one is appended, and removing deletes every match.
        let text = "# my rc\nbulk=3   # three\n  default.project =  P1\nbulk=4\ninclude sub/a.rc\n";
        let set = set_setting(text, "bulk", Some("8"));
        assert_eq!(
            set,
            "# my rc\nbulk=8   # three\n  default.project =  P1\nbulk=8\ninclude sub/a.rc\n"
        );
        let added = set_setting(&set, "default.project", Some("NewP"));
        assert!(added.ends_with("include sub/a.rc\ndefault.project=NewP\n"));
        assert_eq!(
            set_setting(&set, "bulk", None),
            "# my rc\n  default.project =  P1\ninclude sub/a.rc\n"
        );
        assert_eq!(set_setting("a=1", "b", Some("2")), "a=1\nb=2\n");
        assert_eq!(set_setting("", "b", Some("2")), "b=2\n");
        assert_eq!(set_setting("bulker=1\n", "bulk", Some("2")), "bulker=1\nbulk=2\n");
    }

    #[test]
    fn a_config_change_becomes_edits_to_the_root() {
        let mut t = tree(&[
            ("taskrc", "# mine\nbulk=3\ninclude a.rc\n"),
            ("a.rc", "default.project=A\n"),
        ]);
        let old = resolve(&t).parsed.config;
        let mut new = old.clone();
        new.settings.insert("bulk".into(), "7".into());
        new.settings.remove("default.project");
        new.settings.insert("weekstart".into(), "Monday".into());
        apply_edits(&mut t, &edits(&old, &new));
        assert_eq!(t["taskrc"], "# mine\nbulk=7\ninclude a.rc\nweekstart=Monday\n");
        assert_eq!(t["a.rc"], "default.project=A\n", "an included file is not touched");
        assert!(edits(&old, &old).is_empty());
    }
}
