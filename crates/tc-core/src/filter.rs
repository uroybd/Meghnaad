//! Taskwarrior filter expressions: `status:pending +work (due.before:eow or priority:H) /milk/`.
//!
//! Semantics are transcribed from Taskwarrior's `CLI2::desugarFilterAttributes` and
//! `Variant::operator_partial`:
//!  * bare `attr:value` is a *partial* match: string prefix (so `project:Home` also matches
//!    `Homework` and `Home.Kitchen`), same calendar day for dates, equality for numbers;
//!    `status` is always compared case-insensitively;
//!  * `.is` exact, `.not` negated partial, `.isnt` negated exact, `.has` substring, `.hasnt`,
//!    `.startswith`, `.endswith`, `.word`, `.noword`, `.before`, `.after`, `.by`, `.none`, `.any`;
//!  * terms combine with `and` (implicit), `or`, `xor`, `not`, and parentheses;
//!  * `+tag`/`-tag` (including virtual tags like `+OVERDUE`), `/text/` and bare words search
//!    the description, UUID (prefix) and ids (`3`, `1-4,7`) select tasks;
//!  * `limit:N|page|none` is a directive, not a predicate.

use crate::dates::{parse_date_expr, Clock};
use crate::model::Facts;
use crate::taskrc::{Config, UdaType};
use crate::urgency::{coefficients, urgency_with};
use std::collections::BTreeMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterError(pub String);

impl std::fmt::Display for FilterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for FilterError {}

fn err<T>(msg: impl Into<String>) -> Result<T, FilterError> {
    Err(FilterError(msg.into()))
}

/// Everything evaluation needs besides the task itself.
pub struct EvalCtx<'a> {
    pub cfg: &'a Config,
    pub clock: Clock,
    /// Working-set ids (task number -> uuid) that `task 3 ...` refers to.
    pub ids: &'a BTreeMap<Uuid, u32>,
    coef: BTreeMap<String, f64>,
}

impl<'a> EvalCtx<'a> {
    pub fn new(cfg: &'a Config, clock: Clock, ids: &'a BTreeMap<Uuid, u32>) -> Self {
        EvalCtx { cfg, clock, ids, coef: coefficients(cfg) }
    }

    pub fn urgency(&self, f: &Facts) -> f64 {
        urgency_with(f, self.cfg, &self.clock, &self.coef)
    }

