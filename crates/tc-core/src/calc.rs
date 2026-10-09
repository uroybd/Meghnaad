//! `calc`: Taskwarrior's expression calculator (`Eval` and `Variant`), over integers, reals,
//! booleans, strings, dates and durations. `expressions=postfix` reads `1 2 +` instead of `1 + 2`.
//!
//! The evaluator is Taskwarrior's, step for step: the same grammar, the same shunting-yard, the
//! same operator table and the same per-type rules, so its habits come out the same too (integer
//! division truncates, `2 ^ -1` can't be evaluated, `1y` is 365 days). References into Taskwarrior's
//! "DOM" resolve too (`1.due`, `rc.bulk`, `system.version`; see [`DomSource`]). Not supported: the
//! `~` and `!~` match operators, which Taskwarrior itself only has with a task in hand.

use crate::dates::{parse_date, Clock};
use crate::model::Facts;
use crate::taskrc::{Config, UdaType};
use std::collections::BTreeMap;
use taskchampion::chrono::Datelike;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
enum V {
    Bool(bool),
    Int(i64),
    Real(f64),
    /// A string as it was typed, quotes included (Taskwarrior keeps them).
    Str(String),
    Date(i64),
    Dur(i64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum T {
    Op,
    Number,
    Str,
    Date,
    Dur,
    Ident,
}

type Tok = (String, T);

/// What a reference into the DOM holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Dom {
    Int(i64),
    Real(f64),
    Text(String),
    Date(i64),
    Dur(i64),
}

/// Looks a DOM reference up; `None` means it isn't one, and it is then just a word.
pub type DomFn<'a> = &'a dyn Fn(&str) -> Option<Dom>;

const DAY: i64 = 86_400;

/// Seconds in each duration unit Taskwarrior knows (`Duration.cpp`).
const UNITS: &[(&str, i64)] = &[
    ("annual", 365 * DAY), ("biannual", 730 * DAY), ("bimonthly", 61 * DAY), ("biweekly", 14 * DAY),
    ("biyearly", 730 * DAY), ("daily", DAY), ("days", DAY), ("day", DAY), ("d", DAY),
    ("fortnight", 14 * DAY), ("hours", 3600), ("hour", 3600), ("hrs", 3600), ("hr", 3600), ("h", 3600),
    ("minutes", 60), ("minute", 60), ("mins", 60), ("min", 60), ("monthly", 30 * DAY),
    ("months", 30 * DAY), ("month", 30 * DAY), ("mnths", 30 * DAY), ("mths", 30 * DAY),
    ("mth", 30 * DAY), ("mos", 30 * DAY), ("mo", 30 * DAY), ("m", 30 * DAY), ("quarterly", 91 * DAY),
    ("quarters", 91 * DAY), ("quarter", 91 * DAY), ("qrtrs", 91 * DAY), ("qrtr", 91 * DAY),
    ("qtrs", 91 * DAY), ("qtr", 91 * DAY), ("q", 91 * DAY), ("semiannual", 183 * DAY),
    ("sennight", 14 * DAY), ("seconds", 1), ("second", 1), ("secs", 1), ("sec", 1), ("s", 1),
    ("weekdays", DAY), ("weekly", 7 * DAY), ("weeks", 7 * DAY), ("week", 7 * DAY), ("wks", 7 * DAY),
    ("wk", 7 * DAY), ("w", 7 * DAY), ("yearly", 365 * DAY), ("years", 365 * DAY), ("year", 365 * DAY),
    ("yrs", 365 * DAY), ("yr", 365 * DAY), ("y", 365 * DAY),
];

/// Words that name a date by themselves.
const NAMED_DATES: &[&str] = &[
    "now", "today", "yesterday", "tomorrow", "sod", "eod", "sow", "eow", "som", "eom", "soy", "eoy",
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
];

// ---------------------------------------------------------------------------------------------
// Lexing

fn is_boundary(rest: &[u8]) -> bool {
    rest.first().map_or(true, |c| !(c.is_ascii_alphanumeric() || *c == b'_'))
}

/// The ISO date (and date-time) at the start of `s`, and its length.
fn iso_date(s: &[u8]) -> Option<usize> {
    let digits = |from: usize, n: usize| s.get(from..from + n).is_some_and(|p| p.iter().all(u8::is_ascii_digit));
    if digits(0, 4) && s.get(4) == Some(&b'-') && digits(5, 2) && s.get(7) == Some(&b'-') && digits(8, 2) {
        let mut n = 10;
        if s.get(n) == Some(&b'T') && digits(n + 1, 2) && s.get(n + 3) == Some(&b':') && digits(n + 4, 2) {
            n += 6;
            if s.get(n) == Some(&b':') && digits(n + 1, 2) {
                n += 3;
            }
            if s.get(n) == Some(&b'Z') {
                n += 1;
            }
        }
        return is_boundary(&s[n..]).then_some(n);
    }
    // 20261225T100000(Z)
    if digits(0, 8) && s.get(8) == Some(&b'T') && digits(9, 6) {
        let n = if s.get(15) == Some(&b'Z') { 16 } else { 15 };
        return is_boundary(&s[n..]).then_some(n);
    }
    None
}

/// An ISO 8601 duration (`P1D`, `PT90M`, `P1Y2M3DT4H5M6S`) at the start of `s`, in seconds.
fn iso_duration(s: &[u8]) -> Option<(usize, i64)> {
    if s.first() != Some(&b'P') {
        return None;
    }
    let (mut at, mut total, mut time, mut any) = (1usize, 0i64, false, false);
    while at < s.len() {
        if s[at] == b'T' {
            if time {
                return None;
            }
            time = true;
            at += 1;
            continue;
        }
        let start = at;
        while at < s.len() && s[at].is_ascii_digit() {
            at += 1;
        }
        if at == start || at >= s.len() {
            break;
        }
        let n: i64 = std::str::from_utf8(&s[start..at]).ok()?.parse().ok()?;
        let unit = match (s[at], time) {
            (b'Y', false) => 365 * DAY,
            (b'M', false) => 30 * DAY,
            (b'W', false) => 7 * DAY,
            (b'D', false) => DAY,
            (b'H', true) => 3600,
            (b'M', true) => 60,
            (b'S', true) => 1,
            _ => return None,
        };
        total += n * unit;
        any = true;
        at += 1;
    }
    (any && is_boundary(&s[at..])).then_some((at, total))
}

/// A reference such as `1.due` or `3d978566.description` at the start of `s`, and its length.
fn dom_ref(s: &[u8]) -> Option<usize> {
    let mut j = 0;
    while j < s.len() && (s[j].is_ascii_hexdigit() || s[j] == b'-') {
        j += 1;
    }
    // Whatever looks like `<hex>.<word>` is one word; whether it names a task is for the lookup to
    // say (a uuid needs eight characters), and when it doesn't it is just text.
    let is_ref = j > 0 && s.get(j) == Some(&b'.') && s.get(j + 1).is_some_and(u8::is_ascii_alphabetic);
    if !is_ref {
        return None;
    }
    let mut end = j + 1;
    while end < s.len() && (s[end].is_ascii_alphanumeric() || s[end] == b'_' || s[end] == b'.') {
        end += 1;
    }
    Some(end)
}

fn tokenize(src: &str) -> Vec<Tok> {
    let b = src.as_bytes();
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let rest = &b[i..];
        // A quoted string, quotes kept.
        if c == b'\'' || c == b'"' {
            if let Some(end) = rest[1..].iter().position(|x| *x == c) {
                out.push((src[i..i + end + 2].to_owned(), T::Str));
                i += end + 2;
                continue;
            }
        }
        // `1.due`, `3d978566.description`: a task by id or uuid (full, or the start of one), then
        // what to read from it. Tried before the number and duration forms, which would take `3d`.
        if let Some(n) = dom_ref(rest) {
            out.push((src[i..i + n].to_owned(), T::Ident));
            i += n;
            continue;
        }
        // Dates, then durations, then numbers: the order Taskwarrior's lexer tries them in.
        if let Some(n) = iso_date(rest) {
            out.push((src[i..i + n].to_owned(), T::Date));
            i += n;
            continue;
        }
        if let Some((n, _)) = iso_duration(rest) {
            out.push((src[i..i + n].to_owned(), T::Dur));
            i += n;
            continue;
        }
        if c.is_ascii_digit() || (c == b'.' && rest.get(1).is_some_and(u8::is_ascii_digit)) {
            let mut j = 0;
            while j < rest.len() && rest[j].is_ascii_digit() {
                j += 1;
            }
            let mut real = false;
            if rest.get(j) == Some(&b'.') && rest.get(j + 1).is_some_and(u8::is_ascii_digit) {
                real = true;
                j += 1;
                while j < rest.len() && rest[j].is_ascii_digit() {
                    j += 1;
                }
            }
            // A number and a unit, with or without a space between, is a duration.
            let mut k = j;
            while rest.get(k) == Some(&b' ') {
                k += 1;
            }
            let unit = UNITS
                .iter()
                .filter(|(u, _)| rest[k..].starts_with(u.as_bytes()) && is_boundary(&rest[k + u.len()..]))
                .max_by_key(|(u, _)| u.len());
            if let Some((u, _)) = unit {
                let end = k + u.len();
                out.push((src[i..i + end].to_owned(), T::Dur));
                i += end;
                continue;
            }
            let text = &src[i..i + j];
            // Nine digits or more are an epoch time (Taskwarrior reads them as dates).
            let kind = if !real && j >= 9 { T::Date } else { T::Number };
            out.push((text.to_owned(), kind));
            i += j;
            continue;
        }
        // Operators, longest first.
        let ops = ["!==", "==", "!=", "<=", ">=", "!~", "&&", "||", "^", "!", "*", "/", "%", "+", "-", "<", ">", "=", "~", "(", ")"];
        if let Some(op) = ops.iter().find(|o| rest.starts_with(o.as_bytes())) {
            out.push(((*op).to_owned(), T::Op));
            i += op.len();
            continue;
        }
        // Words: `and`, `or`, `xor`, named dates, constants, anything else is a string.
        if c.is_ascii_alphabetic() || c == b'_' {
            let mut j = 0;
            while j < rest.len() && (rest[j].is_ascii_alphanumeric() || rest[j] == b'_' || rest[j] == b'.') {
                j += 1;
            }
            let word = &src[i..i + j];
            let kind = match word {
                "and" | "or" | "xor" => T::Op,
                w if NAMED_DATES.contains(&w) => T::Date,
                _ => T::Ident,
            };
            out.push((word.to_owned(), kind));
            i += j;
            continue;
        }
        // Anything else is a word of its own.
        let ch = src[i..].chars().next().map_or(1, char::len_utf8);
        out.push((src[i..i + ch].to_owned(), T::Ident));
        i += ch;
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Parsing: Taskwarrior's `Eval::infixParse`, which marks unary operators, and the shunting-yard

/// Precedence, and whether it is right-associative, for an operator.
fn operator(op: &str) -> Option<(u32, bool)> {
    Some(match op {
        "^" => (16, true),
        "!" | "_neg_" | "_pos_" => (15, true),
        "_hastag_" | "_notag_" => (14, false),
        "*" | "/" | "%" => (13, false),
        "+" | "-" => (12, false),
        "<=" | ">=" | ">" | "<" => (10, false),
        "=" | "==" | "!=" | "!==" => (9, false),
        "~" | "!~" => (8, false),
        "and" => (5, false),
        "or" => (4, false),
        "xor" => (3, false),
        "(" | ")" => (0, false),
        _ => return None,
    })
}

struct Parse<'a>(&'a mut Vec<Tok>, usize);

impl Parse<'_> {
    fn is_op(&self, ops: &[&str]) -> bool {
        self.0.get(self.1).is_some_and(|(t, k)| *k == T::Op && ops.contains(&t.as_str()))
    }

