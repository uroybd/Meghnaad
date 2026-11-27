//! The Export dialog, as the web app's and the phone's (`ExportDialog.svelte`): what a report table or a board's table
//! section shows, as a file shaped as the person likes. The command line, the file's name and the file itself are the
//! engine's (`tc_core::table_export`), so the three front ends make the same file for the same choice; only the drawing
//! and the saving (to a path here) are this app's.

use super::{Action, App, KeyEvent, Mode};
use crate::settings;
use ratatui::crossterm::event::{KeyCode, KeyModifiers};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tc_core::filter::split_words;
use tc_core::table_export::{self, Choice, Fields, Format};

/// A line of the dialog, top to bottom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Row {
    Format(Format),
    /// JSON only: a list, or a task to a line.
    Array,
    Fields(Fields),
    /// A field to tick, when choosing.
    Pick(String),
    /// Where the file goes.
    Path,
}

/// The dialog's state: what was exported, and the choices.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportBox {
    /// The command line it ran.
    pub line: Vec<String>,
    /// The table's column specs, for "only the table's columns".
    pub columns: Vec<String>,
    /// The engine's name for what `export` wrote (`tasks-next-….json`).
    pub name: String,
    /// What `export` wrote, or why it could not.
    pub text: Result<String, String>,
    pub count: usize,
    /// Every field the tasks have.
    pub available: Vec<String>,
    pub format: Format,
    pub array: bool,
    /// `json.array` as the taskrc has it.
    pub rc_array: bool,
    pub fields: Fields,
    pub picked: Vec<String>,
    pub sel: usize,
    pub path: String,
    /// The file at the path is there: the next ↵ replaces it.
    pub replacing: bool,
}

/// A format as the dialog names it.
pub fn format_name(format: Format) -> &'static str {
    match format {
        Format::Json => "JSON",
        Format::Csv => "CSV",
        Format::Tsv => "TSV",
    }
}

/// The extension of a format's file.
pub fn extension(format: Format) -> &'static str {
    match format {
        Format::Json => "json",
        Format::Csv => "csv",
        Format::Tsv => "tsv",
    }
}

impl ExportBox {
    /// The lines, as the web app's dialog has them: format, JSON's shape, the fields (and each to tick when choosing),
    /// then the path.
    pub fn rows(&self) -> Vec<Row> {
        let mut rows = vec![
            Row::Format(Format::Json),
            Row::Format(Format::Csv),
            Row::Format(Format::Tsv),
        ];
        if self.format == Format::Json {
            rows.push(Row::Array);
        }
        rows.extend([
            Row::Fields(Fields::All),
            Row::Fields(Fields::Table),
            Row::Fields(Fields::Custom),
        ]);
        if self.fields == Fields::Custom {
            rows.extend(self.available.iter().cloned().map(Row::Pick));
        }
        rows.push(Row::Path);
        rows
    }

    /// The row with the focus.
    pub fn current(&self) -> Row {
        let rows = self.rows();
        rows.get(self.sel).cloned().unwrap_or(Row::Path)
    }

    pub fn choice(&self) -> Choice {
        Choice {
            format: self.format,
            array: Some(self.array),
            fields: self.fields,
            picked: self.picked.clone(),
            time_tracking: false,
        }
    }

    /// How many fields the file will have.
    pub fn field_count(&self) -> usize {
        match self.fields {
            Fields::All => self.available.len(),
            Fields::Table => table_export::table_fields(&self.columns).len(),
            Fields::Custom => self.available.iter().filter(|f| self.picked.contains(f)).count(),
        }
    }

