//! The Iter picker: a box over the task form (`Ctrl+O` on its Iter field) to choose an iterative task's rule, as the
//! web's and the phone's do. Two modes: Taskwarrior (its periods, or anything `task` takes, typed) and Calendar, which
//! builds an RFC 5545 rule (RRULE) as calendar apps do: every N days, weeks, months or years; on which weekdays or which
//! day of the month; ending never, on a day, or after a number of times. The rule is the engine's (`tc_core::iter_rule`).

use crate::completer::Input;
use crate::form::Form;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tc_core::complete::RECUR_PRESETS;
use tc_core::iter_rule::{self, CalendarRule, Freq, WEEKDAYS};

/// A line of the picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// Taskwarrior or Calendar.
    Mode,
    /// Taskwarrior: one of its periods.
    Preset,
    /// Taskwarrior: the rule as typed.
    Typed,
    /// Calendar: every how many …
    Every,
    /// … days, weeks, months or years.
    Unit,
    /// Weekly: which weekdays.
    Days,
    /// Monthly: which day of the month.
    Month,
    /// Never, on a day, after a number of times.
    Ends,
    /// Ends on this day.
    Until,
    /// Ends after this many times.
    Count,
}

impl Row {
    pub fn label(self) -> &'static str {
        match self {
            Row::Mode => "Written as",
            Row::Preset => "Period",
            Row::Typed => "Or typed",
            Row::Every => "Every",
            Row::Unit => "Unit",
            Row::Days => "On",
            Row::Month => "On",
            Row::Ends => "Ends",
            Row::Until => "On day",
            Row::Count => "After",
        }
    }
}

/// What a key did.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    /// Done: the rule to put in the field.
    Done(String),
    /// Closed without changing the field.
    Cancel,
}

/// One of the monthly choices: what it says, and the rule with it.
#[derive(Debug, Clone)]
pub struct MonthOption {
    pub key: String,
    pub label: String,
    pub rule: CalendarRule,
}

#[derive(Debug)]
pub struct IterPick {
    /// The form it is over, given back when it closes.
    pub form: Box<Form>,
    pub calendar: bool,
    /// The Taskwarrior mode's rule as typed (kept while the Calendar mode is tried).
    pub typed: Input,
    pub rule: Option<CalendarRule>,
    /// The day a new rule counts from (the task's due date, else today), `2026-12-25`.
    pub anchor: String,
    pub sel: usize,
    /// The weekday the cursor is on in the Days row (0 Monday).
    pub day: usize,
    /// The day it ends on, as typed.
    pub until: Input,
}

impl IterPick {
    /// The picker for `value`, over `form`: a rule the Calendar mode can show opens in it, anything else in the
    /// Taskwarrior mode.
    pub fn new(form: Form, value: &str, anchor: &str) -> IterPick {
        let rule = iter_rule::from_rrule(value);
        IterPick {
            form: Box::new(form),
            calendar: rule.is_some(),
            typed: Input::new(if rule.is_some() { "" } else { value }),
            until: Input::new(rule.as_ref().and_then(|r| r.until.as_deref()).unwrap_or(anchor)),
            rule,
            anchor: anchor.to_owned(),
            sel: 0,
            day: 0,
        }
    }

    /// The rule as it stands.
    pub fn value(&self) -> String {
        match (&self.rule, self.calendar) {
            (Some(r), true) => iter_rule::to_rrule(r),
            _ => self.typed.line.trim().to_owned(),
        }
    }

    /// The rule in words.
    pub fn words(&self) -> String {
        iter_rule::describe_iter(&self.value())
    }

    /// The lines it shows, for its mode and rule.
    pub fn rows(&self) -> Vec<Row> {
        let mut out = vec![Row::Mode];
        match (&self.rule, self.calendar) {
            (Some(r), true) => {
                out.extend([Row::Every, Row::Unit]);
                match r.freq {
                    Freq::Weekly => out.push(Row::Days),
                    Freq::Monthly if !self.month_options().is_empty() => out.push(Row::Month),
                    _ => {}
                }
                out.push(Row::Ends);
                if r.until.is_some() {
                    out.push(Row::Until);
                } else if r.count.is_some() {
                    out.push(Row::Count);
                }
            }
            _ => out.extend([Row::Preset, Row::Typed]),
        }
        out
    }

    pub fn row(&self) -> Row {
        let rows = self.rows();
        rows.get(self.sel.min(rows.len() - 1)).copied().unwrap_or(Row::Mode)
    }