    /// Logical --> Regex {( "and" | "or" | "xor" ) Regex}, and so on down the precedence table.
    fn logical(&mut self) -> bool {
        self.chain(Self::regex, &["and", "or", "xor"])
    }
    fn regex(&mut self) -> bool {
        self.chain(Self::equality, &["~", "!~"])
    }
    fn equality(&mut self) -> bool {
        self.chain(Self::comparative, &["==", "=", "!==", "!="])
    }
    fn comparative(&mut self) -> bool {
        self.chain(Self::arithmetic, &["<=", "<", ">=", ">"])
    }
    fn arithmetic(&mut self) -> bool {
        self.chain(Self::geometric, &["+", "-"])
    }
    fn geometric(&mut self) -> bool {
        self.chain(Self::tag, &["*", "/", "%"])
    }
    fn tag(&mut self) -> bool {
        self.chain(Self::unary, &["_hastag_", "_notag_"])
    }

    fn chain(&mut self, next: fn(&mut Self) -> bool, ops: &[&str]) -> bool {
        if self.1 < self.0.len() && next(self) {
            while self.is_op(ops) {
                self.1 += 1;
                if !next(self) {
                    return false;
                }
            }
            return true;
        }
        false
    }

    /// Unary --> [( "-" | "+" | "!" )] Exponent: where a prefix `-` or `+` is read as one.
    fn unary(&mut self) -> bool {
        if let Some((t, _)) = self.0.get_mut(self.1) {
            match t.as_str() {
                "-" => {
                    *t = "_neg_".into();
                    self.1 += 1;
                }
                "+" => {
                    *t = "_pos_".into();
                    self.1 += 1;
                }
                "!" => self.1 += 1,
                _ => {}
            }
        }
        self.exponent()
    }

