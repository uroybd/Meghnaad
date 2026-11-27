//! `meghnaad export`: `task export` from a shell, with the Export dialog's choices as options. With none of them, it is
//! `task <words> export` exactly: its output, its messages and its exit status. With them, what `export` wrote is
//! reshaped by the engine's `tc_core::table_export`, as the dialog's file is: JSON with fewer fields or the other
//! shape, CSV, or TSV for `cut`, `awk` and `xargs`, each task with its tracked time (`time_tracking`, spec 0041). Like
//! `task export`, it never applies the context.

use crate::task::Task;
use std::io::{self, Write};
use tc_core::table_export::{self, Format};
use tc_core::taskrc::{self, Config};

pub const HELP: &str = "meghnaad export: task export, with the Export dialog's choices.

Usage: meghnaad export [options] [--] [filter and rc. words]

With no options it is `task <words> export`, exactly. With any, each task also has its
tracked time, `time_tracking`. Options come first; the first word that is not one (or `--`)
starts task's words.

      --rich                    our export with nothing else changed: task's, plus time_tracking
  -f, --format json|csv|tsv     the output (json)
  -F, --fields all|table|a,b,c  every field, the report's columns (needs -r), or these, in order
      --include-time-tracking   time_tracking as well as the fields -F chose
  -r, --report NAME             task <filter> export NAME: the report's filter and sort
  -a, --array / -A, --no-array  JSON: a list, or a task to a line (as json.array says)
  -H, --no-header               CSV and TSV: no header line
  -0, --null                    TSV: end each task with NUL instead of a newline (xargs -0)
  -h, --help                    this

time_tracking is {\"total\": seconds, \"sessions\": [{\"start\", \"end\", \"duration\"}]}, from the
journal.time notes as the Time log pairs them: dates as task writes them, durations in
seconds, end null while running. Untracked is a total of 0. In CSV and TSV it is the total.

CSV and TSV write dates as YYYY-MM-DD HH:MM:SS, in UTC. TSV escapes tab, newline, carriage
return and backslash as \\t \\n \\r \\\\ (printf '%b' undoes it), so a line is a task.
";

/// Which fields: every one, the report's columns, or these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fields {
    All,
    Table,
    These(Vec<String>),
}

/// The options given, and the words for `task`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    pub format: Format,
    pub fields: Fields,
    pub report: Option<String>,
    pub array: Option<bool>,
    pub header: bool,
    pub null: bool,
    pub help: bool,
    /// `time_tracking` as well as the fields `-F` chose.
    pub time_tracking: bool,
    /// Whether any option was given (`--rich` is one that changes nothing else): none, and the export is `task`'s own.
    pub any: bool,
    pub words: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::Json,
            fields: Fields::All,
            report: None,
            array: None,
            header: true,
            null: false,
            help: false,
            time_tracking: false,
            any: false,
            words: Vec::new(),
        }
    }
}

/// The options in `args` (what follows `export`). Ours come first: the first word that is not one of ours starts the
/// task words, so a filter word such as `-home` goes to `task`; `--` ends ours on purpose. A word starting with `--`
/// that is not ours is a mistake, since no task word starts so.
///
/// # Errors
///
/// An unknown `--` option, a value missing or not one of the choices, or options that do not go together.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut o = Options::default();
    let mut i = 0;
    while let Some(word) = args.get(i) {
        let (name, inline) = match word.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n, Some(v.to_owned())),
            _ => (word.as_str(), None),
        };
        let mut value = |what: &str| -> Result<String, String> {
            if let Some(v) = inline.clone() {
                return Ok(v);
            }
            i += 1;
            args.get(i).cloned().ok_or_else(|| format!("{name} needs {what}"))
        };
        match name {
            "--" => {
                i += 1;
                break;
            }
            "-f" | "--format" => {
                o.format = match value("a format")?.as_str() {
                    "json" => Format::Json,
                    "csv" => Format::Csv,
                    "tsv" => Format::Tsv,
                    other => return Err(format!("{other:?} is not a format: json, csv or tsv")),
                };
            }
            "-F" | "--fields" => {
                o.fields = match value("the fields")?.as_str() {
                    "all" => Fields::All,
                    "table" => Fields::Table,
                    list => {
                        let names: Vec<String> = list
                            .split(',')
                            .map(str::trim)
                            .filter(|f| !f.is_empty())
                            .map(String::from)
                            .collect();
                        if names.is_empty() {
                            return Err("--fields needs all, table or a list of fields".to_owned());
                        }
                        Fields::These(names)
                    }
                };
            }
            "-r" | "--report" => o.report = Some(value("a report")?),
            "-a" | "--array" => o.array = Some(true),
            "-A" | "--no-array" => o.array = Some(false),
            "-H" | "--no-header" => o.header = false,
            "-0" | "--null" => o.null = true,
            "-h" | "--help" => o.help = true,
            "--rich" => {}
            "--include-time-tracking" => o.time_tracking = true,
            _ if name.starts_with("--") => return Err(format!("{name} is not an option of meghnaad export")),
            _ => break,
        }
        o.any = true;
        i += 1;
    }
    o.words = args.get(i..).unwrap_or_default().to_vec();
    if o.array.is_some() && o.format != Format::Json {
        return Err("--array and --no-array are for JSON".to_owned());
    }
    if !o.header && o.format == Format::Json {
        return Err("--no-header is for CSV and TSV".to_owned());
    }
    if o.null && o.format != Format::Tsv {
        return Err("--null is for TSV".to_owned());
    }
    if o.fields == Fields::Table && o.report.is_none() {
        return Err("--fields table needs a report (-r)".to_owned());
    }
    Ok(o)
}