    /// Pick the row with the focus: a format, the shape, the fields, or a field to tick.
    pub fn choose(&mut self) {
        match self.current() {
            Row::Format(f) => {
                let (old, new) = (extension(self.format), extension(f));
                self.format = f;
                // The path follows the format while it still ends as the old one did.
                if let Some(stem) = self.path.strip_suffix(&format!(".{old}")) {
                    self.path = format!("{stem}.{new}");
                }
            }
            Row::Array => self.array = !self.array,
            Row::Fields(f) => self.fields = f,
            Row::Pick(f) => {
                if let Some(at) = self.picked.iter().position(|p| *p == f) {
                    self.picked.remove(at);
                } else {
                    self.picked.push(f);
                }
            }
            Row::Path => {}
        }
        // Rows come and go (the shape, the fields to tick): keep the focus inside them.
        self.sel = self.sel.min(self.rows().len().saturating_sub(1));
    }

    /// The file for the choice, as the engine makes it.
    ///
    /// # Errors
    ///
    /// Nothing was exported, or what was is not `export`'s.
    pub fn file(&self, cfg: &tc_core::taskrc::Config) -> Result<table_export::File, String> {
        let text = self.text.as_ref().map_err(Clone::clone)?;
        table_export::render(&self.name, text, &self.choice(), &self.columns, (cfg, super::now()))
    }
}

/// The last choices, as `tui.export` in `settings.json` keeps them: format, fields and the fields ticked.
pub fn remembered(v: &Value) -> (Format, Fields, Vec<String>) {
    let format = match v.get("format").and_then(Value::as_str) {
        Some("csv") => Format::Csv,
        Some("tsv") => Format::Tsv,
        _ => Format::Json,
    };
    let fields = match v.get("fields").and_then(Value::as_str) {
        Some("table") => Fields::Table,
        Some("custom") => Fields::Custom,
        _ => Fields::All,
    };
    let picked = v
        .get("picked")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(String::from).collect())
        .unwrap_or_default();
    (format, fields, picked)
}

/// Where a file is saved: a path as typed, `~` being the home folder, relative to the folder the app started in.
fn resolve(path: &str) -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match (path.strip_prefix("~/"), home) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

impl App {
    /// Open the dialog for the table `words` draw (its filter and report), with `columns` its column specs.
    pub fn open_export(&mut self, words: &[String], columns: Vec<String>) {
        let Some(line) = table_export::export_line(words, &self.cfg, None) else {
            self.status = "Only a report's table can be exported.".into();
            return;
        };
        let args: Vec<&str> = line.iter().map(String::as_str).collect();
        let text = self.task.read(&args);
        let tasks = text
            .as_ref()
            .ok()
            .and_then(|t| table_export::parse(t).ok())
            .unwrap_or_default();
        // Each task gets its `time_tracking` in the file, so it is one of the fields to tick.
        let available = table_export::fields_of(&table_export::with_time_tracking(
            tasks.clone(),
            &self.cfg,
            super::now(),
        ));
        let (report, filter) = match line.iter().position(|w| w == "export") {
            Some(at) => (line.get(at + 1).map(String::as_str), line.get(..at).unwrap_or_default()),
            None => (None, &line[..]),
        };
        let now = super::now();
        let name = table_export::file_name(report, filter, now, crate::model::local_offset(now));
        let (format, fields, mut picked) = remembered(&self.settings.tui.export);
        if picked.is_empty() {
            // Nothing ticked yet: start from the table's fields.
            picked = table_export::table_fields(&columns)
                .into_iter()
                .filter(|f| available.contains(f))
                .collect();
        }
        let stem = name.strip_suffix(".json").unwrap_or(&name);
        let path = format!("{stem}.{}", extension(format));
        let rc_array = self.cfg.json_array();
        self.mode = Mode::Export(Box::new(ExportBox {
            line,
            columns,
            name,
            count: tasks.len(),
            text,
            available,
            format,
            array: rc_array,
            rc_array,
            fields,
            picked,
            sel: 0,
            path,
            replacing: false,
        }));
    }

    /// The Export key on the Tasks page: the report as it is shown.
    pub(super) fn export_report(&mut self) {
        let mut words = split_words(&self.filter);
        words.push(self.report_name.clone());
        let columns = tc_core::report::resolve(&self.cfg, &self.report_name)
            .map(|d| d.columns)
            .unwrap_or_default();
        self.open_export(&words, columns);
    }