    /// The monthly choices for the anchor day, and the rule's own first when it is another.
    pub fn month_options(&self) -> Vec<MonthOption> {
        let Some(r) = &self.rule else {
            return Vec::new();
        };
        let mut out: Vec<MonthOption> = Vec::new();
        let mut add = |key: String, label: String, rule: CalendarRule| {
            if !out.iter().any(|o| o.key == key) {
                out.push(MonthOption { key, label, rule });
            }
        };
        let day = |d: u32| CalendarRule {
            month_day: Some(d),
            nth: None,
            weekday: None,
            ..r.clone()
        };
        let nth = |n: i32, w: &str| CalendarRule {
            month_day: None,
            nth: Some(n),
            weekday: Some(w.to_owned()),
            ..r.clone()
        };
        if let Some(d) = r.month_day {
            add(format!("d{d}"), format!("on day {d}"), day(d));
        }
        if let (Some(n), Some(w)) = (r.nth, r.weekday.as_deref()) {
            add(format!("n{n}{w}"), format!("on {}", nth_words(n, w)), nth(n, w));
        }
        if let Some(m) = iter_rule::month_choices(&self.anchor) {
            add(format!("d{}", m.day), format!("on day {}", m.day), day(m.day));
            add(
                format!("n{}{}", m.nth, m.weekday),
                format!("on {}", nth_words(m.nth, &m.weekday)),
                nth(m.nth, &m.weekday),
            );
            if m.last {
                add(
                    format!("n-1{}", m.weekday),
                    format!("on {}", nth_words(-1, &m.weekday)),
                    nth(-1, &m.weekday),
                );
            }
        }
        out
    }

    /// Which monthly choice the rule is.
    pub fn month_at(&self) -> usize {
        let Some(r) = &self.rule else {
            return 0;
        };
        let key = match (r.nth, r.weekday.as_deref()) {
            (Some(n), Some(w)) => format!("n{n}{w}"),
            _ => format!("d{}", r.month_day.unwrap_or(0)),
        };
        self.month_options().iter().position(|o| o.key == key).unwrap_or(0)
    }

    pub fn key(&mut self, key: KeyEvent) -> Outcome {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let n = self.rows().len();
        match key.code {
            KeyCode::Esc => return Outcome::Cancel,
            KeyCode::Enter => return Outcome::Done(self.value()),
            KeyCode::Down | KeyCode::Tab => self.sel = (self.sel + 1).min(n - 1),
            KeyCode::Char('n') if ctrl => self.sel = (self.sel + 1).min(n - 1),
            KeyCode::Up | KeyCode::BackTab => self.sel = self.sel.saturating_sub(1),
            KeyCode::Char('p') if ctrl => self.sel = self.sel.saturating_sub(1),
            _ => self.change(key.code, ctrl),
        }
        Outcome::Stay
    }