/// The command line for `task`: its words, `export`, and the report.
pub fn line(o: &Options) -> Vec<String> {
    let mut line = o.words.clone();
    line.push("export".to_owned());
    line.extend(o.report.clone());
    line
}

/// What `export` wrote (`text`), shaped as `o` asks, each task with its `time_tracking` as of `now`, with `cfg` for
/// `json.array`, the report's columns, which fields are dates and the `journal.time` notes.
///
/// # Errors
///
/// `text` is not what `export` writes.
pub fn shape(o: &Options, text: &str, cfg: &Config, now: i64) -> Result<String, String> {
    let tasks = table_export::with_time_tracking(table_export::parse(text)?, cfg, now);
    let mut fields = match &o.fields {
        Fields::All => table_export::fields_of(&tasks),
        Fields::These(f) => f.clone(),
        Fields::Table => {
            let columns = o
                .report
                .as_deref()
                .and_then(|r| tc_core::report::resolve(cfg, r))
                .map(|d| d.columns)
                .unwrap_or_default();
            table_export::table_fields(&columns)
        }
    };
    if o.time_tracking && !fields.iter().any(|f| f == table_export::TIME_TRACKING) {
        fields.push(table_export::TIME_TRACKING.to_owned());
    }
    Ok(match o.format {
        Format::Csv => {
            let csv = table_export::to_csv(&tasks, &fields, cfg);
            if o.header {
                csv
            } else {
                // The header is the first line; a field's own line break is inside quotes, after it.
                csv.split_once("\r\n")
                    .map_or(String::new(), |(_, rest)| rest.to_owned())
            }
        }
        Format::Tsv => table_export::to_tsv(&tasks, &fields, cfg, o.header, if o.null { '\0' } else { '\n' }),
        Format::Json => {
            let array = o.array.unwrap_or_else(|| cfg.json_array());
            if o.fields == Fields::All {
                table_export::to_json_all(text, &tasks, array)
            } else {
                table_export::to_json(&tasks, &fields, array)
            }
        }
    })
}

/// The settings `task` runs the line with: `_show`, with the line's `rc.` words.
fn settings(task: &Task, words: &[String]) -> Result<Config, String> {
    let mut args: Vec<&str> = vec!["rc.verbose=nothing", "rc.color=off"];
    args.extend(
        words
            .iter()
            .map(String::as_str)
            .filter(|w| w.starts_with("rc.") || w.starts_with("rc:")),
    );
    args.push("_show");
    let out = task.capture(&args)?;
    if !out.ok {
        return Err(crate::task::first_line(&out.stderr, &out.stdout));
    }
    Ok(taskrc::parse(&out.stdout).config)
}