    fn case_sensitive(&self) -> bool {
        self.cfg
            .settings
            .get("search.case.sensitive")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "on" | "yes" | "y" | "true"))
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    None,
    /// `limit:page`: a terminal-height concept; the web UI treats it as "no limit".
    Page,
    N(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Core {
    Id,
    Uuid,
    Status,
    Description,
    Project,
    Priority,
    Entry,
    Start,
    End,
    Due,
    Wait,
    Scheduled,
    Until,
    Modified,
    Recur,
    Parent,
    Depends,
    Tags,
    Annotation,
    Urgency,
}

const CORE_NAMES: &[(&str, Core)] = &[
    ("id", Core::Id),
    ("uuid", Core::Uuid),
    ("status", Core::Status),
    ("description", Core::Description),
    ("project", Core::Project),
    ("priority", Core::Priority),
    ("entry", Core::Entry),
    ("start", Core::Start),
    ("end", Core::End),
    ("due", Core::Due),
    ("wait", Core::Wait),
    ("scheduled", Core::Scheduled),
    ("until", Core::Until),
    ("modified", Core::Modified),
    ("recur", Core::Recur),
    ("parent", Core::Parent),
    ("depends", Core::Depends),
    ("tags", Core::Tags),
    ("annotation", Core::Annotation),
    ("urgency", Core::Urgency),
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Attr {
    Core(Core),
    Uda(String, UdaType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    Str,
    Num,
    Date,
    Many,
}

impl Attr {
    fn ty(&self) -> Ty {
        match self {
            Attr::Core(c) => match c {
                Core::Id | Core::Urgency => Ty::Num,
                Core::Entry | Core::Start | Core::End | Core::Due | Core::Wait | Core::Scheduled
                | Core::Until | Core::Modified => Ty::Date,
                Core::Depends | Core::Tags | Core::Annotation => Ty::Many,
                _ => Ty::Str,
            },
            Attr::Uda(_, UdaType::Numeric) => Ty::Num,
            Attr::Uda(_, UdaType::Date) => Ty::Date,
            Attr::Uda(..) => Ty::Str,
        }
    }

    fn is_status(&self) -> bool {
        *self == Attr::Core(Core::Status)
    }
}

/// Resolve an attribute name, allowing unique abbreviations of at least two characters
/// (`desc`, `proj`), as Taskwarrior does.
fn canonicalize(name: &str, cfg: &Config) -> Option<Attr> {
    let mut all: Vec<(String, Attr)> =
        CORE_NAMES.iter().map(|(n, c)| ((*n).to_owned(), Attr::Core(*c))).collect();
    all.extend(cfg.udas.values().map(|u| (u.name.clone(), Attr::Uda(u.name.clone(), u.ty))));
    if let Some((_, a)) = all.iter().find(|(n, _)| n == name) {
        return Some(a.clone());
    }
    if name.len() < 2 {
        return None;
    }
    let mut hits = all.iter().filter(|(n, _)| n.starts_with(name));
    match (hits.next(), hits.next()) {
        (Some((_, a)), None) => Some(a.clone()),
        _ => None,
    }
}

/// Canonical attribute name for `name` (unique abbreviations allowed), and whether it is a UDA.
pub(crate) fn canonical_attr_name(name: &str, cfg: &Config) -> Option<(String, bool)> {
    canonicalize(name, cfg).map(|a| match a {
        Attr::Uda(n, _) => (n, true),
        Attr::Core(c) => (
            CORE_NAMES.iter().find(|(_, k)| *k == c).map(|(n, _)| (*n).to_owned()).unwrap_or_default(),
            false,
        ),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Partial,
    Is,
    Not,
    Isnt,
    Has,
    Hasnt,
    StartsWith,
    EndsWith,
    Word,
    NoWord,
    Before,
    After,
    By,
    None,
    Any,
}

fn parse_modifier(m: &str) -> Result<Op, FilterError> {
    Ok(match m {
        "" => Op::Partial,
        "before" | "under" | "below" => Op::Before,
        "after" | "over" | "above" => Op::After,
        "by" => Op::By,
        "none" => Op::None,
        "any" => Op::Any,
        "is" | "equals" => Op::Is,
        "not" => Op::Not,
        "isnt" => Op::Isnt,
        "has" | "contains" => Op::Has,
        "hasnt" => Op::Hasnt,
        "startswith" | "left" => Op::StartsWith,
        "endswith" | "right" => Op::EndsWith,
        "word" => Op::Word,
        "noword" => Op::NoWord,
        other => return err(format!("unrecognized attribute modifier '{other}'")),
    })
}

#[derive(Debug, Clone, PartialEq)]
enum Term {
    Tag { name: String, present: bool },
    Attr { attr: Attr, op: Op, value: String, date: Option<i64>, day: bool, num: Option<f64> },
    Text(String),
    Uuid(String),
    Ids(Vec<(u32, u32)>),
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    All,
    Term(Term),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Xor(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    expr: Expr,
    pub limit: Limit,
}

/// Split a command-line-ish string into words, honouring `'` and `"` quotes.
pub fn split_words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '\'' | '"') => {
                quote = Some(c);
                in_word = true;
            }
            (None, '\\') if matches!(chars.peek(), Some('\'' | '"' | '\\' | ' ')) => {
                cur.push(chars.next().unwrap());
                in_word = true;
            }
            (None, c) if c.is_whitespace() => {
                if in_word {
                    out.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            (None, c) => {
                cur.push(c);
                in_word = true;
            }
        }
    }
    if in_word {
        out.push(cur);
    }
    out
}

/// Peel `(` and `)` off the edges of words so `(due:today` and `+a)` become separate tokens.
fn tokenize(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        let mut s = a.as_str();
        let mut tail = 0;
        while let Some(r) = s.strip_prefix('(') {
            out.push("(".to_owned());
            s = r;
        }
        while let Some(r) = s.strip_suffix(')') {
            // Don't split a ')' that closes something opened inside the word.
            if s.matches('(').count() >= s.matches(')').count() {
                break;
            }
            tail += 1;
            s = r;
        }
        if !s.is_empty() {
            out.push(s.to_owned());
        }
        for _ in 0..tail {
            out.push(")".to_owned());
        }
    }
    out
}

fn is_uuidish(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if Uuid::try_parse(&lower).is_ok() {
        return true;
    }
    // Short form: 8+ hex digits containing at least one letter (so plain numbers stay numbers).
    lower.len() >= 8
        && lower.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        && lower.chars().any(|c| matches!(c, 'a'..='f'))
        && !lower.starts_with('-')
}

fn parse_ids(s: &str) -> Option<Vec<(u32, u32)>> {
    if !s.chars().next()?.is_ascii_digit() {
        return None;
    }
    let mut out = Vec::new();
    for part in s.split(',') {
        let (lo, hi) = match part.split_once('-') {
            Some((a, b)) => (a.parse().ok()?, b.parse().ok()?),
            None => {
                let n = part.parse().ok()?;
                (n, n)
            }
        };
        if lo > hi {
            return None;
        }
        out.push((lo, hi));
    }
    Some(out)
}

/// `<name>[.<mod>](:|=)<value>`; `None` if it doesn't have that shape.
fn split_pair(tok: &str) -> Option<(&str, &str, &str)> {
    let sep = tok.find([':', '='])?;
    let (lhs, value) = (&tok[..sep], &tok[sep + 1..]);
    let (name, modifier) = lhs.split_once('.').unwrap_or((lhs, ""));
    let ident = |s: &str| {
        !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    };
    if !ident(name) || (!modifier.is_empty() && !ident(modifier)) {
        return None;
    }
    Some((name, modifier, value))
}

struct Parser<'a, 'c> {
    toks: Vec<String>,
    pos: usize,
    ctx: &'a EvalCtx<'c>,
    limit: Limit,
}

impl Parser<'_, '_> {
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.pos).map(String::as_str)
    }

    fn is_or(t: &str) -> bool {
        matches!(t, "or" | "||" | "xor")
    }

    fn expr(&mut self) -> Result<Expr, FilterError> {
        let mut left = self.and_expr()?;
        while let Some(t) = self.peek() {
            if !Self::is_or(t) {
                break;
            }
            let xor = t == "xor";
            self.pos += 1;
            let right = self.and_expr()?;
            left = if xor {
                Expr::Xor(Box::new(left), Box::new(right))
            } else {
                Expr::Or(Box::new(left), Box::new(right))
            };
        }
        Ok(left)
    }

    fn and_expr(&mut self) -> Result<Expr, FilterError> {
        let mut left = self.unary()?;
        loop {
            match self.peek() {
                None | Some(")") => break,
                Some(t) if Self::is_or(t) => break,
                Some("and" | "&&") => self.pos += 1,
                Some(_) => {} // implicit and
            }
            let right = self.unary()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, FilterError> {
        match self.peek() {
            Some("not" | "!") => {
                self.pos += 1;
                Ok(Expr::Not(Box::new(self.unary()?)))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<Expr, FilterError> {
        let Some(tok) = self.peek().map(str::to_owned) else {
            return err("filter ended where a term was expected");
        };
        self.pos += 1;
        match tok.as_str() {
            "(" => {
                let e = self.expr()?;
                if self.peek() != Some(")") {
                    return err("missing closing parenthesis in filter");
                }
                self.pos += 1;
                Ok(e)
            }
            ")" => err("unexpected closing parenthesis in filter"),
            "and" | "or" | "xor" | "&&" | "||" => err(format!("operator '{tok}' needs a term on both sides")),
            _ => self.term(&tok).map(|t| match t {
                Some(t) => Expr::Term(t),
                None => Expr::All,
            }),
        }
    }

    /// `Ok(None)` for directives that match everything (`limit:`).
    fn term(&mut self, tok: &str) -> Result<Option<Term>, FilterError> {
        let ctx = self.ctx;
        if let Some(v) = tok.strip_prefix("limit:") {
            self.limit = match v {
                "none" | "0" => Limit::None,
                "page" => Limit::Page,
                n => Limit::N(n.parse().map_err(|_| FilterError(format!("invalid limit '{n}'")))?),
            };
            return Ok(None);
        }
        // rc overrides don't belong to the filter.
        if tok.starts_with("rc.") || tok.starts_with("rc:") {
            return Ok(None);
        }

        let tag_char = |c: char| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '@' | '/');
        if let Some(name) = tok.strip_prefix('+') {
            if !name.is_empty() && name.chars().all(tag_char) {
                return Ok(Some(Term::Tag { name: name.to_owned(), present: true }));
            }
        }
        if let Some(name) = tok.strip_prefix('-') {
            if name.chars().next().is_some_and(|c| c.is_alphabetic())
                && name.chars().all(tag_char)
            {
                return Ok(Some(Term::Tag { name: name.to_owned(), present: false }));
            }
        }

        if let Some((name, modifier, value)) = split_pair(tok) {
            if let Some(attr) = canonicalize(name, ctx.cfg) {
                let op = parse_modifier(modifier)?;
                return self.attr_term(attr, op, value).map(Some);
            }
        }

        if tok.len() > 2 && tok.starts_with('/') && tok.ends_with('/') {
            return Ok(Some(Term::Text(tok[1..tok.len() - 1].to_owned())));
        }
        if is_uuidish(tok) {
            return Ok(Some(Term::Uuid(tok.to_ascii_lowercase())));
        }
        if let Some(ids) = parse_ids(tok) {
            return Ok(Some(Term::Ids(ids)));
        }
        Ok(Some(Term::Text(tok.to_owned())))
    }

    fn attr_term(&self, attr: Attr, op: Op, value: &str) -> Result<Term, FilterError> {
        let (mut date, mut day, mut num) = (None, false, None);
        let needs_value = !matches!(op, Op::None | Op::Any);
        if needs_value && !value.is_empty() {
            match attr.ty() {
                Ty::Date => match parse_date_expr(value, &self.ctx.clock) {
                    Some(p) => (date, day) = (Some(p.ts), p.day),
                    None => return err(format!("'{value}' is not a valid date")),
                },
                Ty::Num => match value.parse::<f64>() {
                    Ok(n) => num = Some(n),
                    Err(_) => return err(format!("'{value}' is not a number")),
                },
                _ => {}
            }
        }
        Ok(Term::Attr { attr, op, value: value.to_owned(), date, day, num })
    }
}

impl Filter {
    pub fn match_all() -> Filter {
        Filter { expr: Expr::All, limit: Limit::None }
    }

    /// Parse already-split arguments (e.g. from the command-line parser).
    pub fn parse(args: &[String], ctx: &EvalCtx) -> Result<Filter, FilterError> {
        let toks = tokenize(args);
        if toks.is_empty() {
            return Ok(Filter::match_all());
        }
        let mut p = Parser { toks, pos: 0, ctx, limit: Limit::None };
        let expr = p.expr()?;
        if p.pos < p.toks.len() {
            return if p.toks[p.pos] == ")" {
                err("unexpected closing parenthesis in filter")
            } else {
                err(format!("unexpected '{}' in filter", p.toks[p.pos]))
            };
        }
        Ok(Filter { expr, limit: p.limit })
    }

    pub fn parse_str(s: &str, ctx: &EvalCtx) -> Result<Filter, FilterError> {
        Filter::parse(&split_words(s), ctx)
    }

    pub fn matches(&self, f: &Facts, ctx: &EvalCtx) -> bool {
        eval(&self.expr, f, ctx)
    }
}

/// Join filter fragments (report filter, context, user filter) with an `and` between them.
pub fn conjoin(parts: &[Vec<String>]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in parts.iter().filter(|p| !p.is_empty()) {
        if !out.is_empty() {
            out.push("and".into());
        }
        out.push("(".into());
        out.extend(p.iter().cloned());
        out.push(")".into());
    }
    out
}

fn eval(e: &Expr, f: &Facts, ctx: &EvalCtx) -> bool {
    match e {
        Expr::All => true,
        Expr::Not(x) => !eval(x, f, ctx),
        Expr::And(a, b) => eval(a, f, ctx) && eval(b, f, ctx),
        Expr::Or(a, b) => eval(a, f, ctx) || eval(b, f, ctx),
        Expr::Xor(a, b) => eval(a, f, ctx) != eval(b, f, ctx),
        Expr::Term(t) => eval_term(t, f, ctx),
    }
}

fn contains(hay: &str, needle: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        hay.contains(needle)
    } else {
        hay.to_lowercase().contains(&needle.to_lowercase())
    }
}

fn eval_term(t: &Term, f: &Facts, ctx: &EvalCtx) -> bool {
    match t {
        Term::Tag { name, present } => f.has_tag(name, ctx.cfg, &ctx.clock) == *present,
        Term::Text(s) => contains(&f.description, s, ctx.case_sensitive()),
        Term::Uuid(prefix) => {
            let simple = f.uuid.as_simple().to_string();
            let hyphen = f.uuid.to_string();
            simple.starts_with(prefix) || hyphen.starts_with(prefix)
        }
        Term::Ids(ranges) => ctx
            .ids
            .get(&f.uuid)
            .is_some_and(|id| ranges.iter().any(|(lo, hi)| id >= lo && id <= hi)),
        Term::Attr { attr, op, value, date, day, num } => {
            eval_attr(attr, *op, value, *date, *day, *num, f, ctx)
        }
    }
}

enum Val {
    Unset,
    Str(String),
    Num(f64),
    Date(i64),
    Many(Vec<String>),
}

fn attr_value(attr: &Attr, f: &Facts, ctx: &EvalCtx) -> Val {
    let s = |o: &Option<String>| o.clone().map_or(Val::Unset, Val::Str);
    let d = |o: Option<i64>| o.map_or(Val::Unset, Val::Date);
    match attr {
        Attr::Core(c) => match c {
            Core::Id => ctx.ids.get(&f.uuid).map_or(Val::Unset, |i| Val::Num(f64::from(*i))),
            Core::Uuid => Val::Str(f.uuid.to_string()),
            Core::Status => Val::Str(f.status.clone()),
            Core::Description => Val::Str(f.description.clone()),
            Core::Project => s(&f.project),
            Core::Priority => s(&f.priority),
            Core::Recur => s(&f.recur),
            Core::Parent => f.parent.map_or(Val::Unset, |p| Val::Str(p.to_string())),
            Core::Entry => d(f.entry),
            Core::Start => d(f.start),
            Core::End => d(f.end),
            Core::Due => d(f.due),
            Core::Wait => d(f.wait),
            Core::Scheduled => d(f.scheduled),
            Core::Until => d(f.until),
            Core::Modified => d(f.modified),
            Core::Urgency => Val::Num(ctx.urgency(f)),
            Core::Tags => Val::Many(f.tags.iter().cloned().collect()),
            Core::Depends => Val::Many(f.depends.iter().map(Uuid::to_string).collect()),
            Core::Annotation => Val::Many(f.annotations.iter().map(|a| a.text.clone()).collect()),
        },
        Attr::Uda(name, ty) => match f.extra.get(name) {
            None => Val::Unset,
            Some(raw) => match ty {
                UdaType::Numeric => raw.parse().map_or(Val::Unset, Val::Num),
                UdaType::Date => raw.parse().map_or(Val::Unset, Val::Date),
                _ => Val::Str(raw.clone()),
            },
        },
    }
}

fn is_word_in(hay: &str, needle: &str, case_sensitive: bool) -> bool {
    let (h, n) = if case_sensitive {
        (hay.to_owned(), needle.to_owned())
    } else {
        (hay.to_lowercase(), needle.to_lowercase())
    };
    if n.is_empty() {
        return false;
    }
    let is_w = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0;
    while let Some(i) = h[from..].find(&n) {
        let start = from + i;
        let end = start + n.len();
        let before_ok = h[..start].chars().next_back().map_or(true, |c| !is_w(c));
        let after_ok = h[end..].chars().next().map_or(true, |c| !is_w(c));
        if before_ok && after_ok {
            return true;
        }
        from = start + h[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

fn str_op(have: &str, op: Op, want: &str, status: bool, cs: bool) -> bool {
    let eq = |a: &str, b: &str| if status || !cs { a.eq_ignore_ascii_case(b) } else { a == b };
    let starts = |a: &str, b: &str| {
        if status {
            a.eq_ignore_ascii_case(b)
        } else {
            // Taskwarrior's partial match: an empty side only matches an empty side.
            (a.is_empty() == b.is_empty()) && a.starts_with(b)
        }
    };
    match op {
        Op::Partial => starts(have, want),
        Op::Not => !starts(have, want),
        Op::Is => eq(have, want),
        Op::Isnt => !eq(have, want),
        Op::Has => contains(have, want, cs),
        Op::Hasnt => !contains(have, want, cs),
        Op::StartsWith => {
            if cs { have.starts_with(want) } else { have.to_lowercase().starts_with(&want.to_lowercase()) }
        }
        Op::EndsWith => {
            if cs { have.ends_with(want) } else { have.to_lowercase().ends_with(&want.to_lowercase()) }
        }
        Op::Word => is_word_in(have, want, cs),
        Op::NoWord => !is_word_in(have, want, cs),
        Op::Before => have < want,
        Op::After => have > want,
        Op::By => have <= want,
        Op::None | Op::Any => unreachable!("handled by caller"),
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_attr(
    attr: &Attr,
    op: Op,
    value: &str,
    date: Option<i64>,
    day: bool,
    num: Option<f64>,
    f: &Facts,
    ctx: &EvalCtx,
) -> bool {
    let cs = ctx.case_sensitive();
    let have = attr_value(attr, f, ctx);
    let unset = match &have {
        Val::Unset => true,
        Val::Str(s) => s.is_empty(),
        Val::Many(v) => v.is_empty(),
        _ => false,
    };

    match op {
        Op::None => return unset,
        Op::Any => return !unset,
        _ => {}
    }
    // `due:` with an empty value selects tasks that have no due date, like Taskwarrior.
    if value.is_empty() && matches!(op, Op::Partial | Op::Is) {
        return unset;
    }
    if value.is_empty() && matches!(op, Op::Not | Op::Isnt) {
        return !unset;
    }

    // `status:waiting` is Taskwarrior 3's computed state, not a stored one.
    if attr.is_status() && value.eq_ignore_ascii_case("waiting") && matches!(op, Op::Partial | Op::Is) {
        return f.is_waiting(&ctx.clock);
    }

    match have {
        Val::Unset => matches!(op, Op::Not | Op::Isnt | Op::Hasnt | Op::NoWord),
        Val::Str(h) => str_op(&h, op, value, attr.is_status(), cs),
        Val::Num(h) => {
            let Some(w) = num else { return false };
            match op {
                Op::Partial | Op::Is => h == w,
                Op::Not | Op::Isnt => h != w,
                Op::Before => h < w,
                Op::After => h > w,
                Op::By => h <= w,
                _ => false,
            }
        }
        Val::Date(h) => {
            let Some(w) = date else { return false };
            match op {
                Op::Partial => ctx.clock.same_day(h, w),
                Op::Not => !ctx.clock.same_day(h, w),
                Op::Is => if day { ctx.clock.same_day(h, w) } else { h == w },
                Op::Isnt => if day { !ctx.clock.same_day(h, w) } else { h != w },
                Op::Before => h < w,
                Op::After => h > w,
                Op::By => h <= w,
                _ => false,
            }
        }
        Val::Many(items) => {
            let any = |op| items.iter().any(|i| str_op(i, op, value, false, cs));
            match op {
                // Tags/depends/annotations: "has an element that ...".
                Op::Partial | Op::Is | Op::Has | Op::StartsWith | Op::EndsWith | Op::Word => any(match op {
                    Op::Partial => Op::Is,
                    o => o,
                }),
                Op::Not | Op::Isnt | Op::Hasnt | Op::NoWord => !any(match op {
                    Op::Not | Op::Isnt => Op::Is,
                    Op::Hasnt => Op::Has,
                    _ => Op::Word,
                }),
                _ => false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dates::DAY;
    use crate::model::tests::{clock, task, NOW};
    use crate::taskrc::parse;

    struct H {
        cfg: Config,
        ids: BTreeMap<Uuid, u32>,
    }

    impl H {
        fn new() -> Self {
            H { cfg: Config::default(), ids: BTreeMap::new() }
        }

        fn with_rc(rc: &str) -> Self {
            H { cfg: parse(rc).config, ids: BTreeMap::new() }
        }

        fn ctx(&self) -> EvalCtx<'_> {
            EvalCtx::new(&self.cfg, clock(), &self.ids)
        }

        fn m(&self, filter: &str, f: &Facts) -> bool {
            let ctx = self.ctx();
            Filter::parse_str(filter, &ctx).unwrap().matches(f, &ctx)
        }

        fn parse_err(&self, filter: &str) -> String {
            Filter::parse_str(filter, &self.ctx()).unwrap_err().0
        }
    }

    fn proj(p: &str) -> Facts {
        Facts { project: Some(p.into()), ..task("t") }
    }

    #[test]
    fn empty_filter_matches_everything() {
        assert!(H::new().m("", &task("x")));
    }

    #[test]
    fn project_default_match_is_a_prefix() {
        let h = H::new();
        assert!(h.m("project:Home", &proj("Home")));
        assert!(h.m("project:Home", &proj("Home.Kitchen")));
        // The well-known gotcha, faithfully reproduced:
        assert!(h.m("project:Home", &proj("Homework")));
        assert!(!h.m("project:Home", &proj("Work")));
        assert!(!h.m("project:Home", &task("none")));
        // Exact and negated forms.
        assert!(h.m("project.is:Home", &proj("Home")));
        assert!(!h.m("project.is:Home", &proj("Home.Kitchen")));
        assert!(h.m("project.not:Home", &proj("Work")));
        assert!(h.m("project.isnt:Home", &proj("Home.Kitchen")));
        // Case sensitive by default.
        assert!(!h.m("project:home", &proj("Home")));
        let ci = H::with_rc("search.case.sensitive=no\n");
        assert!(ci.m("project.is:home", &proj("Home")));
    }

    #[test]
    fn empty_value_and_none_any() {
        let h = H::new();
        assert!(h.m("project:", &task("x")));
        assert!(!h.m("project:", &proj("Home")));
        assert!(h.m("project.none:", &task("x")));
        assert!(h.m("project.any:", &proj("Home")));
        assert!(h.m("due.none:", &task("x")));
        let mut t = task("x");
        t.due = Some(NOW);
        assert!(h.m("due.any:", &t) && !h.m("due:", &t));
    }

    #[test]
    fn string_modifiers() {
        let h = H::new();
        let t = task("Buy fresh milk today");
        assert!(h.m("description.has:fresh", &t));
        assert!(!h.m("description.hasnt:fresh", &t));
        assert!(h.m("description.startswith:Buy", &t));
        assert!(h.m("description.endswith:today", &t));
        assert!(h.m("description.word:milk", &t));
        assert!(!h.m("description.word:mil", &t));
        assert!(h.m("description.noword:mil", &t));
        assert!(h.m("desc.has:milk", &t)); // abbreviation
        assert!(h.m("/fresh/", &t) && h.m("milk", &t) && !h.m("bread", &t));
    }

    #[test]
    fn status_is_caseless_and_waiting_is_computed() {
        let h = H::new();
        let mut t = task("x");
        assert!(h.m("status:pending", &t) && h.m("status:PENDING", &t));
        assert!(!h.m("status:completed", &t));
        assert!(!h.m("status:waiting", &t));
        t.wait = Some(NOW + DAY);
        assert!(h.m("status:waiting", &t));
        // pending still matches pending: the stored status is unchanged.
        assert!(h.m("status:pending", &t));
        assert!(!h.m("status:pending -WAITING", &t));
    }

    #[test]
    fn tags_and_virtual_tags() {
        let h = H::new();
        let mut t = task("x");
        t.tags.insert("work".into());
        assert!(h.m("+work", &t) && !h.m("-work", &t) && h.m("-home", &t));
        assert!(h.m("+TAGGED +PENDING", &t));
        assert!(h.m("tags:work", &t) && h.m("tags.has:wor", &t) && !h.m("tags:wor", &t));
        assert!(h.m("tags.any:", &t) && !h.m("tags.none:", &t));
        assert!(h.m("tags.not:home", &t));
    }

    #[test]
    fn boolean_logic_and_precedence() {
        let h = H::new();
        let mut a = task("a");
        a.tags.insert("x".into());
        let mut b = task("b");
        b.tags.insert("y".into());
        let c = task("c");
        assert!(h.m("+x or +y", &a) && h.m("+x or +y", &b) && !h.m("+x or +y", &c));
        assert!(!h.m("+x and +y", &a));
        // and binds tighter than or: (+x and +y) or +z
        let mut z = task("z");
        z.tags.insert("z".into());
        assert!(h.m("+x +y or +z", &z) && !h.m("+x +y or +z", &a));
        assert!(h.m("(+x or +y) and not +z", &a));
        assert!(h.m("not +x", &b));
        assert!(h.m("+x xor +y", &a) && !h.m("+x xor +x", &a));
        assert!(h.m("(+x or +y) -z", &a));
        assert!(h.m("(+x)", &a)); // parens glued to the word
    }

    #[test]
    fn date_attribute_semantics() {
        let h = H::new();
        let mut t = task("x");
        t.due = Some(NOW + DAY); // tomorrow 12:30
        assert!(h.m("due:tomorrow", &t)); // same day
        assert!(!h.m("due:today", &t));
        assert!(h.m("due.before:eow", &t));
        assert!(!h.m("due.before:today", &t));
        assert!(h.m("due.after:today", &t));
        assert!(h.m("due.by:tomorrow", &t) == false); // by = <=; tomorrow 00:00 < 12:30
        assert!(h.m("due.by:2d", &t));
        assert!(h.m("due.before:now+2d", &t));
        assert!(h.m("due.after:eod-1d", &t));
        assert!(h.m("due:2026-10-08", &t));
        assert!(h.m("due:2026-10-08T12:30", &t)); // partial = same day
        // NOW is exactly 12:30:00, so tomorrow's due is exactly 2026-10-08T12:30:00.
        assert!(h.m("due.is:2026-10-08T12:30", &t));
        assert!(!h.m("due.is:2026-10-08T12:31", &t));
    }

    #[test]
    fn exact_date_is_with_time() {
        let h = H::new();
        let mut t = task("x");
        t.due = Some(NOW - NOW % 60 + DAY); // 12:30:00 tomorrow
        assert!(h.m("due.is:2026-10-08T12:30", &t));
        assert!(!h.m("due.is:2026-10-08T12:31", &t));
        assert!(h.m("due.is:2026-10-08", &t)); // date-only is: whole-day
    }

    #[test]
    fn bad_values_are_errors_not_silent_misses() {
        let h = H::new();
        assert!(h.parse_err("due:nonsense").contains("not a valid date"));
        assert!(h.parse_err("due.bogus:today").contains("unrecognized attribute modifier"));
        assert!(h.parse_err("(+a").contains("closing"));
        assert!(h.parse_err("+a)").contains("unexpected closing"));
        assert!(h.parse_err("+a and").contains("expected"));
        assert!(h.parse_err("and +a").contains("needs a term"));
        assert!(h.parse_err("limit:abc").contains("invalid limit"));
    }

    #[test]
    fn unknown_attributes_fall_back_to_words() {
        let h = H::new();
        assert!(h.m("http://example.com", &task("see http://example.com now")));
        assert!(!h.m("foo:bar", &task("nothing")));
        assert!(h.m("foo:bar", &task("a foo:bar b")));
    }

    #[test]
    fn udas_by_type() {
        let h = H::with_rc(
            "uda.estimate.type=string\nuda.estimate.values=huge,large,small\n\
             uda.points.type=numeric\nuda.shipby.type=date\n",
        );
        let mut t = task("x");
        t.extra.insert("estimate".into(), "large".into());
        t.extra.insert("points".into(), "5".into());
        t.extra.insert("shipby".into(), (NOW + DAY).to_string());
        assert!(h.m("estimate:large", &t) && h.m("estimate:lar", &t));
        assert!(h.m("estimate.is:large", &t) && !h.m("estimate.is:lar", &t));
        assert!(h.m("points:5", &t) && h.m("points.after:3", &t) && !h.m("points.after:5", &t));
        assert!(h.m("points.by:5", &t) && h.m("points.before:6", &t));
        assert!(h.m("shipby:tomorrow", &t) && h.m("shipby.before:2d", &t));
        assert!(h.m("+UDA", &t));
        assert!(h.m("estimate.any:", &t) && h.m("shipby.any:", &t));
        assert!(!h.m("estimate:", &t));
        assert!(h.parse_err("points:abc").contains("not a number"));
        // Undefined UDAs aren't attributes: the text becomes a description word.
        assert!(!h.m("unknown:large", &t));
    }

    #[test]
    fn urgency_is_filterable() {
        let h = H::new();
        let mut t = task("x");
        t.priority = Some("H".into()); // 6.0
        assert!(h.m("urgency.after:5", &t) && !h.m("urgency.after:7", &t));
    }

    #[test]
    fn uuid_prefixes_and_ids() {
        let mut h = H::new();
        let mut t = task("x");
        t.uuid = Uuid::parse_str("abcdef12-3456-7890-abcd-ef1234567890").unwrap();
        h.ids.insert(t.uuid, 7);
        assert!(h.m("abcdef12", &t) && h.m("abcdef12-3456", &t));
        assert!(h.m("abcdef12-3456-7890-abcd-ef1234567890", &t));
        assert!(!h.m("abcdef99", &t));
        assert!(h.m("7", &t) && h.m("5-9", &t) && h.m("1,7,20", &t));
        assert!(!h.m("8", &t) && !h.m("1-6", &t));
        assert!(h.m("id:7", &t) && h.m("id.after:3", &t));
        // A task with no working-set id is never selected by number.
        let other = task("y");
        assert!(!h.m("1-100", &other));
    }

    #[test]
    fn limit_directive() {
        let h = H::new();
        let ctx = h.ctx();
        assert_eq!(Filter::parse_str("limit:5", &ctx).unwrap().limit, Limit::N(5));
        assert_eq!(Filter::parse_str("limit:page +a", &ctx).unwrap().limit, Limit::Page);
        assert_eq!(Filter::parse_str("limit:none", &ctx).unwrap().limit, Limit::None);
        // A directive alone still matches everything.
        assert!(Filter::parse_str("limit:3", &ctx).unwrap().matches(&task("x"), &ctx));
    }

    #[test]
    fn quoting_and_conjoin() {
        assert_eq!(split_words(r#"project:"Home Stuff" 'a b' c"#), ["project:Home Stuff", "a b", "c"]);
        let h = H::new();
        let ctx = h.ctx();
        let combined = conjoin(&[
            split_words("status:pending"),
            vec![],
            split_words("+a or +b"),
        ]);
        let f = Filter::parse(&combined, &ctx).unwrap();
        let mut t = task("x");
        t.tags.insert("b".into());
        assert!(f.matches(&t, &ctx));
        // The `or` is fenced inside its own parentheses, so it can't leak into the status term.
        t.status = "completed".into();
        assert!(!f.matches(&t, &ctx));
        assert!(h.m("project:\"Home Stuff\"", &Facts { project: Some("Home Stuff".into()), ..task("x") }));
    }

    #[test]
    fn builtin_report_filters_parse_and_evaluate() {
        let h = H::new();
        let c = clock();
        let _ = c;
        let mut waiting = task("w");
        waiting.wait = Some(NOW + DAY);
        let normal = task("n");
        for name in crate::report::BUILTIN_NAMES {
            let r = crate::report::resolve(&h.cfg, name).unwrap();
            if let Some(f) = r.filter {
                Filter::parse_str(&f, &h.ctx()).unwrap_or_else(|e| panic!("{name}: {e}"));
            }
        }
        // `next` hides waiting tasks; `waiting` shows only them.
        assert!(h.m("status:pending -WAITING", &normal) && !h.m("status:pending -WAITING", &waiting));
        assert!(h.m("+WAITING", &waiting));
    }
}