    /// The Export key on a board: its table section, or a choice of them when it has several.
    pub(super) fn export_board(&mut self) {
        let tables: Vec<(String, Vec<String>)> = self
            .board_data
            .iter()
            .filter_map(|(id, d)| match d {
                crate::boards::Data::Table(_, words) => Some((id.clone(), words.clone())),
                _ => None,
            })
            .collect();
        let titles: Vec<String> = tables
            .iter()
            .map(|(id, _)| {
                self.boards
                    .get(self.board)
                    .and_then(|b| b.sections.iter().find(|s| s.id == *id))
                    .map(|s| s.title.clone())
                    .filter(|t| !t.is_empty())
                    .unwrap_or_else(|| id.clone())
            })
            .collect();
        match tables.len() {
            0 => self.status = "This board has no table to export.".into(),
            1 => self.export_words(tables.first().map(|t| t.1.as_slice()).unwrap_or_default()),
            _ => {
                self.mode = Mode::ExportPick {
                    options: titles,
                    words: tables.into_iter().map(|t| t.1).collect(),
                    sel: 0,
                }
            }
        }
    }

    /// The dialog for a board section's words (its filter, then its report).
    pub(super) fn export_words(&mut self, words: &[String]) {
        let columns = words
            .last()
            .and_then(|r| tc_core::report::resolve(&self.cfg, r))
            .map(|d| d.columns)
            .unwrap_or_default();
        self.open_export(words, columns);
    }

    /// A key in the dialog: ↑/↓ the lines, Space picks, ↵ saves, Esc cancels; typing on the path line edits it.
    pub(super) fn in_export(&mut self, mut b: Box<ExportBox>, key: KeyEvent) -> Action {
        if b.replacing {
            b.replacing = false;
            if key.code == KeyCode::Enter {
                self.save_export(&b);
                return Action::None;
            }
            self.mode = Mode::Export(b);
            return Action::None;
        }
        let n = b.rows().len();
        let on_path = b.current() == Row::Path;
        match key.code {
            KeyCode::Esc => return Action::None,
            KeyCode::Up => b.sel = b.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Tab => b.sel = (b.sel + 1).min(n.saturating_sub(1)),
            KeyCode::BackTab => b.sel = b.sel.saturating_sub(1),
            KeyCode::Enter => {
                let path = resolve(&b.path);
                if b.text.is_err() || b.field_count() == 0 || b.path.trim().is_empty() {
                    self.status = "Nothing to save: no tasks were read, no fields chosen, or no path.".into();
                } else if path.exists() {
                    b.replacing = true;
                } else {
                    self.save_export(&b);
                    return Action::None;
                }
            }
            KeyCode::Backspace if on_path => {
                b.path.pop();
            }
            KeyCode::Char('u') if on_path && key.modifiers.contains(KeyModifiers::CONTROL) => b.path.clear(),
            KeyCode::Char(c) if on_path && !key.modifiers.contains(KeyModifiers::CONTROL) => b.path.push(c),
            KeyCode::Char('k') => b.sel = b.sel.saturating_sub(1),
            KeyCode::Char('j') => b.sel = (b.sel + 1).min(n.saturating_sub(1)),
            KeyCode::Char(' ') => b.choose(),
            _ => {}
        }
        self.mode = Mode::Export(b);
        Action::None
    }

    /// Write the file to the path, and keep the choice for next time.
    fn save_export(&mut self, b: &ExportBox) {
        let path = resolve(&b.path);
        let written = b.file(&self.cfg).and_then(|f| {
            std::fs::write(&path, f.text).map_err(|e| format!("could not write {}: {e}", path.display()))
        });
        self.status = match written {
            Ok(()) => format!("Exported {} to {}.", tasks_word(b.count), shown(&path)),
            Err(why) => why,
        };
        let fields = match b.fields {
            Fields::All => "all",
            Fields::Table => "table",
            Fields::Custom => "custom",
        };
        let keep = json!({ "format": extension(b.format), "fields": fields, "picked": b.picked });
        self.settings.tui.export = keep.clone();
        // The file was saved either way; a choice that could not be kept is only forgotten.
        settings::save_tui("export", keep).ok();
    }
}

