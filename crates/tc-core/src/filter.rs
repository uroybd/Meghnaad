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
use crate::rx::{Rx, TextMatch};
use crate::taskrc::{Config, UdaType};
use crate::urgency::{coefficients, urgency_with};
use std::borrow::Cow;
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
    /// The largest task number, which `+LATEST` is the task of.
    latest: u32,
    coef: BTreeMap<String, f64>,
    /// Urgency of blocking tasks under `urgency.inherit`; empty when that is off.
    inherited: BTreeMap<Uuid, f64>,
    /// The colour rules in force; `None` when `color` is off.
    rules: Option<crate::color::Rules>,
}

impl<'a> EvalCtx<'a> {
    pub fn new(cfg: &'a Config, clock: Clock, ids: &'a BTreeMap<Uuid, u32>) -> Self {
        EvalCtx {
            cfg,
            clock,
            ids,
            latest: ids.values().copied().max().unwrap_or(0),
            coef: coefficients(cfg),
            inherited: BTreeMap::new(),
            rules: crate::color::Rules::from_config(cfg),
        }
    }

    /// The colour the rules give a task, or none.
    pub fn style(&self, f: &Facts) -> Option<crate::color::Style> {
        self.rules.as_ref()?.style_for(f, self.cfg, &self.clock)
    }

    /// The colour rules, when colour is on.
    pub fn rules(&self) -> Option<&crate::color::Rules> {
        self.rules.as_ref()
    }

    /// Apply `urgency.inherit` (when the taskrc turns it on) using every task, since a task's
    /// urgency then depends on the tasks it blocks.
    pub fn with_inheritance(mut self, all: &[Facts]) -> Self {
        if self.cfg.urgency_inherit() {
            self.inherited = crate::urgency::inherited(all, self.cfg, &self.clock, &self.coef);
        }
        self
    }

    pub fn urgency(&self, f: &Facts) -> f64 {
        match self.inherited.get(&f.uuid) {
            Some(v) => *v,
            None => urgency_with(f, self.cfg, &self.clock, &self.coef),
        }
    }

    /// `regex` (on by default): whether text matching uses regular expressions.
    pub fn regex_enabled(&self) -> bool {
        self.cfg.regex_enabled()
    }