    fn exponent(&mut self) -> bool {
        if self.1 < self.0.len() && self.primitive() {
            while self.is_op(&["^"]) {
                self.1 += 1;
                if !self.primitive() {
                    return false;
                }
            }
            return true;
        }
        false
    }

    fn primitive(&mut self) -> bool {
        let Some((t, k)) = self.0.get(self.1) else { return false };
        if t == "(" {
            self.1 += 1;
            if self.1 < self.0.len() && self.logical() && self.0.get(self.1).is_some_and(|(t, _)| t == ")") {
                self.1 += 1;
                return true;
            }
            return false;
        }
        // The named constants are values; any other operator is not.
        if *k != T::Op || matches!(t.as_str(), "true" | "false" | "pi") {
            self.1 += 1;
            return true;
        }
        false
    }
}

fn to_postfix(infix: Vec<Tok>) -> Result<Vec<Tok>, String> {
    if infix.len() == 1 {
        return Ok(infix);
    }
    let mut out: Vec<Tok> = Vec::new();
    let mut stack: Vec<Tok> = Vec::new();
    for tok in infix {
        if tok.1 == T::Op && tok.0 == "(" {
            stack.push(tok);
        } else if tok.1 == T::Op && tok.0 == ")" {
            while stack.last().is_some_and(|t| t.0 != "(") {
                out.extend(stack.pop());
            }
            if stack.pop().is_none() {
                return Err("Mismatched parentheses in expression".into());
            }
        } else if let (T::Op, Some((prec, right))) = (tok.1, operator(&tok.0)) {
            while let Some((prec2, _)) = stack.last().and_then(|t| operator(&t.0)) {
                if (!right && prec <= prec2) || (right && prec < prec2) {
                    out.extend(stack.pop());
                } else {
                    break;
                }
            }
            stack.push(tok);
        } else {
            out.push(tok);
        }
    }
    while let Some(t) = stack.pop() {
        if t.0 == "(" || t.0 == ")" {
            return Err("Mismatched parentheses in expression".into());
        }
        out.push(t);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Values

fn dequote(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 && (b[0] == b'\'' || b[0] == b'"') && b[b.len() - 1] == b[0] {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// C++'s default `double` output (`%g`): six significant digits, no trailing zeros.
fn fmt_real(x: f64) -> String {
    if x == 0.0 {
        return "0".into();
    }
    if !x.is_finite() {
        return if x.is_nan() { "nan".into() } else if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let sci = format!("{x:.5e}");
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let trim = |s: &str| -> String {
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_owned() } else { s.to_owned() }
    };
    if !(-4..6).contains(&exp) {
        format!("{}e{}{:02}", trim(mant), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        trim(&format!("{:.*}", (5 - exp).max(0) as usize, x))
    }
}

/// `Duration::formatISO`.
fn fmt_duration(secs: i64) -> String {
    if secs == 0 {
        return "PT0S".into();
    }
    let sign = if secs < 0 { "-" } else { "" };
    let t = secs.abs();
    let (s, m, h, d) = (t % 60, t / 60 % 60, t / 3600 % 24, t / DAY);
    let mut out = format!("{sign}P");
    if d > 0 {
        out += &format!("{d}D");
    }
    if h > 0 || m > 0 || s > 0 {
        out.push('T');
        if h > 0 {
            out += &format!("{h}H");
        }
        if m > 0 {
            out += &format!("{m}M");
        }
        if s > 0 {
            out += &format!("{s}S");
        }
    }
    out
}

fn fmt_date(ts: i64, clock: &Clock) -> String {
    let (y, mo, d, h, mi, s) = clock.ymd_hms(ts);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}")
}

fn show(v: &V, clock: &Clock) -> String {
    match v {
        V::Bool(b) => b.to_string(),
        V::Int(i) => i.to_string(),
        V::Real(r) => fmt_real(*r),
        V::Str(s) => s.clone(),
        V::Date(d) => fmt_date(*d, clock),
        V::Dur(d) => fmt_duration(*d),
    }
}

fn from_dom(d: Dom) -> V {
    match d {
        Dom::Int(i) => V::Int(i),
        Dom::Real(r) => V::Real(r),
        Dom::Text(t) => V::Str(t),
        Dom::Date(d) => V::Date(d),
        Dom::Dur(d) => V::Dur(d),
    }
}

fn literal(tok: &Tok, clock: &Clock, dom: DomFn) -> Result<V, String> {
    let (text, kind) = tok;
    Ok(match kind {
        T::Op => return Err("Operator expected.".into()),
        T::Number => {
            if text.bytes().all(|c| c.is_ascii_digit()) {
                V::Int(text.parse().map_err(|_| "The expression could not be evaluated.".to_owned())?)
            } else {
                V::Real(text.parse().map_err(|_| "The expression could not be evaluated.".to_owned())?)
            }
        }
        T::Str => V::Str(text.clone()),
        T::Date => V::Date(
            parse_date(text, clock).map(|d| d.ts).ok_or_else(|| "The expression could not be evaluated.".to_owned())?,
        ),
        T::Dur => V::Dur(duration_of(text).ok_or_else(|| "The expression could not be evaluated.".to_owned())?),
        // Named constants first, then the DOM; a word that is neither is a string.
        T::Ident => match text.as_str() {
            "true" => V::Bool(true),
            "false" => V::Bool(false),
            "pi" => V::Real(3.14159165),
            _ => dom(text).map_or_else(|| V::Str(text.clone()), from_dom),
        },
    })
}

/// A duration literal in seconds: ISO, or a number and a unit.
fn duration_of(text: &str) -> Option<i64> {
    if let Some((_, secs)) = iso_duration(text.as_bytes()) {
        return Some(secs);
    }
    let split = text.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let n: f64 = text[..split].parse().ok()?;
    let unit = text[split..].trim();
    let (_, per) = UNITS.iter().find(|(u, _)| *u == unit)?;
    Some((n * *per as f64) as i64)
}

// ---------------------------------------------------------------------------------------------
// Operators, per type, as `Variant` has them

type R = Result<V, String>;

fn err<X>(m: &str) -> Result<X, String> {
    Err(m.to_owned())
}

fn truthy(v: &V) -> bool {
    match v {
        V::Bool(b) => *b,
        V::Int(i) => *i != 0,
        V::Real(r) => *r != 0.0,
        V::Str(s) => !dequote(s).is_empty(),
        V::Date(d) | V::Dur(d) => *d != 0,
    }
}

fn as_int(v: &V) -> i64 {
    match v {
        V::Bool(b) => i64::from(*b),
        V::Int(i) | V::Date(i) | V::Dur(i) => *i,
        V::Real(r) => *r as i64,
        V::Str(s) => dequote(s).trim().parse().unwrap_or(0),
    }
}

fn as_real(v: &V) -> f64 {
    match v {
        V::Real(r) => *r,
        V::Str(s) => dequote(s).trim().parse().unwrap_or(0.0),
        other => as_int(other) as f64,
    }
}

fn as_text(v: &V, clock: &Clock) -> String {
    show(v, clock)
}

/// How two values compare (`<`, `==`, ...): both brought to a common type the way `Variant` does.
/// `None` is "can't say", which compares false.
fn compare(l: &V, r: &V, clock: &Clock) -> Option<std::cmp::Ordering> {
    use std::cmp::Ordering;
    let norm = |v: &V| match v {
        V::Str(s) => V::Str(dequote(s).to_owned()),
        other => other.clone(),
    };
    let (l, r) = (norm(l), norm(r));
    let real = |a: f64, b: f64| a.partial_cmp(&b);
    Some(match (&l, &r) {
        (V::Bool(a), V::Bool(b)) => a.cmp(b),
        (V::Int(a), V::Int(b)) => a.cmp(b),
        (V::Bool(_) | V::Int(_) | V::Real(_), V::Bool(_) | V::Int(_) | V::Real(_)) => real(as_real(&l), as_real(&r))?,
        (V::Str(a), V::Str(b)) => a.cmp(b),
        (V::Str(_), V::Bool(_) | V::Int(_) | V::Real(_)) | (V::Bool(_) | V::Int(_) | V::Real(_), V::Str(_)) => {
            as_text(&l, clock).cmp(&as_text(&r, clock))
        }
        // Dates and durations: the other side is read as the same kind.
        (V::Date(a), other) => a.cmp(&to_date(other, clock)?),
        (other, V::Date(b)) if !matches!(other, V::Dur(_)) => to_date(other, clock)?.cmp(b),
        (V::Dur(a), other) => a.cmp(&to_dur(other, clock)?),
        (other, V::Dur(b)) => to_dur(other, clock)?.cmp(b),
        _ => return None::<Ordering>,
    })
}

fn to_date(v: &V, clock: &Clock) -> Option<i64> {
    match v {
        V::Str(s) => parse_date(dequote(s), clock).map(|d| d.ts),
        other => Some(as_int(other)),
    }
}

fn to_dur(v: &V, _clock: &Clock) -> Option<i64> {
    match v {
        V::Str(s) => duration_of(dequote(s)),
        other => Some(as_int(other)),
    }
}

fn add(l: &V, r: &V, clock: &Clock) -> R {
    let rs = |v: &V| match v {
        V::Str(s) => dequote(s).to_owned(),
        other => as_text(other, clock),
    };
    Ok(match (l, r) {
        (V::Bool(_), V::Bool(_)) => return err("Cannot add two Boolean values"),
        (V::Bool(a), V::Int(b)) | (V::Int(b), V::Bool(a)) => V::Int(i64::from(*a) + b),
        (V::Bool(a), V::Real(b)) => V::Real(f64::from(u8::from(*a)) + b),
        (V::Bool(_), V::Str(_)) => V::Str(format!("{}{}", as_text(l, clock), rs(r))),
        (V::Bool(a), V::Date(b)) => V::Date(i64::from(*a) + b),
        (V::Bool(a), V::Dur(b)) => V::Dur(i64::from(*a) + b),
        (V::Int(a), V::Int(b)) => V::Int(a + b),
        (V::Int(a), V::Real(b)) => V::Real(*a as f64 + b),
        (V::Int(_), V::Str(_)) => V::Str(format!("{}{}", as_text(l, clock), rs(r))),
        (V::Int(a), V::Date(b)) => V::Date(a + b),
        (V::Int(a), V::Dur(b)) => V::Dur(a + b),
        (V::Real(a), V::Bool(_) | V::Int(_) | V::Real(_)) => V::Real(a + as_real(r)),
        (V::Real(_), V::Str(_)) => V::Str(format!("{}{}", as_text(l, clock), rs(r))),
        (V::Real(a), V::Date(b)) => V::Date(*a as i64 + b),
        (V::Real(a), V::Dur(b)) => V::Dur(*a as i64 + b),
        (V::Str(a), other) => V::Str(format!("{a}{}", rs(other))),
        (V::Date(a), V::Bool(_) | V::Int(_) | V::Real(_)) => V::Date(a + as_int(r)),
        (V::Date(_), V::Str(_)) => V::Str(format!("{}{}", as_text(l, clock), rs(r))),
        (V::Date(_), V::Date(_)) => return err("Cannot add two date values"),
        (V::Date(a), V::Dur(b)) | (V::Dur(b), V::Date(a)) => V::Date(a + b),
        (V::Dur(a), V::Bool(_) | V::Int(_) | V::Real(_)) => V::Dur(a + as_int(r)),
        (V::Dur(_), V::Str(_)) => V::Str(format!("{}{}", as_text(l, clock), rs(r))),
        (V::Dur(a), V::Dur(b)) => V::Dur(a + b),
    })
}

fn sub(l: &V, r: &V) -> R {
    Ok(match (l, r) {
        (V::Bool(_), _) => return err("Cannot subtract from a Boolean value"),
        (V::Int(a), V::Bool(_) | V::Int(_)) => V::Int(a - as_int(r)),
        (V::Int(a), V::Real(b)) => V::Real(*a as f64 - b),
        (V::Int(_), V::Str(_)) => return err("Cannot subtract strings"),
        (V::Int(a), V::Date(b)) => V::Date(a - b),
        (V::Int(a), V::Dur(b)) => V::Dur(a - b),
        (V::Real(_), V::Str(_)) => return err("Cannot subtract strings"),
        (V::Real(a), other) => V::Real(a - as_real(other)),
        (V::Str(a), V::Str(b)) => V::Str(format!("{a}-{b}")),
        (V::Str(_), _) => return err("Cannot subtract strings"),
        (V::Date(a), V::Bool(_) | V::Int(_) | V::Real(_)) => V::Date(a - as_int(r)),
        (V::Date(_), V::Str(_)) => return err("Cannot subtract strings"),
        (V::Date(a), V::Date(b)) => V::Dur(a - b),
        (V::Date(a), V::Dur(b)) => V::Date(a - b),
        (V::Dur(a), V::Bool(_) | V::Int(_) | V::Real(_)) => V::Dur(a - as_int(r)),
        (V::Dur(_), V::Str(_)) => return err("Cannot subtract strings"),
        (V::Dur(_), V::Date(_)) => return err("Cannot subtract a date"),
        (V::Dur(a), V::Dur(b)) => V::Dur(a - b),
    })
}

fn mul(l: &V, r: &V) -> R {
    let rs = |v: &V| match v {
        V::Str(s) => dequote(s).to_owned(),
        _ => String::new(),
    };
    Ok(match (l, r) {
        (V::Bool(_), V::Bool(_)) => return err("Cannot multiply Boolean values"),
        (V::Bool(a), V::Int(b)) => V::Int(i64::from(*a) * b),
        (V::Bool(a), V::Real(b)) => V::Real(f64::from(u8::from(*a)) * b),
        (V::Bool(a), V::Str(_)) => V::Str(if *a { rs(r) } else { String::new() }),
        (V::Bool(a), V::Dur(b)) => V::Dur(i64::from(*a) * b),
        (V::Bool(_), V::Date(_)) | (V::Int(_), V::Date(_)) | (V::Real(_), V::Date(_)) | (V::Date(_), _) => {
            return err("Cannot multiply date values")
        }
        (V::Int(a), V::Bool(_) | V::Int(_)) => V::Int(a * as_int(r)),
        (V::Int(a), V::Real(b)) => V::Real(*a as f64 * b),
        (V::Int(a), V::Str(_)) => V::Str(rs(r).repeat((*a).max(0) as usize)),
        (V::Int(a), V::Dur(b)) => V::Dur(a * b),
        (V::Real(a), V::Bool(_) | V::Int(_) | V::Real(_)) => V::Real(a * as_real(r)),
        (V::Real(_), V::Str(_)) => return err("Cannot multiply real numbers by strings"),
        (V::Real(a), V::Dur(b)) => V::Dur((a * *b as f64) as i64),
        (V::Str(a), V::Bool(b)) => V::Str(if *b { a.clone() } else { String::new() }),
        (V::Str(a), V::Int(b)) => V::Str(a.repeat((*b).max(1) as usize)),
        (V::Str(_), V::Real(_)) => return err("Cannot multiply strings by real numbers"),
        (V::Str(_), V::Str(_)) => return err("Cannot multiply strings by strings"),
        (V::Str(_), V::Date(_)) => return err("Cannot multiply strings by dates"),
        (V::Str(_), V::Dur(_)) => return err("Cannot multiply strings by durations"),
        (V::Dur(a), V::Bool(_) | V::Int(_)) => V::Dur(a * as_int(r)),
        (V::Dur(a), V::Real(b)) => V::Dur((*a as f64 * b) as i64),
        (V::Dur(_), V::Str(_)) => return err("Cannot multiply durations by strings"),
        (V::Dur(_), V::Date(_)) => return err("Cannot multiply durations by dates"),
        (V::Dur(_), V::Dur(_)) => return err("Cannot multiply durations by durations"),
    })
}

fn div(l: &V, r: &V) -> R {
    let zero = || err::<V>("Cannot divide by zero");
    Ok(match (l, r) {
        (V::Bool(_), _) => return err("Cannot divide Boolean values"),
        (V::Int(_), V::Bool(_)) => return err("Cannot divide integers by Boolean values"),
        (V::Int(a), V::Int(b)) => {
            if *b == 0 {
                return zero();
            }
            V::Int(a / b)
        }
        (V::Int(a), V::Real(b)) => {
            if *b == 0.0 {
                return zero();
            }
            V::Real(*a as f64 / b)
        }
        (V::Int(_), V::Str(_)) => return err("Cannot divide integer by string"),
        (V::Int(_), V::Date(_)) => return err("Cannot divide integer by date values"),
        (V::Int(a), V::Dur(b)) => {
            if *b == 0 {
                return zero();
            }
            V::Dur(a / b)
        }
        (V::Real(_), V::Bool(_)) => return err("Cannot divide real by Boolean"),
        (V::Real(a), V::Int(_) | V::Real(_)) => {
            let d = as_real(r);
            if d == 0.0 {
                return zero();
            }
            V::Real(a / d)
        }
        (V::Real(_), V::Str(_)) => return err("Cannot divide real numbers by strings"),
        (V::Real(_), V::Date(_)) => return err("Cannot divide real numbers by dates"),
        (V::Real(a), V::Dur(b)) => {
            if *b == 0 {
                return zero();
            }
            V::Dur((a / *b as f64) as i64)
        }
        (V::Str(_), _) => return err("Cannot divide real numbers by strings"),
        (V::Date(_), _) => return err("Cannot divide real numbers by dates"),
        (V::Dur(_), V::Bool(_)) => return err("Cannot divide duration by Boolean"),
        (V::Dur(a), V::Int(b)) => {
            if *b == 0 {
                return zero();
            }
            V::Dur(a / b)
        }
        (V::Dur(a), V::Real(b)) => {
            if *b == 0.0 {
                return zero();
            }
            V::Dur((*a as f64 / b) as i64)
        }
        (V::Dur(_), V::Str(_)) => return err("Cannot divide durations by strings"),
        (V::Dur(_), V::Date(_)) => return err("Cannot divide durations by dates"),
        (V::Dur(a), V::Dur(b)) => V::Real(*a as f64 / *b as f64),
    })
}

fn modulo(l: &V, r: &V) -> R {
    let zero = || err::<V>("Cannot modulo zero");
    Ok(match (l, r) {
        (V::Bool(_), _) => return err("Cannot modulo Booleans"),
        (V::Int(_), V::Bool(_)) => return err("Cannot modulo integer by Boolean"),
        (V::Int(a), V::Int(b)) => {
            if *b == 0 {
                return zero();
            }
            V::Int(a % b)
        }
        (V::Int(a), V::Real(b)) => {
            if *b == 0.0 {
                return zero();
            }
            V::Real(*a as f64 % b)
        }
        (V::Int(_), V::Str(_)) => return err("Cannot modulo integer by string"),
        (V::Int(_), V::Date(_)) => return err("Cannot modulo integer by date values"),
        (V::Int(_), V::Dur(_)) => return err("Cannot modulo integer by duration values"),
        (V::Real(_), V::Bool(_)) => return err("Cannot modulo real by Boolean"),
        (V::Real(a), V::Int(_) | V::Real(_)) => {
            let d = as_real(r);
            if d == 0.0 {
                return zero();
            }
            V::Real(a % d)
        }
        (V::Real(_), V::Str(_)) => return err("Cannot modulo real numbers by strings"),
        (V::Real(_), V::Date(_)) => return err("Cannot modulo real numbers by dates"),
        (V::Real(_), V::Dur(_)) => return err("Cannot modulo real by duration values"),
        (V::Str(_), _) => return err("Cannot modulo string values"),
        (V::Date(_), _) => return err("Cannot modulo date values"),
        (V::Dur(_), _) => return err("Cannot modulo duration values"),
    })
}

fn pow(l: &V, r: &V) -> R {
    Ok(match (l, r) {
        (V::Bool(_), _) | (_, V::Bool(_)) => return err("Cannot exponentiate Booleans"),
        (V::Int(a), V::Int(b)) => V::Int((*a as f64).powf(*b as f64) as i32 as i64),
        (V::Int(_) | V::Real(_), V::Real(_)) => return err("Cannot exponentiate to a non-integer power"),
        (V::Real(a), V::Int(b)) => V::Real(a.powf(*b as f64)),
        (V::Str(_), _) | (_, V::Str(_)) => return err("Cannot exponentiate strings"),
        (V::Date(_), _) | (_, V::Date(_)) => return err("Cannot exponentiate dates"),
        (V::Dur(_), _) | (_, V::Dur(_)) => return err("Cannot exponentiate durations"),
    })
}

/// `=`: a string matches by prefix; anything else is equality.
fn partial_equal(l: &V, r: &V, clock: &Clock) -> bool {
    if let (V::Str(a), V::Str(b)) = (l, r) {
        return dequote(a).starts_with(dequote(b));
    }
    compare(l, r, clock) == Some(std::cmp::Ordering::Equal)
}

// ---------------------------------------------------------------------------------------------
// Evaluation

fn run_postfix(tokens: &[Tok], clock: &Clock, dom: DomFn) -> Result<V, String> {
    use std::cmp::Ordering::{Equal, Greater, Less};
    if tokens.is_empty() {
        return err("No expression to evaluate.");
    }
    let mut stack: Vec<V> = Vec::new();
    let could_not = || "The expression could not be evaluated.".to_owned();
    for tok in tokens {
        if tok.1 != T::Op {
            stack.push(literal(tok, clock, dom)?);
            continue;
        }
        match tok.0.as_str() {
            "!" => {
                let v = stack.pop().ok_or_else(could_not)?;
                stack.push(V::Bool(!truthy(&v)));
            }
            "_neg_" => {
                let v = stack.pop().ok_or_else(could_not)?;
                stack.push(sub(&V::Int(0), &v)?);
            }
            "_pos_" => {}
            op => {
                if stack.len() < 2 {
                    return Err(could_not());
                }
                let r = stack.pop().ok_or_else(could_not)?;
                let l = stack.pop().ok_or_else(could_not)?;
                let ord = |want: &[std::cmp::Ordering]| V::Bool(compare(&l, &r, clock).is_some_and(|o| want.contains(&o)));
                stack.push(match op {
                    "and" | "&&" => V::Bool(truthy(&l) && truthy(&r)),
                    "or" | "||" => V::Bool(truthy(&l) || truthy(&r)),
                    "xor" => V::Bool(truthy(&l) != truthy(&r)),
                    "<" => ord(&[Less]),
                    "<=" => ord(&[Less, Equal]),
                    ">" => ord(&[Greater]),
                    ">=" => ord(&[Greater, Equal]),
                    "==" => ord(&[Equal]),
                    "!==" => V::Bool(compare(&l, &r, clock) != Some(Equal)),
                    "=" => V::Bool(partial_equal(&l, &r, clock)),
                    "!=" => V::Bool(!partial_equal(&l, &r, clock)),
                    "+" => add(&l, &r, clock)?,
                    "-" => sub(&l, &r)?,
                    "*" => mul(&l, &r)?,
                    "/" => div(&l, &r)?,
                    "%" => modulo(&l, &r)?,
                    "^" => pow(&l, &r)?,
                    other => return Err(format!("Unsupported operator '{other}'.")),
                });
            }
        }
    }
    if stack.len() != 1 {
        return err("The value is not an expression.");
    }
    Ok(stack.remove(0))
}

/// Evaluate `expression`, read as infix (`1 + 2`) or, with `postfix`, as `1 2 +`.
pub fn calc(expression: &str, postfix: bool, clock: &Clock, dom: DomFn) -> Result<String, String> {
    let mut tokens = tokenize(expression);
    if !postfix {
        Parse(&mut tokens, 0).logical();
        tokens = to_postfix(tokens)?;
    }
    let v = run_postfix(&tokens, clock, dom)?;
    Ok(show(&v, clock))
}


// ---------------------------------------------------------------------------------------------
// The DOM (`DOM.cpp`)

/// What `calc` can look up: the settings, the program, and the tasks.
///
/// * `rc.<name>`: a setting, as text.
/// * `tw.version`, `tw.program`, `tw.args`, `tw.width`, `tw.height`, `tw.syncneeded`, the older
///   `context.*` spellings of the same, and `system.version`, `system.os`.
/// * `<id>.<attribute>`, `<uuid>.<attribute>` (a uuid may be cut short): any attribute, in its own
///   type (a date, a duration, a number, text); `tags.<word>`; `<date>.year` and the other parts of
///   a date; `annotations.count`, `annotations.<N>.description`, `.entry` and its parts.
pub struct DomSource<'a> {
    pub tasks: &'a [Facts],
    pub ids: &'a BTreeMap<Uuid, u32>,
    pub cfg: &'a Config,
    pub clock: &'a Clock,
    pub urgency: &'a dyn Fn(&Facts) -> f64,
    /// The command line as typed (`tw.args`).
    pub args: String,
    /// Whether there are local changes not yet synced (`tw.syncneeded`).
    pub sync_needed: bool,
}

impl DomSource<'_> {
    pub fn get(&self, name: &str) -> Option<Dom> {
        if name.is_empty() {
            return None;
        }
        let elements: Vec<&str> = name.split('.').collect();
        // A task named first: by id, or by uuid (all of it or its start).
        if elements.len() > 1 {
            let head = elements[0];
            let hex = head.len() >= 8 && head.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-');
            let task = if hex {
                let h = head.to_ascii_lowercase();
                self.tasks.iter().find(|t| t.uuid.to_string().starts_with(&h))
            } else if !head.is_empty() && head.bytes().all(|c| c.is_ascii_digit()) {
                let id: u32 = head.parse().unwrap_or(0);
                (id != 0).then(|| self.tasks.iter().find(|t| self.ids.get(&t.uuid) == Some(&id))).flatten()
            } else {
                None
            };
            if let Some(t) = task {
                if let Some(v) = self.of_task(t, &elements[1..]) {
                    return Some(v);
                }
            }
        }
        self.general(name)
    }

    /// Settings, the program and the system: nothing to do with a task.
    fn general(&self, name: &str) -> Option<Dom> {
        let text = |s: &str| Some(Dom::Text(s.to_owned()));
        if let Some(key) = name.strip_prefix("rc.").filter(|k| !k.is_empty()) {
            // Everything the app holds (UDAs, reports, contexts, urgency, settings), else the default.
            let held = crate::taskrc::render(self.cfg);
            let found = held.lines().find_map(|l| l.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v));
            return found
                .or_else(|| crate::taskrc::SETTING_DEFAULTS.iter().find(|(k, _)| *k == key).map(|(_, v)| *v))
                .and_then(text);
        }
        match name {
            "tw.syncneeded" => Some(Dom::Int(i64::from(self.sync_needed))),
            "tw.program" | "context.program" => text("task"),
            "tw.args" | "context.args" => text(&format!("task {}", self.args)),
            "tw.width" | "context.width" => Some(Dom::Int(80)),
            "tw.height" | "context.height" => Some(Dom::Int(24)),
            "tw.version" | "system.version" => text("3.5.0"),
            "system.os" => text("Cloudflare Workers"),
            _ => None,
        }
    }

    fn date(&self, ts: Option<i64>) -> Dom {
        match ts.filter(|t| *t != 0) {
            Some(t) => Dom::Date(t),
            None => Dom::Text(String::new()),
        }
    }

    fn of_task(&self, t: &Facts, rest: &[&str]) -> Option<Dom> {
        let clock = self.clock;
        // Attribute names are exact here: `1.desc` is not `1.description` (checked against task 3.5.0).
        let canonical = |name: &str| -> Option<(String, bool)> {
            crate::filter::canonical_attr_name(name, self.cfg)
                .filter(|(n, _)| n == name)
                .or_else(|| matches!(name, "mask" | "imask" | "rtype").then(|| (name.to_owned(), false)))
        };
        if let (1 | 2, Some((attr, is_uda))) = (rest.len(), canonical(rest[0])) {
            if rest.len() == 1 {
                return Some(match (attr.as_str(), is_uda) {
                    ("id", _) => Dom::Int(i64::from(self.ids.get(&t.uuid).copied().unwrap_or(0))),
                    ("urgency", _) => Dom::Real((self.urgency)(t)),
                    ("status", _) => Dom::Text(if t.status == "pending" && t.is_waiting(clock) {
                        "waiting".into()
                    } else {
                        t.status.clone()
                    }),
                    ("tags", _) => Dom::Text(t.tags.iter().cloned().collect::<Vec<_>>().join(",")),
                    ("uuid", _) => Dom::Text(t.uuid.to_string()),
                    ("description", _) => Dom::Text(t.description.clone()),
                    ("project", _) => Dom::Text(t.project.clone().unwrap_or_default()),
                    ("priority", _) => Dom::Text(t.priority.clone().unwrap_or_default()),
                    ("entry", _) => self.date(t.entry),
                    ("start", _) => self.date(t.start),
                    ("end", _) => self.date(t.end),
                    ("due", _) => self.date(t.due),
                    ("wait", _) => self.date(t.wait),
                    ("scheduled", _) => self.date(t.scheduled),
                    ("until", _) => self.date(t.until),
                    ("modified", _) => self.date(t.modified),
                    ("recur", _) => Dom::Dur(t.recur.as_deref().and_then(crate::dates::parse_duration).unwrap_or(0)),
                    ("parent", _) => Dom::Text(t.parent.map(|p| p.to_string()).unwrap_or_default()),
                    ("depends", _) => {
                        Dom::Text(t.depends.iter().map(Uuid::to_string).collect::<Vec<_>>().join(","))
                    }
                    ("mask", _) => Dom::Text(t.mask.clone().unwrap_or_default()),
                    ("imask", _) => Dom::Real(t.imask.map_or(0.0, |i| i as f64)),
                    (name, true) => {
                        let Some(raw) = t.extra.get(name) else { return Some(Dom::Text(String::new())) };
                        match self.cfg.udas.get(name).map(|u| u.ty) {
                            Some(UdaType::Numeric) => Dom::Real(raw.trim().parse().unwrap_or(0.0)),
                            Some(UdaType::Date) => self.date(raw.trim().parse().ok()),
                            Some(UdaType::Duration) => Dom::Dur(crate::dates::parse_duration(raw).unwrap_or(0)),
                            _ => Dom::Text(raw.clone()),
                        }
                    }
                    _ => return None,
                });
            }
            // `tags.<word>`
            if attr == "tags" {
                return Some(Dom::Text(if t.tags.contains(rest[1]) { rest[1].to_owned() } else { String::new() }));
            }
            // `due.year` and the other parts of a date.
            let ts = match attr.as_str() {
                "entry" => Some(t.entry),
                "start" => Some(t.start),
                "end" => Some(t.end),
                "due" => Some(t.due),
                "wait" => Some(t.wait),
                "scheduled" => Some(t.scheduled),
                "until" => Some(t.until),
                "modified" => Some(t.modified),
                name if is_uda && self.cfg.udas.get(name).is_some_and(|u| u.ty == UdaType::Date) => {
                    Some(t.extra.get(name).and_then(|v| v.trim().parse().ok()))
                }
                _ => None,
            };
            if let Some(ts) = ts {
                return self.date_part(ts.unwrap_or(0), rest[1]);
            }
        }
        // Annotations, counted from 1 in the order they were made.
        let mut notes: Vec<_> = t.annotations.iter().collect();
        notes.sort_by_key(|n| n.entry);
        match rest {
            ["annotations", "count"] => Some(Dom::Int(notes.len() as i64)),
            ["annotations", n, field @ ("entry" | "description")] => {
                let note = notes.get(n.parse::<usize>().ok()?.checked_sub(1)?)?;
                Some(if *field == "entry" { Dom::Date(note.entry) } else { Dom::Text(note.text.clone()) })
            }
            ["annotations", n, "entry", part] => {
                let note = notes.get(n.parse::<usize>().ok()?.checked_sub(1)?)?;
                self.date_part(note.entry, part)
            }
            _ => None,
        }
    }

    /// `year`, `month`, `day`, `week`, `weekday` (0 is Sunday), `julian` (day of the year), `hour`,
    /// `minute` or `second` of a moment.
    fn date_part(&self, ts: i64, part: &str) -> Option<Dom> {
        let (y, mo, d, h, mi, s) = self.clock.ymd_hms(ts);
        let date = taskchampion::chrono::NaiveDate::from_ymd_opt(y, mo, d)?;
        Some(Dom::Int(match part {
            "year" => i64::from(y),
            "month" => i64::from(mo),
            "day" => i64::from(d),
            "week" => i64::from(crate::calendar::week_number(date, self.clock.week_starts_monday)),
            "weekday" => i64::from(self.clock.day_of_week(ts)),
            "julian" => i64::from(date.ordinal()),
            "hour" => i64::from(h),
            "minute" => i64::from(mi),
            "second" => i64::from(s),
            _ => return None,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(expr: &str) -> String {
        calc(expr, false, &Clock::utc(1_791_376_200), &|_| None).unwrap_or_else(|e| format!("! {e}"))
    }
    fn p(expr: &str) -> String {
        calc(expr, true, &Clock::utc(1_791_376_200), &|_| None).unwrap_or_else(|e| format!("! {e}"))
    }

    // Every expected value below is what `task calc` 3.5.0 printed.
    #[test]
    fn arithmetic_like_taskwarrior() {
        for (e, want) in [
            ("1 + 2", "3"), ("1 + 2 * 3", "7"), ("(1 + 2) * 3", "9"), ("7 / 2", "3"), ("7.0 / 2", "3.5"),
            ("7 % 3", "1"), ("2 ^ 10", "1024"), ("2 ^ 3 ^ 2", "512"), ("-3 + 1", "-2"), ("- 3", "-3"),
            ("3 - -2", "5"), ("1.5 + 1.5", "3"), ("0.1 + 0.2", "0.3"), ("1 / 3", "0"), ("-7 / 2", "-3"),
            ("-7 % 3", "-1"), ("7 % -3", "1"), ("1 ++ 2", "3"), ("1 / 3.0", "0.333333"),
            ("100.0 / 3", "33.3333"), ("0.000001", "1e-06"), ("123456789.123", "1.23457e+08"),
            ("2.5 ^ 2", "6.25"), ("pi", "3.14159"), ("pi * 2", "6.28318"),
        ] {
            assert_eq!(c(e), want, "{e}");
        }
    }

    #[test]
    fn errors_use_taskwarriors_words() {
        for (e, want) in [
            ("5 / 0", "! Cannot divide by zero"), ("5 % 0", "! Cannot modulo zero"), ("5.0 / 0", "! Cannot divide by zero"),
            ("2 ^ 0.5", "! Cannot exponentiate to a non-integer power"),
            // The unary minus is stacked so that `^` is applied too early; Taskwarrior can't evaluate it either.
            ("2 ^ -1", "! The expression could not be evaluated."),
            ("1 +", "! The expression could not be evaluated."), ("(1 + 2", "! Mismatched parentheses in expression"),
            ("1 2", "! The value is not an expression."), ("not true", "! The value is not an expression."),
            ("", "! No expression to evaluate."),
            ("2026-12-25 * 2", "! Cannot multiply date values"), ("2026-12-25 + 2026-12-25", "! Cannot add two date values"),
        ] {
            assert_eq!(c(e), want, "{e}");
        }
    }

    #[test]
    fn logic_and_comparison() {
        for (e, want) in [
            ("1 < 2", "true"), ("2 < 1", "false"), ("1 <= 1", "true"), ("1 == 1", "true"), ("1 != 2", "true"),
            ("1 == 1.0", "true"), ("true and false", "false"), ("true or false", "true"), ("true xor true", "false"),
            ("!true", "false"), ("1 and 0", "false"), ("4 xor 0", "true"), ("!0", "true"), ("!5", "false"),
            ("1 < 2 < 3", "true"), ("3 > 2 > 1", "false"), ("1 = 1", "true"), ("1 != 1", "false"),
            ("true + 1", "2"), ("true * 3", "3"),
        ] {
            assert_eq!(c(e), want, "{e}");
        }
    }

    #[test]
    fn durations() {
        for (e, want) in [
            ("2 days", "P2D"), ("2d", "P2D"), ("2 days + 3 hours", "P2DT3H"), ("1h + 30min", "PT1H30M"),
            ("2d * 2", "P4D"), ("2d / 2", "P1D"), ("1w + 1d", "P8D"), ("P1D", "P1D"), ("P1D + P2D", "P3D"),
            ("2d > 1d", "true"), ("90m", "P2700D"), ("120min", "PT2H"), ("3600s", "PT1H"), ("36 hours", "P1DT12H"),
            ("1y", "P365D"), ("1 year", "P365D"), ("2 months", "P60D"), ("1q", "P91D"), ("1d + 30s", "P1DT30S"),
            ("2h * 1.5", "PT3H"), ("1h / 3", "PT20M"), ("1d / 1h", "24"), ("7d / 2d", "3.5"), ("10 * 1d", "P10D"),
            ("PT90M", "PT1H30M"), ("P1Y2M3DT4H5M6S", "P428DT4H5M6S"), ("P1W", "P7D"), ("0d", "PT0S"),
            ("-1d", "-P1D"), ("2d - 3d", "-P1D"),
        ] {
            assert_eq!(c(e), want, "{e}");
        }
    }

    #[test]
    fn dates() {
        for (e, want) in [
            ("2026-12-25", "2026-12-25T00:00:00"), ("2026-12-25 - 2026-12-01", "P24D"),
            ("2026-12-25 + 1w", "2027-01-01T00:00:00"), ("2026-12-25 + 2d", "2026-12-27T00:00:00"),
            ("2026-12-25 - 1d", "2026-12-24T00:00:00"), ("2026-12-25T10:00", "2026-12-25T10:00:00"),
            ("2026-12-25 < 2026-12-26", "true"), ("2026-12-25 == 2026-12-25", "true"),
            ("5 + 2026-12-25", "2026-12-25T00:00:05"), ("2026-12-25 - 5", "2026-12-24T23:59:55"),
            ("2026-12-25 + 1y", "2027-12-25T00:00:00"), ("2026-12-25 + 1 month", "2027-01-24T00:00:00"),
            ("now - now", "PT0S"), ("today - today", "PT0S"), ("now > 2000-01-01", "true"),
        ] {
            assert_eq!(c(e), want, "{e}");
        }
    }

    #[test]
    fn strings_keep_their_quotes_as_typed() {
        for (e, want) in [
            ("\"abc\"", "\"abc\""), ("'abc'", "'abc'"), ("abc", "abc"), ("abc + def", "abcdef"),
            ("\"abc\" + \"def\"", "\"abc\"def"), ("\"abc\" + 1", "\"abc\"1"), ("1 + \"abc\"", "1abc"),
            ("\"abc\" - \"def\"", "\"abc\"-\"def\""), ("\"abc\" * 3", "\"abc\"\"abc\"\"abc\""),
            ("\"abc\" == \"abc\"", "true"), ("\"abc\" == \"ABC\"", "false"), ("\"abc\" = \"ab\"", "true"),
            ("\"abc\" == abc", "true"), ("a < b", "true"), ("\"abc\" < \"abd\"", "true"), ("\"abc\" and 1", "true"),
            ("\"a b\"", "\"a b\""), ("\"\" + \"\"", "\"\""),
        ] {
            assert_eq!(c(e), want, "{e}");
        }
    }

    #[test]
    fn postfix_reads_the_operator_last() {
        for (e, want) in [
            ("1 2 +", "3"), ("1 2 + 3 *", "9"), ("2 days 3 hours +", "P2DT3H"), ("1 2 < ", "true"),
            ("2 3 ^", "8"), ("1 +", "! The expression could not be evaluated."),
            ("1 2", "! The value is not an expression."),
        ] {
            assert_eq!(p(e), want, "{e}");
        }
        // Taskwarrior ignores what its infix parser makes of the words, and just reorders them, so
        // these happen to give the same answer in infix. (Checked against task 3.5.0.)
        assert_eq!(c("1 2 +"), "3");
        assert_eq!(p("5 !"), "false");
        assert_eq!(p("1 2 3 + +"), "6");
        assert_eq!(p("3 2 -"), "1");
        assert_eq!(p("2 neg"), "! The value is not an expression.");
    }

    #[test]
    fn reals_print_like_cplusplus() {
        assert_eq!(fmt_real(3.0), "3");
        assert_eq!(fmt_real(0.5), "0.5");
        assert_eq!(fmt_real(1e10), "1e+10");
        assert_eq!(fmt_real(-2.5e-5), "-2.5e-05");
        assert_eq!(fmt_real(999999.5), "1e+06");
        assert_eq!(fmt_real(123456.0), "123456");
    }
}