fn tasks_word(n: usize) -> String {
    format!("{n} {}", if n == 1 { "task" } else { "tasks" })
}

/// A path as the status line says it: as typed when it is relative.
fn shown(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dialog() -> ExportBox {
        ExportBox {
            line: split_words("export next"),
            columns: split_words("id due.relative description"),
            name: "tasks-next-20261116-010203.json".into(),
            text: Ok("[\n{\"id\":1,\"description\":\"a\",\"due\":\"20261119T180000Z\",\"uuid\":\"u1\"}\n]\n".into()),
            count: 1,
            available: split_words("id description due uuid"),
            format: Format::Json,
            array: true,
            rc_array: true,
            fields: Fields::All,
            picked: Vec::new(),
            sel: 0,
            path: "tasks-next-20261116-010203.json".into(),
            replacing: false,
        }
    }

    #[test]
    fn the_lines_are_the_web_apps_in_its_order() {
        let mut b = dialog();
        assert_eq!(
            b.rows(),
            [
                Row::Format(Format::Json),
                Row::Format(Format::Csv),
                Row::Format(Format::Tsv),
                Row::Array,
                Row::Fields(Fields::All),
                Row::Fields(Fields::Table),
                Row::Fields(Fields::Custom),
                Row::Path,
            ]
        );
        b.format = Format::Tsv;
        b.fields = Fields::Custom;
        let rows = b.rows();
        assert!(!rows.contains(&Row::Array));
        assert_eq!(rows.iter().filter(|r| matches!(r, Row::Pick(_))).count(), 4);
    }

    #[test]
    fn picking_tsv_changes_the_file_and_the_path_follows() {
        let mut b = dialog();
        b.sel = 2;
        b.choose();
        assert_eq!(b.format, Format::Tsv);
        assert_eq!(b.path, "tasks-next-20261116-010203.tsv");
        let f = b.file(&tc_core::taskrc::Config::default()).unwrap();
        assert_eq!(f.name, "tasks-next-20261116-010203.tsv");
        assert_eq!(
            f.text,
            "id\tdescription\tdue\tuuid\ttime_tracking\n1\ta\t2026-11-19 18:00:00\tu1\t0\n"
        );
        // A path typed by hand stays as it is.
        b.path = "out.txt".into();
        b.sel = 1;
        b.choose();
        assert_eq!(b.path, "out.txt");
    }

    #[test]
    fn the_tables_fields_or_the_ones_ticked() {
        let mut b = dialog();
        b.fields = Fields::Table;
        assert_eq!(b.field_count(), 3);
        b.fields = Fields::Custom;
        b.sel = b.rows().iter().position(|r| *r == Row::Pick("uuid".into())).unwrap();
        b.choose();
        assert_eq!(b.picked, ["uuid"]);
        b.format = Format::Csv;
        let f = b.file(&tc_core::taskrc::Config::default()).unwrap();
        assert_eq!(f.text, "uuid\r\nu1\r\n");
        // CSV has no JSON shape line, so the field is a line higher.
        b.sel = b.rows().iter().position(|r| *r == Row::Pick("uuid".into())).unwrap();
        b.choose();
        assert_eq!(b.field_count(), 0);
    }

    #[test]
    fn the_last_choice_is_read_back_and_anything_else_is_the_default() {
        assert_eq!(
            remembered(&json!({ "format": "tsv", "fields": "custom", "picked": ["due", 3] })),
            (Format::Tsv, Fields::Custom, vec!["due".to_owned()])
        );
        assert_eq!(remembered(&Value::Null), (Format::Json, Fields::All, Vec::new()));
    }
}