    fn change(&mut self, code: KeyCode, ctrl: bool) {
        let step: i64 = match code {
            KeyCode::Right | KeyCode::Char(' ' | '+') => 1,
            KeyCode::Left | KeyCode::Char('-') => -1,
            _ => 0,
        };
        match self.row() {
            Row::Mode if step != 0 => self.switch(),
            Row::Preset if step != 0 => {
                let at = RECUR_PRESETS.iter().position(|(v, _)| *v == self.typed.line.trim());
                let len = i64::try_from(RECUR_PRESETS.len()).unwrap_or(1);
                let next = match at {
                    Some(i) => (i64::try_from(i).unwrap_or(0) + step).rem_euclid(len),
                    None if step > 0 => 0,
                    None => len - 1,
                };
                if let Some((v, _)) = usize::try_from(next).ok().and_then(|i| RECUR_PRESETS.get(i)) {
                    self.typed = Input::new(v);
                }
            }
            Row::Typed => edit_input(&mut self.typed, code, ctrl),
            Row::Until => {
                edit_input(&mut self.until, code, ctrl);
                let day = self.until.line.trim().to_owned();
                self.edit(|r| r.until = Some(day));
            }
            Row::Every => self.edit(|r| r.interval = number(r.interval, code, step)),
            Row::Count => self.edit(|r| r.count = Some(number(r.count.unwrap_or(10), code, step))),
            Row::Unit if step != 0 => {
                let units = [Freq::Daily, Freq::Weekly, Freq::Monthly, Freq::Yearly];
                let Some(r) = &self.rule else {
                    return;
                };
                let at = units.iter().position(|u| *u == r.freq).unwrap_or(1);
                let freq = units[usize::try_from((i64::try_from(at).unwrap_or(1) + step).rem_euclid(4)).unwrap_or(1)];
                let days = if freq == Freq::Weekly && r.days.is_empty() {
                    iter_rule::default_for(&self.anchor).days
                } else if freq == Freq::Weekly {
                    r.days.clone()
                } else {
                    Vec::new()
                };
                let month_day = (freq == Freq::Monthly)
                    .then(|| iter_rule::month_choices(&self.anchor).map(|m| m.day))
                    .flatten();
                self.edit(|r| {
                    r.freq = freq;
                    r.days = days;
                    r.month_day = month_day;
                    r.nth = None;
                    r.weekday = None;
                });
            }
            Row::Days => match code {
                KeyCode::Left => self.day = self.day.saturating_sub(1),
                KeyCode::Right => self.day = (self.day + 1).min(6),
                KeyCode::Char(' ') => {
                    let d = WEEKDAYS[self.day];
                    self.edit(|r| {
                        let on = r.days.iter().any(|x| x == d);
                        // At least one day stays on: none is the start date's anyway.
                        if !(on && r.days.len() == 1) {
                            r.days = WEEKDAYS
                                .iter()
                                .filter(|x| if **x == d { !on } else { r.days.iter().any(|y| y == *x) })
                                .map(|x| (*x).to_owned())
                                .collect();
                        }
                    });
                }
                _ => {}
            },
            Row::Month if step != 0 => {
                let options = self.month_options();
                let len = i64::try_from(options.len()).unwrap_or(1).max(1);
                let next = (i64::try_from(self.month_at()).unwrap_or(0) + step).rem_euclid(len);
                if let Some(o) = usize::try_from(next).ok().and_then(|i| options.get(i)) {
                    self.rule = Some(o.rule.clone());
                }
            }
            Row::Ends if step != 0 => {
                let anchor = self.anchor.clone();
                let until = self.until.line.trim().to_owned();
                self.edit(|r| match (r.until.is_some(), r.count.is_some(), step > 0) {
                    (false, false, true) | (false, true, false) => {
                        r.until = Some(if until.is_empty() { anchor } else { until });
                        r.count = None;
                    }
                    (true, _, true) | (false, false, false) => {
                        r.until = None;
                        r.count = Some(10);
                    }
                    _ => {
                        r.until = None;
                        r.count = None;
                    }
                });
                if let Some(u) = self.rule.as_ref().and_then(|r| r.until.clone()) {
                    self.until = Input::new(&u);
                }
            }
            _ => {}
        }
    }

    /// From one mode to the other: into the Calendar mode with the rule typed when it can show it, else a new one from
    /// the anchor day; back to what was typed.
    fn switch(&mut self) {
        self.calendar = !self.calendar;
        if self.calendar && self.rule.is_none() {
            self.rule =
                Some(iter_rule::from_rrule(&self.typed.line).unwrap_or_else(|| iter_rule::default_for(&self.anchor)));
        }
        self.day = self
            .rule
            .as_ref()
            .and_then(|r| r.days.first())
            .and_then(|d| WEEKDAYS.iter().position(|x| x == d))
            .unwrap_or(0);
    }

    fn edit(&mut self, change: impl FnOnce(&mut CalendarRule)) {
        if let Some(r) = self.rule.as_mut() {
            change(r);
        }
    }
}

/// A number typed or stepped: a digit is added, Backspace takes the last off, ← and → step; never below 1.
fn number(n: u32, code: KeyCode, step: i64) -> u32 {
    let next = match code {
        KeyCode::Char(c) if c.is_ascii_digit() => n
            .saturating_mul(10)
            .saturating_add(c.to_digit(10).unwrap_or(0))
            .min(999),
        KeyCode::Backspace => n / 10,
        _ => u32::try_from(i64::from(n) + step).unwrap_or(1),
    };
    next.max(1)
}

fn edit_input(i: &mut Input, code: KeyCode, ctrl: bool) {
    match code {
        KeyCode::Char('a') if ctrl => i.home(),
        KeyCode::Char('e') if ctrl => i.end(),
        KeyCode::Char('w') if ctrl => i.delete_word(),
        KeyCode::Char('u') if ctrl => *i = Input::default(),
        KeyCode::Char(c) if !ctrl => i.insert(c),
        KeyCode::Backspace => i.backspace(),
        KeyCode::Delete => i.delete(),
        KeyCode::Left => i.left(),
        KeyCode::Right => i.right(),
        KeyCode::Home => i.home(),
        KeyCode::End => i.end(),
        _ => {}
    }
}