    /// `search.case.sensitive` (on by default).
    pub fn case_sensitive(&self) -> bool {
        self.cfg
            .settings
            .get("search.case.sensitive")
            .is_none_or(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "on" | "yes" | "y" | "true"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    None,
    /// `limit:page`: a terminal-height concept; the web UI treats it as "no limit".
    Page,
    N(usize),
}

/// The value of a `limit:` word or of the `limit` setting: `none` or `0` (unlimited), `page`, or a
/// number of tasks. `None` if it is none of these.
pub fn parse_limit(v: &str) -> Option<Limit> {
    match v {
        "none" | "0" => Some(Limit::None),
        "page" => Some(Limit::Page),
        n => n.parse().ok().map(Limit::N),
    }
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
    Mask,
    Imask,
    Rtype,
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
    ("mask", Core::Mask),
    ("imask", Core::Imask),
    ("rtype", Core::Rtype),
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
                Core::Id | Core::Urgency | Core::Imask => Ty::Num,
                Core::Entry
                | Core::Start
                | Core::End
                | Core::Due
                | Core::Wait
                | Core::Scheduled
                | Core::Until
                | Core::Modified => Ty::Date,
                Core::Depends | Core::Annotation => Ty::Many,
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

/// Resolve an attribute name, allowing unique abbreviations of at least `abbreviation.minimum`
/// characters (`desc`, `proj`), as Taskwarrior does.
fn canonicalize(name: &str, cfg: &Config) -> Option<Attr> {
    let mut all: Vec<(String, Attr)> = CORE_NAMES
        .iter()
        .map(|(n, c)| ((*n).to_owned(), Attr::Core(*c)))
        .collect();
    all.extend(
        cfg.udas
            .values()
            .map(|u| (u.name.clone(), Attr::Uda(u.name.clone(), u.ty))),
    );
    if let Some((_, a)) = all.iter().find(|(n, _)| n == name) {
        return Some(a.clone());
    }
    // Shorter than `abbreviation.minimum` it is not an abbreviation of anything.
    if name.len() < cfg.abbreviation_minimum().max(1) {
        return None;
    }
    let mut hits = all.iter().filter(|(n, _)| n.starts_with(name));
    match (hits.next(), hits.next()) {
        (Some((_, a)), None) => Some(a.clone()),
        _ => None,
    }
}

/// Whether the word is the whole name of an attribute (a UDA too), not an abbreviation of one.
pub(crate) fn is_attribute_name(name: &str, cfg: &Config) -> bool {
    CORE_NAMES.iter().any(|(n, _)| *n == name) || cfg.udas.values().any(|u| u.name == name)
}

/// Canonical attribute name for `name` (unique abbreviations allowed), and whether it is a UDA.
pub(crate) fn canonical_attr_name(name: &str, cfg: &Config) -> Option<(String, bool)> {
    canonicalize(name, cfg).map(|a| match a {
        Attr::Uda(n, _) => (n, true),
        Attr::Core(c) => (
            CORE_NAMES
                .iter()
                .find(|(_, k)| *k == c)
                .map(|(n, _)| (*n).to_owned())
                .unwrap_or_default(),
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
    /// `<`, `<=`, `>` and `>=` between an attribute and a value; there is no dotted modifier for these.
    Lt,
    Le,
    Gt,
    Ge,
}

/// The comparison operators Taskwarrior reads between an attribute and a value (`due < eom`, `urgency>=5`,
/// `project !~ ^Work`), each as the `Op` an `attribute.modifier:value` pair would use. `=`, `==`, `!=` and `!==` are
/// partial equal, equal, not partial equal and not equal; `~` and `!~` are the regular expression match.
const INFIX: &[(&str, Op)] = &[
    ("!==", Op::Isnt),
    ("==", Op::Is),
    ("!=", Op::Not),
    ("<=", Op::Le),
    (">=", Op::Ge),
    ("!~", Op::Hasnt),
    ("<", Op::Lt),
    (">", Op::Gt),
    ("~", Op::Has),
    ("=", Op::Partial),
];

/// A value that begins with an arithmetic operator is not a value to Taskwarrior's lexer: `description ~ ^x` is
/// `description ^ x` and then a `~` with nothing to its left, and the `^` fails first, on what the attribute holds.
fn operator_led(attr: &Attr, value: &str) -> Result<(), FilterError> {
    let Some(sign) = value.chars().next().filter(|c| matches!(c, '^' | '*' | '%')) else {
        return Ok(());
    };
    let rest = &value[1..];
    let unsolved = "The expression could not be evaluated.";
    err(match (attr.ty(), sign) {
        (Ty::Date, '^') => "Cannot exponentiate dates",
        (Ty::Date, '*') => "Cannot multiply date values",
        (Ty::Date, _) => "Cannot modulo date values",
        (Ty::Num, '^') if rest.parse::<f64>().is_err() && !rest.is_empty() => "Cannot exponentiate strings",
        (Ty::Num, _) => unsolved,
        (_, '^') => "Cannot exponentiate strings",
        (_, '*') => "Cannot multiply strings by strings",
        (_, _) => "Cannot modulo string values",
    })
}

/// Whether the word is one of the comparison operators, standing apart from the names it compares.
pub(crate) fn is_infix_op(word: &str) -> bool {
    infix_op(word).is_some()
}

fn infix_op(word: &str) -> Option<Op> {
    INFIX.iter().find(|(w, _)| *w == word).map(|(_, op)| *op)
}

/// `due<2026-10-28`, `project!=Work`: an attribute name, a comparison operator and a value in one word. `=` and `==`
/// stay with `split_pair`, which reads `project=Work` as `project:Work` (and `project==Work` as the value `=Work`, as
/// Taskwarrior's lexer does).
fn split_infix(tok: &str) -> Option<(&str, Op, &str)> {
    let name_end = tok.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
    let (name, rest) = tok.split_at(name_end);
    if name.is_empty() {
        return None;
    }
    INFIX
        .iter()
        .filter(|(w, _)| !matches!(*w, "=" | "=="))
        .find_map(|(w, op)| rest.strip_prefix(w).map(|value| (name, *op, value)))
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
    Tag {
        name: String,
        present: bool,
    },
    /// An attribute's name standing alone beside a `!` (`priority !`): what it holds, as true or false.
    Truth(Attr),
    Attr {
        attr: Attr,
        op: Op,
        value: String,
        date: Option<i64>,
        num: Option<f64>,
        /// The pattern behind `.has`, `.startswith`, `.endswith`, `.word` and their negations, when
        /// `regex` is on. Taskwarrior turns each into a `~` match: `^value`, `value$`, `\bvalue\b`.
        rx: Option<Rx>,
    },
    /// A plain word or `/pattern/`; found in the description or in any annotation.
    Text(TextMatch),
    Uuid(String),
    Ids(Vec<(u32, u32)>),
}

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    All,
    Term(Term),
    Not(Box<Expr>),
    /// A term with `!` before it and no parenthesis: Taskwarrior's `!` takes the one value next to it, so
    /// `! project:Work` is `(!project) = 'Work'`, the truth of the project compared with the word. The number is how
    /// many `!` there are.
    Bang(u8, Term),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Xor(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    expr: Expr,
    pub limit: Limit,
    /// A `limit:` word was given. Without one, a report falls back to the `limit` setting.
    pub limit_set: bool,
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
                cur.extend(chars.next());
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

/// Peel `(` and `)` off the edges of words so `(due:today` and `+a)` become separate tokens. An argument with spaces in
/// it, such as `"+a or (due:today and +b)"` in quotes, is a whole filter and is read again as one, as Taskwarrior does;
/// unless it starts with a `name:value`, which then takes all of it (`project:Home and +a` is one project, as it is there), or is
/// one `/pattern/`.
fn tokenize(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        let first = a.split_whitespace().next().unwrap_or_default();
        let pattern = a.len() > 1 && a.starts_with('/') && a.ends_with('/');
        if a.contains(char::is_whitespace) && split_pair(first).is_none() && !pattern {
            out.extend(peel(&split_words(a)));
        } else {
            out.extend(peel(std::slice::from_ref(a)));
        }
    }
    out
}

/// `true` as a filter that selects everything, `false` as one that selects nothing.
fn constant(truth: bool) -> Expr {
    if truth {
        Expr::All
    } else {
        Expr::Not(Box::new(Expr::All))
    }
}

/// Whether Taskwarrior counts a word as true: it is not empty, `0` or `0.0`.
fn truth_of_word(word: &str) -> bool {
    !(word.is_empty() || word == "0" || word == "0.0")
}

const MISMATCHED: &str = "Mismatched parentheses in expression";

/// A word that stands for something Taskwarrior cannot work out.
const BROKEN: &str = "\0";

fn peel(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for a in args {
        let mut s = a.as_str();
        let mut tail = 0;
        // `!(+a or +b)`: the `!` stands apart from the parenthesis, as it does in Taskwarrior.
        // A `!` stands apart from what follows unless it begins `!=`, `!==` or `!~`.
        while let Some(r) = s.strip_prefix('!').filter(|r| !r.starts_with(['=', '~'])) {
            out.push("!".to_owned());
            s = r;
            // `!+a` is no tag: with nothing between them the `+` is an operator, and the filter cannot be worked out.
            if s.len() > 1 && s.starts_with(['+', '-']) && s[1..].starts_with(|c: char| c.is_alphabetic()) {
                out.push(BROKEN.to_owned());
                s = "";
            }
        }
        while let Some(r) = s.strip_prefix('(') {
            out.push("(".to_owned());
            s = r;
        }
        if s.starts_with('/') || split_pair(s).is_some() {
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
        } else {
            // Taskwarrior's lexer breaks any other word at every parenthesis: `not(+a)` is `not`, `(`, `+a`, `)`.
            let mut word = String::new();
            for c in s.chars() {
                if matches!(c, '(' | ')') {
                    if !word.is_empty() {
                        out.push(std::mem::take(&mut word));
                    }
                    out.push(c.to_string());
                } else {
                    word.push(c);
                }
            }
            if !word.is_empty() {
                out.push(word);
            }
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
pub(crate) fn split_pair(tok: &str) -> Option<(&str, &str, &str)> {
    let sep = tok.find([':', '='])?;
    let (lhs, value) = (&tok[..sep], &tok[sep + 1..]);
    let (name, modifier) = lhs.split_once('.').unwrap_or((lhs, ""));
    let ident = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
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
    limit_set: bool,
    /// How many `!` stood after the term being read again (`+a !`): the value in it is made the truth of itself (see
    /// [`Parser::trailing_bang`]).
    value_not: u8,
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
        let mut last_start = self.pos;
        let mut parts = vec![self.unary()?];
        let mut explicit = false;
        // A `!` after a term was the last thing read: no term may follow it without an `and` or `or`.
        let mut banged = false;
        loop {
            match self.peek() {
                None | Some(")") => break,
                Some(t) if Self::is_or(t) => break,
                Some("and" | "&&") => {
                    self.pos += 1;
                    explicit = true;
                    banged = false;
                }
                // A `!` after a term belongs to the term (`+a !`).
                Some("!") if parts.last().is_some_and(|p| *p != Expr::All) => {
                    let e = self.trailing_bang(last_start, parts.pop().unwrap_or(Expr::All))?;
                    parts.push(e);
                    banged = true;
                    continue;
                }
                // No `and` is put before a `!`, and then two values are left with nothing between them.
                // (An override of a setting before it is no value.)
                Some("!") if parts.iter().any(|p| *p != Expr::All) => return err("The value is not an expression."),
                Some(_) if banged => return err("The value is not an expression."),
                Some(_) => {} // implicit and
            }
            last_start = self.pos;
            parts.push(self.unary()?);
        }
        // Task numbers and uuids named side by side pick a group, as in Taskwarrior: `task 1 3 done` is task 1 or
        // task 3, and anything else in the filter narrows that group (`1 3 +work`). With an explicit `and` they are
        // taken as written.
        if !explicit {
            let (ids, mut rest): (Vec<Expr>, Vec<Expr>) = parts
                .into_iter()
                .partition(|e| matches!(e, Expr::Term(Term::Ids(_) | Term::Uuid(_))));
            if let Some(group) = ids.into_iter().reduce(|a, b| Expr::Or(Box::new(a), Box::new(b))) {
                rest.push(group);
            }
            parts = rest;
        }
        let first = parts.remove(0);
        Ok(parts
            .into_iter()
            .fold(first, |left, right| Expr::And(Box::new(left), Box::new(right))))
    }

    /// `!` after a term, written `+a !` or `project:Work !`. Taskwarrior's `!` takes the one value before it as well as
    /// the one after it, so what is turned into its truth is the *value* in the term (`+a !` is `+a` with the tag
    /// `false`, `project:Work !` is `project:false`). A group in parentheses, a task number or a uuid is a whole
    /// expression of its own, and `!` reverses it. `last` is what was read from `start`.
    fn trailing_bang(&mut self, start: usize, last: Expr) -> Result<Expr, FilterError> {
        let after = self.pos;
        let mut bangs = 0u8;
        while self.peek() == Some("!") {
            self.pos += 1;
            bangs = bangs.saturating_add(1);
        }
        let end = self.pos;
        let head = self.toks.get(start).map_or("", String::as_str);
        let whole = head == "(" || matches!(last, Expr::Term(Term::Ids(_) | Term::Uuid(_)));
        if whole {
            return Ok((0..bangs).fold(last, |e, _| Expr::Not(Box::new(e))));
        }
        if head == "!" || head == BROKEN {
            return err("The value is not an expression.");
        }
        self.pos = start;
        self.value_not = bangs;
        let again = self.unary();
        self.value_not = 0;
        let e = again?;
        if self.pos != after {
            return err("The value is not an expression.");
        }
        self.pos = end;
        Ok(e)
    }

    fn unary(&mut self) -> Result<Expr, FilterError> {
        let mut bangs = 0u8;
        while self.peek() == Some("!") {
            self.pos += 1;
            bangs = bangs.saturating_add(1);
        }
        if bangs == 0 {
            return self.primary();
        }
        // Before a parenthesis `!` reverses the whole group. Before anything else it takes only that one value.
        let Some(next) = self.peek().map(str::to_owned) else {
            return err("The expression could not be evaluated.");
        };
        let mut e = if next == "(" {
            self.primary()?
        } else {
            if matches!(next.as_str(), ")" | "and" | "or" | "xor" | "&&" | "||" | BROKEN) {
                return err("The expression could not be evaluated.");
            }
            self.pos += 1;
            let term = match self.infix(&next)? {
                Some(t) => Some(t),
                None => match canonicalize(&next, self.ctx.cfg) {
                    Some(attr) => Some(Term::Truth(attr)),
                    None => self.term(&next)?,
                },
            };
            match term {
                None => Expr::All,
                // `+tag` is `tags _hastag_ tag`, which looks at the task and not at what `!` made of `tags`.
                Some(t @ Term::Tag { .. }) => return Ok(Expr::Term(t)),
                // A task number or uuid stands in a parenthesis of its own.
                // A word that is no pattern and no attribute (`! alpha`) is a value of its own: the truth of the word.
                // So is a task number that follows an operator (`! 1`), which is a number there and not a task.
                Some(Term::Text(_)) if !next.starts_with('/') => {
                    return Ok(constant(truth_of_word(&next) != (bangs % 2 == 1)))
                }
                Some(Term::Ids(ranges)) if next.bytes().all(|b| b.is_ascii_digit()) && ranges.len() == 1 => {
                    return Ok(constant(truth_of_word(&next) != (bangs % 2 == 1)));
                }
                Some(t @ (Term::Ids(_) | Term::Uuid(_) | Term::Truth(_))) => {
                    let mut e = Expr::Term(t);
                    for _ in 0..bangs {
                        e = Expr::Not(Box::new(e));
                    }
                    return Ok(e);
                }
                Some(t) => return Ok(Expr::Bang(bangs, t)),
            }
        };
        for _ in 0..bangs {
            e = Expr::Not(Box::new(e));
        }
        Ok(e)
    }

    fn primary(&mut self) -> Result<Expr, FilterError> {
        let Some(tok) = self.peek().map(str::to_owned) else {
            return err("filter ended where a term was expected");
        };
        self.pos += 1;
        match tok.as_str() {
            "(" => {
                if self.peek() == Some(")") {
                    return err("No expression to evaluate.");
                }
                let e = self.expr()?;
                if self.peek() != Some(")") {
                    return err(MISMATCHED);
                }
                self.pos += 1;
                Ok(e)
            }
            ")" => err(MISMATCHED),
            BROKEN | "and" | "or" | "xor" | "&&" | "||" => err("The expression could not be evaluated."),
            _ => {
                if let Some(t) = self.infix(&tok)? {
                    return Ok(self.wrap(t));
                }
                if self.value_not > 0 {
                    if let Some(attr) = canonicalize(&tok, self.ctx.cfg) {
                        let truth = Expr::Term(Term::Truth(attr));
                        return Ok((0..self.value_not).fold(truth, |e, _| Expr::Not(Box::new(e))));
                    }
                }
                Ok(match self.term(&tok)? {
                    // `x !`: the word is a value of its own, and the truth of it is what is left.
                    Some(Term::Text(_)) if self.value_not > 0 && !tok.starts_with('/') => {
                        constant(truth_of_word(&tok) != (self.value_not % 2 == 1))
                    }
                    Some(t) => self.wrap(t),
                    None => Expr::All,
                })
            }
        }
    }

    /// A term as an expression. After `!`, `.any:` and `.none:` have the truth of their empty value to compare with,
    /// which every task has and none has.
    fn wrap(&self, term: Term) -> Expr {
        match term {
            Term::Attr { op: Op::Any, .. } if self.value_not > 0 => Expr::All,
            Term::Attr { op: Op::None, .. } if self.value_not > 0 => constant(false),
            t => Expr::Term(t),
        }
    }

    /// An attribute, a comparison operator and a value, written `due<eom` or `due < eom`. Anything else is not one.
    fn infix(&mut self, tok: &str) -> Result<Option<Term>, FilterError> {
        let ctx = self.ctx;
        let missing = || err("The expression could not be evaluated.");
        if let Some((name, op, value)) = split_infix(tok) {
            let Some(attr) = canonicalize(name, ctx.cfg) else {
                return Ok(None);
            };
            let value = if value.is_empty() {
                let Some(next) = self.peek().map(str::to_owned) else {
                    return missing();
                };
                self.pos += 1;
                next
            } else {
                value.to_owned()
            };
            operator_led(&attr, &value)?;
            return self.attr_term(attr, op, &value).map(Some);
        }
        let Some(op) = self.peek().and_then(infix_op) else {
            return Ok(None);
        };
        let Some(attr) = canonicalize(tok, ctx.cfg) else {
            return Ok(None);
        };
        // Written apart (`tags != a`), `tags` is a list in the expression, and Taskwarrior cannot compare a list.
        if attr == Attr::Core(Core::Tags) {
            return missing();
        }
        let Some(value) = self.toks.get(self.pos + 1).cloned() else {
            return missing();
        };
        operator_led(&attr, &value)?;
        self.pos += 2;
        self.attr_term(attr, op, &value).map(Some)
    }

    /// `Ok(None)` for directives that match everything (`limit:`).
    fn term(&mut self, tok: &str) -> Result<Option<Term>, FilterError> {
        let ctx = self.ctx;
        if let Some(v) = tok.strip_prefix("limit:") {
            self.limit = parse_limit(v).ok_or_else(|| FilterError(format!("invalid limit '{v}'")))?;
            self.limit_set = true;
            return Ok(None);
        }
        // rc overrides don't belong to the filter.
        if tok.starts_with("rc.") || tok.starts_with("rc:") {
            return Ok(None);
        }

        // The lexer's tag: a sign, a letter (or one of `@ # $ _`, or anything beyond ASCII), then anything up to a space.
        let tag_word = |name: &str| {
            let mut chars = name.chars();
            chars
                .next()
                .is_some_and(|c| !c.is_ascii() || c.is_ascii_alphabetic() || matches!(c, '@' | '#' | '$' | '_'))
                && !name.contains(char::is_whitespace)
        };
        // After `+a !` the tag is the truth of the word: `false`.
        let tag_name = |name: &str| match self.value_not {
            0 => name.to_owned(),
            n => (truth_of_word(name) != (n % 2 == 1)).to_string(),
        };
        if let Some(name) = tok.strip_prefix('+') {
            if tag_word(name) {
                return Ok(Some(Term::Tag {
                    name: tag_name(name),
                    present: true,
                }));
            }
        }
        if let Some(name) = tok.strip_prefix('-') {
            if tag_word(name) {
                return Ok(Some(Term::Tag {
                    name: tag_name(name),
                    present: false,
                }));
            }
        }

        if let Some((name, modifier, value)) = split_pair(tok) {
            if let Some(attr) = canonicalize(name, ctx.cfg) {
                let op = parse_modifier(modifier)?;
                return self.attr_term(attr, op, value).map(Some);
            }
        }

        if tok.len() > 2 && tok.starts_with('/') && tok.ends_with('/') {
            let pattern = &tok[1..tok.len() - 1];
            // `/x/ !` is `description ~ x` with the truth of `x`.
            if self.value_not > 0 {
                return self
                    .attr_term(Attr::Core(Core::Description), Op::Has, pattern)
                    .map(Some);
            }
            return self.text(pattern).map(|m| Some(Term::Text(m)));
        }
        if is_uuidish(tok) {
            return Ok(Some(Term::Uuid(tok.to_ascii_lowercase())));
        }
        if let Some(ids) = parse_ids(tok) {
            return Ok(Some(Term::Ids(ids)));
        }
        self.text(tok).map(|m| Some(Term::Text(m)))
    }

    /// Plain text from the filter, as a pattern or not according to the `regex` setting.
    fn text(&self, text: &str) -> Result<TextMatch, FilterError> {
        TextMatch::new(text, self.ctx.regex_enabled(), self.ctx.case_sensitive())
            .map_err(|why| FilterError(format!("'{text}' is not a valid regular expression: {why}")))
    }

    fn attr_term(&self, attr: Attr, op: Op, value: &str) -> Result<Term, FilterError> {
        let (mut date, mut num) = (None, None);
        // `project:Work !`: the value is its own truth, as the word `false` or the number 0 that the attribute is
        // compared with.
        let truth = (self.value_not > 0 && !matches!(op, Op::None | Op::Any))
            .then(|| truth_of_word(value) != (self.value_not % 2 == 1));
        let value = &truth.map_or_else(|| value.to_owned(), |t| t.to_string());
        // The value is read as a date only where the date itself is compared: `.has` and its kin take text.
        let text_op = matches!(
            op,
            Op::Has | Op::Hasnt | Op::StartsWith | Op::EndsWith | Op::Word | Op::NoWord
        );
        let needs_value = !matches!(op, Op::None | Op::Any) && !(text_op && attr.ty() == Ty::Date);
        if needs_value && !value.is_empty() {
            match attr.ty() {
                Ty::Date if truth.is_some() => date = Some(i64::from(truth == Some(true))),
                Ty::Num if truth.is_some() => num = Some(f64::from(u8::from(truth == Some(true)))),
                Ty::Date => match parse_date_expr(value, &self.ctx.clock) {
                    Some(p) => date = Some(p.ts),
                    None => return err(format!("'{value}' is not a valid date")),
                },
                Ty::Num => match value.parse::<f64>() {
                    Ok(n) => num = Some(n),
                    Err(_) => return err(format!("'{value}' is not a number")),
                },
                _ => {}
            }
        }
        // With `regex` on, Taskwarrior rewrites these as a `~` match with the value used as a pattern.
        let pattern = match op {
            Op::Has | Op::Hasnt => Some(value.to_owned()),
            Op::StartsWith => Some(format!("^{value}")),
            Op::EndsWith => Some(format!("{value}$")),
            Op::Word | Op::NoWord => Some(format!("\\b{value}\\b")),
            _ => None,
        };
        let rx = match pattern {
            Some(p) if self.ctx.regex_enabled() && attr.ty() != Ty::Num => Some(
                Rx::new(&p, self.ctx.case_sensitive())
                    .map_err(|why| FilterError(format!("'{value}' is not a valid regular expression: {why}")))?,
            ),
            _ => None,
        };
        Ok(Term::Attr {
            attr,
            op,
            value: value.to_owned(),
            date,
            num,
            rx,
        })
    }
}

impl Filter {
    pub fn match_all() -> Filter {
        Filter {
            expr: Expr::All,
            limit: Limit::None,
            limit_set: false,
        }
    }

    /// Parse already-split arguments (e.g. from the command-line parser).
    pub fn parse(args: &[String], ctx: &EvalCtx) -> Result<Filter, FilterError> {
        let toks = tokenize(args);
        if toks.is_empty() {
            return Ok(Filter::match_all());
        }
        let mut p = Parser {
            toks,
            pos: 0,
            ctx,
            limit: Limit::None,
            limit_set: false,
            value_not: 0,
        };
        let expr = p.expr()?;
        if p.pos < p.toks.len() {
            return if p.toks[p.pos] == ")" {
                err(MISMATCHED)
            } else {
                err(format!("unexpected '{}' in filter", p.toks[p.pos]))
            };
        }
        Ok(Filter {
            expr,
            limit: p.limit,
            limit_set: p.limit_set,
        })
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
        Expr::Bang(n, t) => eval_bang(*n, t, f, ctx),
        Expr::And(a, b) => eval(a, f, ctx) && eval(b, f, ctx),
        Expr::Or(a, b) => eval(a, f, ctx) || eval(b, f, ctx),
        Expr::Xor(a, b) => eval(a, f, ctx) != eval(b, f, ctx),
        Expr::Term(t) => eval_term(t, f, ctx),
    }
}

/// Whether `needle` is in `hay` when case does not matter. Plain ASCII on both sides (nearly always) needs no lowercase
/// copy of either; anything else is lowercased as Unicode, where one letter can become several.
fn contains(hay: &str, needle: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        hay.contains(needle)
    } else if hay.is_ascii() && needle.is_ascii() {
        let (h, n) = (hay.as_bytes(), needle.as_bytes());
        n.is_empty() || h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n))
    } else {
        hay.to_lowercase().contains(&needle.to_lowercase())
    }
}

/// `starts_with` (or `ends_with`, when `end`) when case does not matter; see [`contains`].
fn edge_matches(hay: &str, needle: &str, end: bool) -> bool {
    if hay.is_ascii() && needle.is_ascii() {
        let (h, n) = (hay.as_bytes(), needle.as_bytes());
        return h.len() >= n.len()
            && if end {
                h[h.len() - n.len()..].eq_ignore_ascii_case(n)
            } else {
                h[..n.len()].eq_ignore_ascii_case(n)
            };
    }
    let (h, n) = (hay.to_lowercase(), needle.to_lowercase());
    if end {
        h.ends_with(&n)
    } else {
        h.starts_with(&n)
    }
}

fn eval_term(t: &Term, f: &Facts, ctx: &EvalCtx) -> bool {
    match t {
        Term::Truth(attr) => truthy(&attr_value(attr, f, ctx)),
        Term::Tag { name, present } => {
            let has = if name == "LATEST" {
                ctx.ids.get(&f.uuid).is_some_and(|id| *id == ctx.latest)
            } else {
                f.has_tag(name, ctx.cfg, &ctx.clock)
            };
            has == *present
        }
        Term::Text(m) => m.is_match(&f.description) || f.annotations.iter().any(|a| m.is_match(&a.text)),
        Term::Uuid(prefix) => {
            let (mut simple, mut hyphen) = ([0u8; 32], [0u8; 36]);
            f.uuid.simple().encode_lower(&mut simple).starts_with(prefix.as_str())
                || f.uuid
                    .hyphenated()
                    .encode_lower(&mut hyphen)
                    .starts_with(prefix.as_str())
        }
        Term::Ids(ranges) => ctx
            .ids
            .get(&f.uuid)
            .is_some_and(|id| ranges.iter().any(|(lo, hi)| id >= lo && id <= hi)),
        Term::Attr {
            attr,
            op,
            value,
            date,
            num,
            rx,
        } => eval_attr(attr, *op, value, *date, *num, rx.as_ref(), f, ctx),
    }
}

/// An attribute's value, borrowed from the task where it can be (a filter looks at every task).
enum Val<'a> {
    Unset,
    Str(Cow<'a, str>),
    Num(f64),
    Date(i64),
    Many(Vec<Cow<'a, str>>),
}

fn attr_value<'a>(attr: &Attr, f: &'a Facts, ctx: &EvalCtx) -> Val<'a> {
    let s = |o: &'a Option<String>| o.as_deref().map_or(Val::Unset, |v| Val::Str(Cow::Borrowed(v)));
    let d = |o: Option<i64>| o.map_or(Val::Unset, Val::Date);
    match attr {
        Attr::Core(c) => match c {
            Core::Id => ctx.ids.get(&f.uuid).map_or(Val::Unset, |i| Val::Num(f64::from(*i))),
            Core::Uuid => Val::Str(Cow::Owned(f.uuid.to_string())),
            Core::Status => Val::Str(Cow::Borrowed(f.status.as_str())),
            Core::Description => Val::Str(Cow::Borrowed(f.description.as_str())),
            Core::Project => s(&f.project),
            Core::Priority => s(&f.priority),
            Core::Recur => s(&f.recur),
            Core::Parent => f.parent.map_or(Val::Unset, |p| Val::Str(Cow::Owned(p.to_string()))),
            Core::Mask => s(&f.mask),
            // A number that every task has: 0 when there is none, so `imask:0` also finds a template and a plain task,
            // and `imask.any:` finds all, as in Taskwarrior.
            Core::Imask => Val::Num(f.imask.unwrap_or(0) as f64),
            Core::Rtype => s(&f.rtype),
            Core::Entry => d(f.entry),
            Core::Start => d(f.start),
            Core::End => d(f.end),
            Core::Due => d(f.due),
            Core::Wait => d(f.wait),
            Core::Scheduled => d(f.scheduled),
            Core::Until => d(f.until),
            Core::Modified => d(f.modified),
            Core::Urgency => Val::Num(ctx.urgency(f)),
            // The tags are one string, sorted and joined by commas, as Taskwarrior reads them: `tags:a` is the start of it,
            // and `tags.is:a` is a task with that one tag and no other. `+a` is how to ask for a tag.
            Core::Tags if f.tags.is_empty() => Val::Unset,
            Core::Tags => Val::Str(Cow::Owned(
                f.tags.iter().map(String::as_str).collect::<Vec<_>>().join(","),
            )),
            Core::Depends => Val::Many(f.depends.iter().map(|d| Cow::Owned(d.to_string())).collect()),
            Core::Annotation => Val::Many(f.annotations.iter().map(|a| Cow::Borrowed(a.text.as_str())).collect()),
        },
        Attr::Uda(name, ty) => match f.extra.get(name) {
            None => Val::Unset,
            Some(raw) => match ty {
                UdaType::Numeric => raw.parse().map_or(Val::Unset, Val::Num),
                UdaType::Date => raw.parse().map_or(Val::Unset, Val::Date),
                _ => Val::Str(Cow::Borrowed(raw.as_str())),
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
        let before_ok = h[..start].chars().next_back().is_none_or(|c| !is_w(c));
        let after_ok = h[end..].chars().next().is_none_or(|c| !is_w(c));
        if before_ok && after_ok {
            return true;
        }
        from = start + h[start..].chars().next().map_or(1, char::len_utf8);
    }
    false
}

fn str_op(have: &str, op: Op, want: &str, status: bool, cs: bool) -> bool {
    let eq = |a: &str, b: &str| {
        if status || !cs {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
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
            if cs {
                have.starts_with(want)
            } else {
                edge_matches(have, want, false)
            }
        }
        Op::EndsWith => {
            if cs {
                have.ends_with(want)
            } else {
                edge_matches(have, want, true)
            }
        }
        Op::Word => is_word_in(have, want, cs),
        Op::NoWord => !is_word_in(have, want, cs),
        Op::Before => have < want,
        Op::After => have > want,
        Op::By => have <= want,
        Op::Lt | Op::Le | Op::Gt | Op::Ge => holds(
            op,
            if cs {
                have.cmp(want)
            } else {
                have.to_lowercase().cmp(&want.to_lowercase())
            },
        ),
        Op::None | Op::Any => unreachable!("handled by caller"),
    }
}

/// Whether `op` (`<`, `<=`, `>`, `>=`) holds when the attribute's value compares to the wanted one as `order`.
fn holds(op: Op, order: std::cmp::Ordering) -> bool {
    use std::cmp::Ordering::{Greater, Less};
    match op {
        Op::Lt => order == Less,
        Op::Le => order != Greater,
        Op::Gt => order == Greater,
        Op::Ge => order != Less,
        _ => false,
    }
}

/// The values a string attribute may take, highest first, when they have an order of their own: `priority` (H, M, L
/// unless the taskrc redefines it) and a UDA with a `values` list. A comparison then follows that order, as
/// `priority > L` finding H and M, and a task with no value is below all of them.
fn value_order(attr: &Attr, cfg: &Config) -> Option<Vec<String>> {
    let name = match attr {
        Attr::Core(Core::Priority) => "priority",
        Attr::Uda(name, UdaType::String) => name.as_str(),
        _ => return None,
    };
    match cfg.udas.get(name) {
        Some(d) if !d.values.is_empty() => Some(d.values.clone()),
        None if name == "priority" => Some(["H", "M", "L"].map(String::from).to_vec()),
        _ => None,
    }
}

/// Where `value` stands in `order`, 0 for none (or one that is not listed) and more for each step up.
fn rank(order: &[String], value: Option<&str>) -> usize {
    value
        .and_then(|v| order.iter().rev().position(|o| o == v))
        .map_or(0, |i| i + 1)
}

/// Whether a value counts as true when Taskwarrior casts it to a boolean: not empty, not `0` or `0.0`, not zero.
fn truthy(v: &Val) -> bool {
    match v {
        Val::Unset => false,
        Val::Str(s) => !(s.is_empty() || s == "0" || s == "0.0"),
        Val::Num(n) => *n != 0.0,
        Val::Date(d) => *d != 0,
        Val::Many(items) => !items.is_empty(),
    }
}

/// A term with `!` right before it (see [`Expr::Bang`]). What is compared is the *truth* of the attribute, `true` or
/// `false`, turned into the other side's kind the way Taskwarrior's `Variant` does: the word `true` or `false` for a
/// string, 1 or 0 for a number or a date.
fn eval_bang(bangs: u8, term: &Term, f: &Facts, ctx: &EvalCtx) -> bool {
    let negate = bangs % 2 == 1;
    match term {
        Term::Text(m) => {
            let said = truthy(&Val::Str(Cow::Borrowed(&f.description))) != negate;
            m.is_match(if said { "true" } else { "false" })
        }
        Term::Attr {
            attr,
            op,
            value,
            date,
            num,
            rx,
        } => {
            let b = truthy(&attr_value(attr, f, ctx)) != negate;
            let (word, n) = (if b { "true" } else { "false" }, i64::from(b));
            let cs = ctx.case_sensitive();
            let order = |op: Op| match attr.ty() {
                Ty::Num => num.map(|w| holds(op, (n as f64).total_cmp(&w))),
                Ty::Date => date.map(|w| holds(op, n.cmp(&w))),
                Ty::Str | Ty::Many => Some(holds(op, word.cmp(value.as_str()))),
            };
            let equal = |partial: bool| match attr.ty() {
                Ty::Num => num.is_some_and(|w| n as f64 == w),
                Ty::Date => date.is_some_and(|w| if partial { ctx.clock.same_day(n, w) } else { n == w }),
                Ty::Str | Ty::Many => word == value,
            };
            let matches = |negated: bool| {
                let hit = match rx {
                    Some(rx) => rx.is_match(word),
                    None => contains(word, value, cs),
                };
                hit != negated
            };
            match op {
                Op::Partial => equal(true),
                Op::Not => !equal(true),
                Op::Is => equal(false),
                Op::Isnt => !equal(false),
                Op::Has | Op::StartsWith | Op::EndsWith | Op::Word => matches(false),
                Op::Hasnt | Op::NoWord => matches(true),
                Op::Before | Op::Lt => order(Op::Lt).unwrap_or(false),
                Op::After | Op::Gt => order(Op::Gt).unwrap_or(false),
                Op::By | Op::Le => order(Op::Le).unwrap_or(false),
                Op::Ge => order(Op::Ge).unwrap_or(false),
                // `.none:` and `.any:` are `== ''` and `!== ''`, which a `true` or `false` never equals.
                Op::None => false,
                Op::Any => true,
            }
        }
        // A tag, a task number or a uuid is not taken apart by `!`.
        other => eval_term(other, f, ctx),
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_attr(
    attr: &Attr,
    op: Op,
    value: &str,
    date: Option<i64>,
    num: Option<f64>,
    rx: Option<&Rx>,
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

    // `.has`, `.startswith` and the others on a date read it as text, `2026-10-30T00:00:00` in the clock's zone, whatever
    // `dateformat` says; a date that is not there is the empty text.
    if attr.ty() == Ty::Date
        && matches!(
            op,
            Op::Has | Op::Hasnt | Op::StartsWith | Op::EndsWith | Op::Word | Op::NoWord
        )
    {
        let text = match have {
            Val::Date(ts) => {
                let (y, mo, d, h, mi, s) = ctx.clock.ymd_hms(ts);
                format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}")
            }
            _ => String::new(),
        };
        return match rx {
            Some(rx) => rx.is_match(&text) != matches!(op, Op::Hasnt | Op::NoWord),
            None => str_op(&text, op, value, false, cs),
        };
    }

    // Text operations as patterns. On the description, annotations count too, as in Taskwarrior.
    if let Some(rx) = rx {
        let negated = matches!(op, Op::Hasnt | Op::NoWord);
        let hit = match &have {
            Val::Str(h) => {
                rx.is_match(h)
                    || (matches!(attr, Attr::Core(Core::Description))
                        && f.annotations.iter().any(|a| rx.is_match(&a.text)))
            }
            Val::Many(items) => items.iter().any(|i| rx.is_match(i)),
            _ => false,
        };
        return hit != negated;
    }

    // `status:waiting` is Taskwarrior 3's computed state, not a stored one.
    if attr.is_status() && value.eq_ignore_ascii_case("waiting") && matches!(op, Op::Partial | Op::Is) {
        return f.is_waiting(&ctx.clock);
    }

    // Nor is a waiting task pending: Taskwarrior 3 reads a pending task with a `wait` in the future as `waiting`.
    if attr.is_status()
        && value.eq_ignore_ascii_case("pending")
        && matches!(op, Op::Partial | Op::Is | Op::Not | Op::Isnt)
    {
        let pending = f.status == "pending" && !f.is_waiting(&ctx.clock);
        return pending == matches!(op, Op::Partial | Op::Is);
    }
    if matches!(op, Op::Lt | Op::Le | Op::Gt | Op::Ge) {
        if let Some(order) = value_order(attr, ctx.cfg) {
            let have = match &have {
                Val::Str(s) => Some(s.as_ref()),
                _ => None,
            };
            return holds(op, rank(&order, have).cmp(&rank(&order, Some(value))));
        }
    }

    match have {
        // After `!` the number or date is 0 or 1 and its word `false` or `true` (see `attr_term`); a task without one
        // is the empty text, which sorts before that word.
        Val::Unset if matches!(attr.ty(), Ty::Num | Ty::Date) && matches!(value, "true" | "false") => matches!(
            op,
            Op::Not | Op::Isnt | Op::Hasnt | Op::NoWord | Op::Before | Op::Lt | Op::By | Op::Le
        ),
        Val::Unset => matches!(op, Op::Not | Op::Isnt | Op::Hasnt | Op::NoWord),
        Val::Str(h) => str_op(&h, op, value, attr.is_status(), cs),
        Val::Num(h) => {
            let Some(w) = num else { return false };
            match op {
                Op::Partial | Op::Is => h == w,
                Op::Not | Op::Isnt => h != w,
                Op::Before | Op::Lt => h < w,
                Op::After | Op::Gt => h > w,
                Op::By | Op::Le => h <= w,
                Op::Ge => h >= w,
                _ => false,
            }
        }
        Val::Date(h) => {
            let Some(w) = date else { return false };
            match op {
                Op::Partial => ctx.clock.same_day(h, w),
                Op::Not => !ctx.clock.same_day(h, w),
                // `is` and `isnt` are the instant, where `:` is the day.
                Op::Is => h == w,
                Op::Isnt => h != w,
                Op::Before | Op::Lt => h < w,
                Op::After | Op::Gt => h > w,
                Op::By | Op::Le => h <= w,
                Op::Ge => h >= w,
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
            H {
                cfg: Config::default(),
                ids: BTreeMap::new(),
            }
        }

        fn with_rc(rc: &str) -> Self {
            H {
                cfg: parse(rc).config,
                ids: BTreeMap::new(),
            }
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
        Facts {
            project: Some(p.into()),
            ..task("t")
        }
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
        // A waiting task is not pending, as Taskwarrior 3 reads its status.
        assert!(!h.m("status:pending", &t) && !h.m("+PENDING", &t));
        assert!(h.m("status.not:pending", &t) && h.m("status.isnt:pending", &t));
    }

    #[test]
    fn tags_and_virtual_tags() {
        let h = H::new();
        let mut t = task("x");
        t.tags.insert("work".into());
        assert!(h.m("+work", &t) && !h.m("-work", &t) && h.m("-home", &t));
        assert!(h.m("+TAGGED +PENDING", &t));
        assert!(h.m("tags:work", &t) && h.m("tags.has:wor", &t));
        // The tags are one string, sorted and joined by commas: `:` finds its start and `.is` all of it.
        t.tags.insert("alpha".into());
        assert!(
            h.m("tags:alpha", &t) && !h.m("tags:work", &t) && h.m("tags.is:alpha,work", &t) && !h.m("tags.is:work", &t)
        );
        assert!(h.m("tags.has:work", &t) && h.m("tags.word:work", &t) && h.m("tags~alpha,w", &t));
        t.tags.remove("alpha");
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
        assert!(h.m("(+x or +y) and !(+z)", &a));
        assert!(h.m("!(+x)", &b));
        // `not` is no operator in Taskwarrior: it is a word to find in the description.
        assert!(
            !h.m("not +x", &b)
                && h.m(
                    "not +x",
                    &Facts {
                        tags: a.tags.clone(),
                        ..task("do not stop")
                    }
                )
        );
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
        assert!(!h.m("due.by:tomorrow", &t)); // by = <=; tomorrow 00:00 < 12:30
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
    fn comparison_operators_between_an_attribute_and_a_value() {
        let h = H::new();
        let mut t = task("x");
        t.due = Some(NOW + DAY); // 2026-10-08 12:30
                                 // Written together or apart, they read the same.
        for f in [
            "due<2026-10-09",
            "due < 2026-10-09",
            "due<=2026-10-08T12:30",
            "due>2026-10-08",
            "due >= 2026-10-08",
        ] {
            assert!(h.m(f, &t), "{f}");
        }
        for f in [
            "due<2026-10-08",
            "due>2026-10-09",
            "due<2026-10-08T12:30",
            "due>2026-10-08T12:30",
        ] {
            assert!(!h.m(f, &t), "{f}");
        }
        assert!(h.m("due<eom and due>=today", &t));
        // A task without the attribute is on neither side of a string or date comparison.
        assert!(!h.m("wait<2030-01-01", &t) && !h.m("project>a", &t) && !h.m("project<z", &t));
        assert!(h.m("urgency>-100", &t) && !h.m("urgency>100", &t) && h.m("urgency <= 100", &t));
        // Strings go by their bytes, as the search does.
        let p = proj("Work");
        assert!(
            h.m("project>Home", &p)
                && h.m("project >= Work", &p)
                && !h.m("project<Work", &p)
                && h.m("project<=Work", &p)
        );
        assert!(h.m("project<work", &p) && !h.m("project>work", &p));
        // The equals family: `=` is the partial match `:` is, `==` and `!==` the whole value, `!=` the partial match's other side.
        assert!(h.m("project = Wor", &p) && h.m("project == Work", &p) && !h.m("project == Wor", &p));
        assert!(h.m("project != Home", &p) && !h.m("project != Wor", &p) && h.m("project !== Wor", &p));
        assert!(h.m("project!=Home", &p) && h.m("project!==Wor", &p) && !h.m("project!==Work", &p));
        // `~` and `!~` take a regular expression.
        assert!(h.m("project ~ Wo", &p) && h.m("project~rk$", &p) && !h.m("project ~ zz", &p));
        assert!(h.m("project !~ zz", &p) && h.m("project!~zz", &p) && !h.m("project !~ Wo", &p));
        assert!(h.m("description ~ x", &t) && !h.m("description !~ x", &t));
        // Nothing to compare with is an error, not a filter that selects nothing.
        assert!(h.parse_err("due <").contains("could not be evaluated"));
        assert!(h.parse_err("due>").contains("could not be evaluated"));
        // A word that is no attribute stays a word.
        assert!(h.m("a>b", &task("is a>b ok")) && h.m("a < b", &task("is a < b ok")));
    }

    #[test]
    fn priority_and_uda_values_compare_in_their_own_order() {
        let h = H::new();
        let with = |p: &str| Facts {
            priority: Some(p.into()),
            ..task("x")
        };
        let (high, med, low, none) = (with("H"), with("M"), with("L"), task("x"));
        assert!(h.m("priority>M", &high) && !h.m("priority>M", &med) && !h.m("priority>M", &low));
        assert!(h.m("priority >= M", &med) && h.m("priority < M", &low) && h.m("priority<=L", &low));
        // A task with none is below them all.
        assert!(h.m("priority<L", &none) && h.m("priority <= M", &none) && !h.m("priority>L", &none));
    }

    #[test]
    fn latest_is_the_task_with_the_largest_number_and_uda_includes_priority() {
        let mut h = H::new();
        let (a, b) = (
            task("a"),
            Facts {
                uuid: Uuid::from_u128(2),
                ..task("b")
            },
        );
        h.ids.insert(a.uuid, 1);
        h.ids.insert(b.uuid, 2);
        assert!(h.m("+LATEST", &b) && !h.m("+LATEST", &a) && h.m("-LATEST", &a));
        // `priority` is a user defined attribute in Taskwarrior.
        let high = Facts {
            priority: Some("H".into()),
            ..task("c")
        };
        assert!(h.m("+UDA", &high) && !h.m("+UDA", &a));
    }

    #[test]
    fn this_week_counts_from_monday_whatever_weekstart_says() {
        // 2026-10-07 is a Wednesday; Sunday 2026-10-04 is before Monday 2026-10-05.
        let h = H::with_rc("weekstart=Sunday\n");
        let sunday = Facts {
            due: Some(NOW - 3 * DAY),
            ..task("x")
        };
        assert!(
            !h.m("+WEEK", &sunday)
                && h.m(
                    "+WEEK",
                    &Facts {
                        due: Some(NOW + 4 * DAY),
                        ..task("y")
                    }
                )
        );
        assert!(!h.m(
            "+WEEK",
            &Facts {
                due: Some(NOW + 5 * DAY),
                ..task("z")
            }
        ));
    }

    #[test]
    fn a_bang_without_a_parenthesis_takes_one_value_as_taskwarrior_does() {
        let h = H::new();
        let mut t = task("x");
        t.project = Some("Work".into());
        t.tags.insert("a".into());
        // `! project:Work` is `(!project) = 'Work'`: `false` against `Work`, for a task with a project or without.
        assert!(!h.m("! project:Work", &t) && !h.m("! project:Work", &task("x")));
        // `! +a` is `(!tags) _hastag_ a`, and that looks at the tags of the task: the `!` does nothing.
        for (f, want) in [("! +a", true), ("! +b", false), ("! -b", true), ("!! +a", true)] {
            assert_eq!(h.m(f, &t), want, "{f}");
        }
        // The truth of the project, as a word: `false` for a project, `true` for none.
        assert!(!h.m("! project ~ false", &task("x")) && h.m("! project ~ true", &task("x")));
        assert!(h.m("! project = true", &task("x")) && !h.m("! project = true", &t));
        assert!(h.m("!! project = true", &t) && !h.m("!! project = true", &task("x")));
        // A pattern is looked for in `true` or `false`; any other word is a value of its own, true because it is
        // not empty, so that `! word` is false for every task and `!! word` true. Likewise a number after `!`.
        assert!(
            h.m("! /fal/", &t)
                && !h.m("! /tru/", &t)
                && h.m(
                    "! /tru/",
                    &Facts {
                        description: String::new(),
                        ..task("")
                    }
                )
        );
        assert!(!h.m("! fal", &t) && !h.m("! 1", &t) && h.m("!! fal", &t) && h.m("!! 1", &t) && h.m("! 0", &t));
        // Glued to the next word the `!` still stands apart, but `+a` after it is no tag.
        assert!(!h.m("!project:Work", &t) && h.parse_err("!+a").contains("could not be evaluated"));
        // No `and` goes in front of a `!`, so two values are left side by side.
        assert!(h.parse_err("+a ! project:Work").contains("not an expression"));
        assert!(!h.m("+a and ! project:Work", &t));
        // With a parenthesis it reverses the group, and a number or uuid is in a group of its own.
        assert!(h.m("!(project:Work)", &task("x")) && !h.m("!(project:Work)", &t));
        assert!(h.parse_err("!").contains("could not be evaluated") && h.parse_err("+a and !").contains("could not"));
        assert!(h.parse_err("! and +a").contains("could not be evaluated"));
    }

    #[test]
    fn a_bang_stands_apart_from_its_parenthesis() {
        let h = H::new();
        let mut t = task("x");
        t.tags.insert("a".into());
        assert!(!h.m("!(+a)", &t) && h.m("!(+b)", &t) && h.m("+a and !(+b or +c)", &t));
    }

    #[test]
    fn a_bang_after_a_term_makes_its_value_the_truth_of_it() {
        let h = H::new();
        let with_tags = |tags: &[&str]| {
            let mut t = task("x");
            t.tags = tags.iter().map(|s| (*s).to_owned()).collect();
            t
        };
        // `+a !` is `tags _hastag_ (!a)`: the tag `false`; `!!` makes it `true`.
        let (falsy, truthy) = (with_tags(&["a", "false"]), with_tags(&["a", "true"]));
        assert!(h.m("+a !", &falsy) && !h.m("+a !", &truthy) && h.m("+a !!", &truthy) && h.m("+a ! !", &truthy));
        assert!(h.m("-a !", &truthy) && !h.m("-a !", &falsy));
        // The same for the value of an attribute, a pattern, a spaced comparison and a modifier.
        let (mut falsy, mut truthy) = (proj("false"), proj("true"));
        falsy.description = "x".into();
        truthy.description = "x".into();
        assert!(h.m("project:Work !", &falsy) && !h.m("project:Work !", &truthy) && h.m("project:Work !!", &truthy));
        assert!(
            h.m("project.is:Work !", &falsy) && h.m("project = Work !", &falsy) && h.m("project == Work !", &falsy)
        );
        assert!(h.m("description ~ x !", &task("false")) && !h.m("description ~ x !", &task("x")));
        assert!(h.m("/x/ !", &task("false")) && !h.m("/x/ !", &task("x")));
        // A number or a date is compared with 0 or 1, and a task that has none is the empty text, which comes first.
        let mut due = task("x");
        due.due = Some(NOW + DAY);
        assert!(h.m("due > 2026-10-01 !", &due) && !h.m("due < 2026-10-30 !", &due));
        assert!(
            h.m("due.before:eom !", &task("x"))
                && !h.m("due.before:eom !", &due)
                && !h.m("due.after:eom !", &task("x"))
        );
        assert!(h.m("urgency > 5 !", &proj("Work")) && !h.m("urgency < 5 !", &proj("Work")));
        // `.any:` and `.none:` have the truth of an empty value to compare with: every task, and none.
        assert!(h.m("due.any: !", &task("x")) && h.m("due.any: !", &due) && !h.m("due.none: !", &due));
        // A word that is no pattern is a value of its own, an attribute's name is what it holds.
        assert!(!h.m("x !", &task("x")) && h.m("x !!", &task("x")) && !h.m("x !", &task("y")));
        assert!(h.m("project !", &task("x")) && !h.m("project !", &proj("Work")) && h.m("project !!", &proj("Work")));
        assert!(!h.m("description !", &task("x")) && h.m("! project", &task("x")));
        // A group is reversed, and what follows a `!` needs an `and` or an `or`.
        assert!(!h.m("( +a ) !", &with_tags(&["a"])) && h.m("( +a ) !", &with_tags(&["b"])));
        assert!(h.m("( +a ! ) or +b", &with_tags(&["b"])) && h.m("+a ! or +b", &with_tags(&["b"])));
        assert!(!h.m("+a ! and +b", &with_tags(&["b"])) && !h.m("+a ! and +b", &falsy));
        assert!(
            h.parse_err("+a ! +b").contains("not an expression") && h.parse_err("x ! y").contains("not an expression")
        );
    }

    #[test]
    fn a_word_is_broken_at_each_parenthesis_inside_it() {
        let h = H::new();
        let mut t = task("not now");
        t.tags.insert("a".into());
        // `not(+a)` is `not` and the group `(+a)`, which an implicit `and` puts together.
        assert!(h.m("not(+a)", &t) && !h.m("not(+b)", &t) && h.m("(not)(+a)", &t) && h.m("not( +a )", &t));
        assert!(h.m("no(w)", &t) && !h.m("no(zz)", &t) && h.m("(not)now", &t));
        // A name and its value keep theirs.
        assert!(h.m("description:not(now)", &task("not(now)")) && !h.m("description:not(now)", &t));
        assert!(h.parse_err("not(+a").contains("Mismatched parentheses"));
    }

    #[test]
    fn a_value_that_begins_with_an_operator_is_an_expression_that_fails() {
        let h = H::new();
        // `^` is the power operator to Taskwarrior's lexer, so a pattern anchored with it written apart is an error.
        for (f, why) in [
            ("description ~ ^x", "Cannot exponentiate strings"),
            ("description !~ ^x", "Cannot exponentiate strings"),
            ("description ~ ^ x", "Cannot exponentiate strings"),
            ("project != ^W", "Cannot exponentiate strings"),
            ("project~^W", "Cannot exponentiate strings"),
            ("due ~ ^2026", "Cannot exponentiate dates"),
            ("description = *x", "Cannot multiply strings by strings"),
            ("description = %x", "Cannot modulo string values"),
            ("due > *x", "Cannot multiply date values"),
            ("urgency > ^2", "could not be evaluated"),
            ("urgency > ^x", "Cannot exponentiate strings"),
        ] {
            assert!(h.parse_err(f).contains(why), "{f}: {}", h.parse_err(f));
        }
        // Together with the name by `:` or in a modifier it is a value like any other.
        assert!(h.m("description.has:^x", &task("x y")) && !h.m("description:^x", &task("x")));
    }

    #[test]
    fn text_operators_on_a_date_read_it_as_iso_text() {
        let h = H::new();
        let mut t = task("x");
        t.due = Some(NOW + DAY); // 2026-10-08T12:30:00
        for f in [
            "due.has:10-08",
            "due.startswith:2026-10",
            "due.endswith:30:00",
            "due.word:2026",
            "due~T12:30",
            "due.hasnt:11-",
        ] {
            assert!(h.m(f, &t), "{f}");
        }
        for f in [
            "due.has:11-08",
            "due.startswith:10",
            "due.hasnt:10-08",
            "due.noword:2026",
        ] {
            assert!(!h.m(f, &t), "{f}");
        }
        // No date is the empty text: the empty pattern is in it and no word is.
        assert!(h.m("due.has:", &task("y")) && h.m("due.hasnt:10", &task("y")) && !h.m("due.has:1", &task("y")));
    }

    #[test]
    fn exact_date_is_with_time() {
        let h = H::new();
        let mut t = task("x");
        t.due = Some(NOW - NOW % 60 + DAY); // 12:30:00 tomorrow
        assert!(h.m("due.is:2026-10-08T12:30", &t));
        assert!(!h.m("due.is:2026-10-08T12:31", &t));
        assert!(h.m("due:2026-10-08", &t)); // date-only `:` is the whole day
        assert!(!h.m("due.is:2026-10-08", &t)); // `is` is the instant: midnight
    }

    #[test]
    fn bad_values_are_errors_not_silent_misses() {
        let h = H::new();
        assert!(h.parse_err("due:nonsense").contains("not a valid date"));
        assert!(h
            .parse_err("due.bogus:today")
            .contains("unrecognized attribute modifier"));
        assert!(h.parse_err("(+a").contains("Mismatched parentheses"));
        assert!(h.parse_err("+a)").contains("Mismatched parentheses"));
        assert!(h.parse_err("( )").contains("No expression to evaluate"));
        assert!(h.parse_err("+a and").contains("expected"));
        assert!(h.parse_err("and +a").contains("could not be evaluated"));
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

    /// Checked against `task` 3.5.0: ids and uuids side by side are one "or" group, even with other terms between.
    #[test]
    fn several_ids_and_uuids_pick_a_group_and_the_rest_narrows_it() {
        let mut h = H::new();
        let mut ts = Vec::new();
        for (i, d) in ["a", "b", "c"].into_iter().enumerate() {
            let mut t = desc(d);
            t.uuid = Uuid::parse_str(&format!("abcdef00-0000-0000-0000-00000000000{i}")).unwrap();
            h.ids.insert(t.uuid, i as u32 + 1);
            ts.push(t);
        }
        let hits = |h: &H, f: &str| -> String {
            ts.iter()
                .filter(|t| h.m(f, t))
                .map(|t| t.description.as_str())
                .collect::<Vec<_>>()
                .join("")
        };
        assert_eq!(hits(&h, "1 2"), "ab");
        assert_eq!(hits(&h, "1 3"), "ac");
        assert_eq!(hits(&h, "1,2"), "ab");
        assert_eq!(hits(&h, "1 2 description:a"), "a");
        assert_eq!(hits(&h, "1 description:b 2"), "b");
        assert_eq!(hits(&h, "3 +PENDING 1"), "ac");
        assert_eq!(
            hits(
                &h,
                "abcdef00-0000-0000-0000-000000000000 abcdef00-0000-0000-0000-000000000002"
            ),
            "ac"
        );
        assert_eq!(hits(&h, "1 abcdef00-0000-0000-0000-000000000002"), "ac");
        // Kept: one id, a group in parentheses, an explicit `and`, `or`.
        assert_eq!(hits(&h, "2"), "b");
        assert_eq!(hits(&h, "( 1 2 ) description:a"), "a");
        assert_eq!(hits(&h, "1 and 2"), "");
        assert_eq!(hits(&h, "1 or 3"), "ac");
    }

    /// The app's links carry the first 8 characters of a uuid, which can be all digits; a bare `12345678` is a
    /// range of task numbers, so the app asks with `uuid.startswith:`.
    #[test]
    fn a_uuid_prefix_of_digits_only_is_found_with_startswith() {
        let h = H::new();
        let mut t = task("x");
        t.uuid = Uuid::parse_str("12345678-9012-3456-7890-123456789012").unwrap();
        let mut other = task("y");
        other.uuid = Uuid::parse_str("12345699-9012-3456-7890-123456789012").unwrap();
        assert!(h.m("uuid.startswith:12345678", &t) && !h.m("uuid.startswith:12345678", &other));
        assert!(h.m("uuid.startswith:12345678-9012", &t));
        assert!(!h.m("uuid.startswith:ffffffff", &t));
    }

    fn desc(d: &str) -> Facts {
        Facts {
            description: d.into(),
            ..task("x")
        }
    }

    #[test]
    fn plain_words_and_slash_patterns_are_regular_expressions() {
        let h = H::new();
        let milk = desc("Buy milk today");
        assert!(h.m("m.lk", &milk), "a plain word is a pattern, as in Taskwarrior");
        assert!(h.m("/m[aeiou]lk/", &milk));
        assert!(h.m("/^Buy/", &milk) && !h.m("/^milk/", &milk));
        assert!(h.m("/today$/", &milk) && !h.m("/milk$/", &milk));
        assert!(h.m("/milk|eggs/", &milk) && h.m("/colou?r|milk/", &milk));
        assert!(!h.m("/MILK/", &milk), "case sensitive by default");
        // Searched anywhere, not just from the start.
        assert!(
            h.m("'/ilk to/'", &milk),
            "quoted, as on a command line, so the space is part of the pattern"
        );
    }

    #[test]
    fn text_matching_also_looks_in_annotations() {
        let h = H::new();
        let mut t = desc("Buy milk");
        t.annotations.push(crate::model::Note {
            entry: NOW,
            text: "ask Sam about oat".into(),
        });
        assert!(h.m("Sam", &t) && h.m("/oat$/", &t));
        assert!(h.m("desc.has:Sam", &t), "description attribute includes annotations");
        assert!(!h.m("desc.hasnt:Sam", &t));
        assert!(h.m("desc.hasnt:eggs", &t));
        assert!(!h.m("project.has:Sam", &t), "other attributes do not");
    }

    #[test]
    fn modifiers_are_patterns_the_way_taskwarrior_rewrites_them() {
        let h = H::new();
        let t = desc("buy milk");
        assert!(h.m("desc.has:m.lk", &t) && h.m("desc.contains:^buy", &t));
        assert!(h.m("desc.startswith:buy", &t) && !h.m("desc.startswith:milk", &t));
        assert!(h.m("desc.left:b.y", &t), "the value is a pattern, so . is any byte");
        assert!(h.m("desc.endswith:milk", &t) && !h.m("desc.endswith:buy", &t));
        assert!(h.m("desc.right:m.lk", &t));
        assert!(h.m("desc.word:milk", &t) && !h.m("desc.word:mil", &t));
        assert!(h.m("desc.noword:mil", &t) && !h.m("desc.noword:milk", &t));
        assert!(!h.m("desc.word:milk", &desc("milkshake")));
        // Over a list of values (tags): any one of them.
        let mut tagged = task("x");
        tagged.tags.insert("urgent".into());
        tagged.tags.insert("home".into());
        assert!(h.m("tags.has:ur.ent", &tagged) && h.m("tags.startswith:hom", &tagged));
        assert!(!h.m("tags.has:work", &tagged) && h.m("tags.hasnt:work", &tagged));
    }

    #[test]
    fn matching_reads_bytes_and_folds_only_ascii_case_like_std_regex() {
        let h = H::new();
        let cafe = desc("café");
        assert!(h.m("/caf./", &cafe) && !h.m("/caf.$/", &cafe), "é is two bytes");
        assert!(h.m("/café/", &cafe));
        assert!(!h.m("/\\w$/", &cafe), "\\w is ASCII only");
        let ci = H::with_rc("search.case.sensitive=no\n");
        assert!(ci.m("/MILK/", &desc("buy milk")) && ci.m("desc.has:MILK", &desc("buy milk")));
        assert!(!ci.m("/CAFÉ/", &cafe), "ASCII case folding only");
    }

    #[test]
    fn a_bad_pattern_is_an_error_that_says_why() {
        let h = H::new();
        let e = h.parse_err("/(/");
        assert!(e.contains("not a valid regular expression"), "{e}");
        assert!(h.parse_err("desc.has:[a-").contains("not a valid regular expression"));
        let e = h.parse_err("/foo(?=bar)/");
        assert!(e.contains("lookahead") && e.contains("not supported here"), "{e}");
        assert!(h.parse_err("/(?i)milk/").contains("inline flags"));
        assert!(h.parse_err("/(a)\\1/").contains("backreferences"));
    }

    #[test]
    fn regex_off_matches_plain_text() {
        let h = H::with_rc("regex=off\n");
        let milk = desc("Buy milk. Today");
        assert!(h.m("milk.", &milk) && !h.m("m.lk", &milk), "a dot is a dot");
        assert!(h.m("/Buy/", &milk) && h.m("/^Buy/", &milk) && h.m("/Today$/", &milk));
        assert!(h.m("[a", &desc("x [a y")), "no pattern, so nothing to get wrong");
        assert!(h.m("desc.has:milk", &milk) && h.m("desc.startswith:Buy", &milk) && h.m("desc.endswith:Today", &milk));
        assert!(h.m("desc.word:milk", &milk) && !h.m("desc.word:mil", &milk));
    }

    #[test]
    fn limit_directive() {
        let h = H::new();
        let ctx = h.ctx();
        assert_eq!(Filter::parse_str("limit:5", &ctx).unwrap().limit, Limit::N(5));
        assert_eq!(Filter::parse_str("limit:page +a", &ctx).unwrap().limit, Limit::Page);
        assert_eq!(Filter::parse_str("limit:none", &ctx).unwrap().limit, Limit::None);
        // Whether one was given matters: without one a report uses the `limit` setting.
        assert!(Filter::parse_str("limit:none", &ctx).unwrap().limit_set);
        assert!(Filter::parse_str("limit:5 +a", &ctx).unwrap().limit_set);
        assert!(!Filter::parse_str("+a", &ctx).unwrap().limit_set);
        assert!(!Filter::parse_str("", &ctx).unwrap().limit_set);
        assert_eq!(parse_limit("12"), Some(Limit::N(12)));
        assert_eq!(parse_limit("page"), Some(Limit::Page));
        assert_eq!(parse_limit("0"), Some(Limit::None));
        assert_eq!(parse_limit("-3"), None);
        assert_eq!(parse_limit("lots"), None);
        // A directive alone still matches everything.
        assert!(Filter::parse_str("limit:3", &ctx).unwrap().matches(&task("x"), &ctx));
    }

    #[test]
    fn quoting_and_conjoin() {
        assert_eq!(
            split_words(r#"project:"Home Stuff" 'a b' c"#),
            ["project:Home Stuff", "a b", "c"]
        );
        let h = H::new();
        let ctx = h.ctx();
        let combined = conjoin(&[split_words("status:pending"), vec![], split_words("+a or +b")]);
        let f = Filter::parse(&combined, &ctx).unwrap();
        let mut t = task("x");
        t.tags.insert("b".into());
        assert!(f.matches(&t, &ctx));
        // The `or` is fenced inside its own parentheses, so it can't leak into the status term.
        t.status = "completed".into();
        assert!(!f.matches(&t, &ctx));
        assert!(h.m(
            "project:\"Home Stuff\"",
            &Facts {
                project: Some("Home Stuff".into()),
                ..task("x")
            }
        ));
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