/// Run `meghnaad export` with `args` (what follows `export`); the exit status for the shell.
///
/// # Errors
///
/// Writing to standard output or standard error failed.
pub fn run(task: &Task, args: &[String]) -> io::Result<i32> {
    let o = match parse(args) {
        Ok(o) => o,
        Err(why) => {
            writeln!(io::stderr(), "meghnaad export: {why}\nmeghnaad export --help says how.")?;
            return Ok(2);
        }
    };
    if o.help {
        io::stdout().write_all(HELP.as_bytes())?;
        return Ok(0);
    }
    if !o.any {
        return Ok(match task.inherit(&line(&o)) {
            Ok(status) => status.code().unwrap_or(1),
            Err(why) => {
                writeln!(io::stderr(), "meghnaad export: {why}")?;
                1
            }
        });
    }
    let out = match task.raw(&line(&o)) {
        Ok(out) => out,
        Err(why) => {
            writeln!(io::stderr(), "meghnaad export: {why}")?;
            return Ok(1);
        }
    };
    io::stderr().write_all(&out.stderr)?;
    if !out.status.success() {
        io::stdout().write_all(&out.stdout)?;
        return Ok(out.status.code().unwrap_or(1));
    }
    let now = crate::app::now();
    let shaped = settings(task, &o.words).and_then(|cfg| shape(&o, &String::from_utf8_lossy(&out.stdout), &cfg, now));
    match shaped {
        Ok(text) => {
            io::stdout().write_all(text.as_bytes())?;
            Ok(0)
        }
        Err(why) => {
            writeln!(io::stderr(), "meghnaad export: {why}")?;
            Ok(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(line: &str) -> Vec<String> {
        line.split_whitespace().map(String::from).collect()
    }
    fn words(line: &str) -> Result<(Options, Vec<String>), String> {
        parse(&args(line)).map(|o| {
            let w = o.words.clone();
            (o, w)
        })
    }

    #[test]
    fn with_no_options_the_words_go_to_task_as_they_are() {
        let (o, w) = words("project:work rc.json.array=off").unwrap();
        assert!(!o.any);
        assert_eq!(w, args("project:work rc.json.array=off"));
        assert_eq!(line(&o), args("project:work rc.json.array=off export"));
    }

    #[test]
    fn rich_is_an_option_that_changes_nothing_else() {
        let (o, w) = words("--rich project:work").unwrap();
        assert!(o.any && !o.time_tracking && o.fields == Fields::All && o.format == Format::Json);
        assert_eq!(w, args("project:work"));
        let (o, _) = words("--include-time-tracking -F table -r next").unwrap();
        assert!(o.any && o.time_tracking);
    }

    #[test]
    fn short_and_long_options_and_ours_stop_at_the_first_task_word() {
        let (o, w) = words("-f csv -F due,uuid -r next -H -home +x").unwrap();
        assert!(o.any && !o.header);
        assert_eq!((o.format, o.report.as_deref()), (Format::Csv, Some("next")));
        assert_eq!(o.fields, Fields::These(args("due uuid")));
        assert_eq!(w, args("-home +x"));
        assert_eq!(line(&o), args("-home +x export next"));
        let (o, w) = words("--format=tsv --null --no-header -- -f").unwrap();
        assert_eq!((o.format, o.null, o.header), (Format::Tsv, true, false));
        assert_eq!(w, args("-f"));
        let (o, _) = words("--fields table --report=list -A").unwrap();
        assert_eq!((o.fields, o.array), (Fields::Table, Some(false)));
    }

    #[test]
    fn options_that_are_wrong_or_do_not_go_together_are_refused() {
        for (line, why) in [
            ("--fromat csv", "not an option"),
            ("-f xml", "not a format"),
            ("-f", "needs a format"),
            ("-f csv -a", "for JSON"),
            ("-H", "for CSV and TSV"),
            ("-f csv -0", "for TSV"),
            ("-F table", "needs a report"),
            ("-F ,", "list of fields"),
        ] {
            let got = parse(&args(line)).unwrap_err();
            assert!(got.contains(why), "{line}: {got}");
        }
    }

    const TEXT: &str = "[\n{\"id\":1,\"description\":\"a\\tb\",\"due\":\"20261119T180000Z\",\"uuid\":\"u1\"},\n\
        {\"id\":2,\"description\":\"c\",\"uuid\":\"u2\"}\n]\n";

    #[test]
    fn the_export_is_shaped_as_asked() {
        let cfg = taskrc::parse("report.mine.columns=id,due.relative\n").config;
        let opts = |line: &str| parse(&args(line)).unwrap();
        let none = ",\"time_tracking\":{\"total\":0,\"sessions\":[]}}";
        let rich = TEXT
            .replace("\"u1\"}", &format!("\"u1\"{none}"))
            .replace("\"u2\"}", &format!("\"u2\"{none}"));
        assert_eq!(shape(&opts("-a"), TEXT, &cfg, 0).unwrap(), rich);
        assert_eq!(shape(&opts("--rich"), TEXT, &cfg, 0).unwrap(), rich);
        assert_eq!(
            shape(&opts("-F uuid --include-time-tracking -A"), TEXT, &cfg, 0).unwrap(),
            format!("{{\"uuid\":\"u1\"{none}\n{{\"uuid\":\"u2\"{none}\n")
        );
        assert_eq!(
            shape(
                &opts("-f tsv -F table -r mine --include-time-tracking -H"),
                TEXT,
                &cfg,
                0
            )
            .unwrap(),
            "1\t2026-11-19 18:00:00\t0\n2\t\t0\n"
        );
        assert_eq!(
            shape(&opts("-F uuid,id -A"), TEXT, &cfg, 0).unwrap(),
            "{\"uuid\":\"u1\",\"id\":1}\n{\"uuid\":\"u2\",\"id\":2}\n"
        );
        assert_eq!(
            shape(&opts("-f tsv -F table -r mine"), TEXT, &cfg, 0).unwrap(),
            "id\tdue\n1\t2026-11-19 18:00:00\n2\t\n"
        );
        assert_eq!(shape(&opts("-f tsv -F uuid -H -0"), TEXT, &cfg, 0).unwrap(), "u1\0u2\0");
        assert_eq!(
            shape(&opts("-f tsv -F description -H"), TEXT, &cfg, 0).unwrap(),
            "a\\tb\nc\n"
        );
        assert_eq!(
            shape(&opts("-f csv -F id,due -H"), TEXT, &cfg, 0).unwrap(),
            "1,2026-11-19 18:00:00\r\n2,\r\n"
        );
    }
}