/// "the fourth Friday", "the last Friday".
pub fn nth_words(nth: i32, weekday: &str) -> String {
    const NAMES: [&str; 7] = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    const ORDINALS: [&str; 4] = ["first", "second", "third", "fourth"];
    let name = WEEKDAYS
        .iter()
        .position(|x| *x == weekday)
        .map_or(weekday, |i| NAMES[i]);
    let which = if nth == -1 {
        "last"
    } else {
        usize::try_from(nth - 1)
            .ok()
            .and_then(|i| ORDINALS.get(i))
            .copied()
            .unwrap_or("first")
    };
    format!("the {which} {name}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(p: &mut IterPick, codes: &[KeyCode]) {
        for c in codes {
            p.key(KeyEvent::new(*c, KeyModifiers::NONE));
        }
    }

    fn picker(value: &str) -> IterPick {
        let cfg = tc_core::taskrc::parse("").config;
        IterPick::new(
            Form::add(&tc_core::preview::Preview::default(), &cfg),
            value,
            "2026-12-25",
        )
    }

    #[test]
    fn a_period_opens_in_the_taskwarrior_mode_and_the_arrows_choose_another() {
        let mut p = picker("monthly");
        assert!(!p.calendar);
        assert_eq!(p.rows(), [Row::Mode, Row::Preset, Row::Typed]);
        press(&mut p, &[KeyCode::Down, KeyCode::Right]);
        assert_eq!(p.value(), "quarterly");
        press(
            &mut p,
            &[KeyCode::Down, KeyCode::End, KeyCode::Backspace, KeyCode::Backspace],
        );
        assert_eq!(p.value(), "quarter");
        assert_eq!(p.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)), Outcome::Cancel);
    }

    #[test]
    fn the_calendar_mode_builds_a_rule_as_a_calendar_app_does() {
        let mut p = picker("");
        press(&mut p, &[KeyCode::Char(' ')]);
        // Every week on the anchor's weekday (2026-12-25 is a Friday).
        assert_eq!(p.value(), "FREQ=WEEKLY;BYDAY=FR");
        assert_eq!(p.rows(), [Row::Mode, Row::Every, Row::Unit, Row::Days, Row::Ends]);
        // Every 2 weeks, on Monday too.
        press(&mut p, &[KeyCode::Down, KeyCode::Right, KeyCode::Down, KeyCode::Down]);
        assert_eq!(p.day, 4, "the cursor starts on the rule's first day");
        press(
            &mut p,
            &[
                KeyCode::Left,
                KeyCode::Left,
                KeyCode::Left,
                KeyCode::Left,
                KeyCode::Char(' '),
            ],
        );
        assert_eq!(p.value(), "FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR");
        // Monthly, on the last Friday, ten times.
        press(&mut p, &[KeyCode::Up, KeyCode::Right, KeyCode::Down]);
        assert_eq!(p.value(), "FREQ=MONTHLY;INTERVAL=2;BYMONTHDAY=25");
        let labels: Vec<String> = p.month_options().into_iter().map(|o| o.label).collect();
        assert_eq!(labels, ["on day 25", "on the fourth Friday", "on the last Friday"]);
        press(&mut p, &[KeyCode::Left, KeyCode::Down, KeyCode::Left]);
        assert_eq!(p.value(), "FREQ=MONTHLY;INTERVAL=2;BYDAY=-1FR;COUNT=10");
        assert_eq!(p.words(), "Every 2 months on the last Friday, 10 times");
        // Ending on a day instead, typed.
        press(&mut p, &[KeyCode::Left, KeyCode::Down]);
        assert_eq!(p.row(), Row::Until);
        for _ in 0..10 {
            press(&mut p, &[KeyCode::Backspace]);
        }
        for c in "2027-06-30".chars() {
            press(&mut p, &[KeyCode::Char(c)]);
        }
        assert_eq!(
            p.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Outcome::Done("FREQ=MONTHLY;INTERVAL=2;BYDAY=-1FR;UNTIL=20270630".into())
        );
    }

    #[test]
    fn a_calendar_rule_opens_in_its_mode_and_switching_back_keeps_what_was_typed() {
        let p = picker("FREQ=WEEKLY;BYDAY=TU,TH");
        assert!(p.calendar);
        assert_eq!(
            p.rule.as_ref().map(|r| r.days.clone()),
            Some(vec!["TU".into(), "TH".into()])
        );
        let mut q = picker("3d");
        press(&mut q, &[KeyCode::Right]);
        assert_eq!(q.value(), "FREQ=WEEKLY;BYDAY=FR");
        press(&mut q, &[KeyCode::Left]);
        assert_eq!(q.value(), "3d");
    }
}
