//! The commands that change the tasks they select (`modify`, `done`, `delete`, `annotate`, `duplicate`, `purge`,
//! ...), and `undo`.

use super::{
    add_note, apply_changes, asks, chain_repairs, changed, confirm_item, conjoin, context_read, declined_line, error,
    ident, invert, model, modify, modify_plan, ok, passed_over, permission_question, plural, recur, selected, verb_of,
    warm_facts, Annotate, Annotation, Append, Ask, BTreeMap, BTreeSet, Change, CliResult, Config, ConfirmItem, Delete,
    Denotate, Done, Duplicate, EvalCtx, Facts, Kind, Mode, Modify, Operations, Options, Parsed, Prepend, Purge, Repair,
    Replica, Start, Status, Stop, Storage, Task, TimeZone, UndoStack, Utc, Uuid, Writer,
};

/// `undo`: the last write command's changes, reversed.
pub(super) async fn undo<S: Storage>(
    replica: &mut Replica<S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    opts: &Options,
    undo: &mut UndoStack,
) -> Done {
    let Some(last) = undo.0.last().cloned() else {
        return ok(CliResult::Text {
            lines: vec!["Nothing to undo.".into()],
        });
    };
    if cfg.confirmation() && !opts.confirmed {
        return ok(CliResult::Confirm {
            message: "The undo command is not reversible.  Are you sure you want to revert to the previous state?"
                .into(),
            ask: Ask::Plain,
            items: vec![],
        });
    }
    let reverse = match invert(
        replica,
        &last,
        Utc.timestamp_opt(ctx.clock.now, 0).single().unwrap_or_else(Utc::now),
    )
    .await
    {
        Ok(r) => r,
        Err(m) => return error(m),
    };
    if let Err(e) = replica.commit_operations(reverse).await {
        return error(e.to_string());
    }
    undo.0.pop();
    Done {
        result: CliResult::Text {
            lines: vec!["Undone.".into()],
        },
        wrote: true,
        command: None,
        config: None,
        feedback: Vec::new(),
    }
}

/// A step of a write command: what it found, or the answer the command gives now (an error, a question, "No matches").
type Step<T> = Result<T, Box<Done>>;

/// The answer a step stops the command with.
fn halt(d: impl Into<Done>) -> Box<Done> {
    Box::new(d.into())
}

/// A write command, in the order Taskwarrior goes: the tasks the filter picks, what follows the command word, the tasks
/// it would change, permission for them, the follow-up questions (the rest of a series, a broken dependency chain, a
/// template's instances), then the change to each task, written in one commit.
pub(super) async fn write_selected<S: Storage>(
    w: Writer<'_, S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    kind: Kind,
    p: &Parsed,
) -> Done {
    match write_steps(w, cfg, ctx, all, kind, p).await {
        Ok(done) => done,
        Err(done) => *done,
    }
}

async fn write_steps<S: Storage>(
    w: Writer<'_, S>,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    kind: Kind,
    p: &Parsed,
) -> Step<Done> {
    let Writer {
        replica,
        opts,
        undo,
        hk,
    } = w;
    let sel = selection(cfg, ctx, all, p, &opts)?;
    let words = read_words(kind, p, cfg, opts.typed)?;
    let would_change = would_change(kind, &sel, &words, ctx, all)?;
    let declined = permission(kind, &sel, &would_change, &opts, cfg, ctx, all)?;
    let chosen = without(&sel, &declined);
    let changing = without(&would_change, &declined);
    let agreed = agree(kind, &changing, &opts, cfg, ctx, all)?;
    let targets = targets(kind, &chosen, &changing, &agreed, all);

    let mut batch = Batch::new();
    if kind == Purge {
        unlink(replica, &targets, all, &mut batch.ops).await?;
    }
    let plan = Plan {
        kind,
        words: &words,
        propagate: &agreed.propagate,
        targets: &targets,
        journal: cfg.journal(),
    };
    for f in &targets {
        change(replica, f, &plan, ctx, all, hk, &mut batch).await?;
    }
    write_masks(replica, &mut batch).await?;
    let touched_any = !batch.touched.is_empty() || !batch.instances_touched.is_empty();
    let repaired = if touched_any {
        repair(replica, &agreed.repairs, &mut batch.ops).await?
    } else {
        Vec::new()
    };
    let Batch {
        ops,
        mut touched,
        passed,
        instances_touched,
        notes,
        ..
    } = batch;
    touched.extend(instances_touched);
    if touched.is_empty() {
        return Ok(nothing_done(kind, &sel, ctx));
    }

    undo.push(&ops);
    replica
        .commit_operations(ops)
        .await
        .map_err(|e| halt(error(e.to_string())))?;
    let after = warm_facts(replica, opts.warm.as_ref()).await.unwrap_or_default();
    let past = match kind {
        Done => "Completed",
        Delete => "Deleted",
        Start => "Started",
        Stop => "Stopped",
        Annotate => "Annotated",
        Denotate => "Updated",
        Duplicate => "Duplicated",
        Purge => "Purged",
        _ => "Modified",
    };
    // The tasks it passed over are named as it goes, before the count.
    let mut message = String::new();
    for n in &passed {
        message.push_str(&format!("{n} "));
    }
    message.push_str(&format!("{past} {}.", plural(touched.len(), "task")));
    for n in &notes {
        message.push_str(&format!(" {n}"));
    }
    if !declined.is_empty() {
        message.push_str(&format!(" Skipped {}.", plural(declined.len(), "task")));
    }
    if !repaired.is_empty() {
        message.push_str(&format!(
            " Repaired the dependencies of {}.",
            plural(repaired.len(), "task")
        ));
        touched.extend(repaired);
    }
    Ok(Done {
        result: changed(&after, &touched, message),
        wrote: true,
        command: None,
        config: None,
        feedback: Vec::new(),
    })
}

/// The tasks the filter picks, after Taskwarrior's safety net: a command that changes tasks, given no filter, would
/// change them all (finished and deleted ones too). `allow.empty.filter` forbids that; otherwise it asks, and with
/// `confirmation` off it refuses. The context's filter counts as a filter.
fn selection<'a>(
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &'a [Facts],
    p: &Parsed,
    opts: &Options,
) -> Step<Vec<&'a Facts>> {
    let empty_filter = conjoin(&[context_read(cfg), p.filter.clone()]).is_empty();
    if empty_filter {
        if !cfg.allow_empty_filter() {
            return Err(halt(error(
                "You did not specify a filter, and with the 'allow.empty.filter' value, no action is taken.",
            )));
        }
        if !cfg.confirmation() {
            return Err(halt(error("Command prevented from running.")));
        }
    }
    let (sel, _) = selected(all, ctx, cfg, &p.filter).map_err(halt)?;
    if sel.is_empty() {
        return Err(halt(ok(CliResult::Text {
            lines: vec!["No matches.".into()],
        })));
    }
    if empty_filter && !opts.confirmed {
        return Err(halt(ok(CliResult::Confirm {
            message: format!(
                "This command has no filter, and will modify all (including completed and deleted) tasks.  Are you sure? ({} in all.)",
                plural(sel.len(), "task")
            ),
            ask: Ask::Plain,
            items: vec![],
        })));
    }
    Ok(sel)
}

/// What follows the command word, read as the command reads it.
struct Words {
    /// The changes, for the commands that read them (`modify`, and the attributes and tags of the others).
    mods: Option<modify::Mods>,
    /// The plain words: the annotation, the text to append or prepend, the annotation `denotate` matches.
    text: String,
    /// The attributes, tags and substitution given beside the text (`done end:-2h`, `annotate hello due:eow`).
    extra: Option<modify::Mods>,
}

/// `modify` reads it all as changes (words replace the description). `done`, `delete`, `start`, `stop`, `annotate`,
/// `append` and `prepend` read it as Taskwarrior does: attributes, tags and substitutions are applied to each task
/// (`done end:-2h`, `start due:eow`, `annotate hello due:eow`), and the plain words that are left are the annotation or
/// the text. `denotate` takes the whole of it as the text to match.
fn read_words(kind: Kind, p: &Parsed, cfg: &Config, typed: bool) -> Step<Words> {
    let reads_changes = matches!(
        kind,
        Modify | Done | Delete | Start | Stop | Annotate | Append | Prepend | Duplicate
    );
    let mods = if reads_changes {
        Some(modify::parse_mods(&p.mods, cfg).map_err(halt)?)
    } else {
        None
    };
    let (text, extra): (String, Option<modify::Mods>) = match (&mods, kind) {
        // A GUI note or addition is text, whatever it looks like.
        (_, Annotate | Append | Prepend | Duplicate) if !typed => (p.mods.join(" "), None),
        (Some(m), Done | Delete | Start | Stop | Annotate | Append | Prepend | Duplicate) => {
            let others =
                !m.attrs.is_empty() || !m.add_tags.is_empty() || !m.remove_tags.is_empty() || m.subst.is_some();
            (
                m.words.join(" "),
                others.then(|| modify::Mods {
                    words: Vec::new(),
                    ..m.clone()
                }),
            )
        }
        _ => (p.mods.join(" "), None),
    };
    if matches!(kind, Annotate | Append | Prepend) && text.trim().is_empty() && extra.is_none() {
        return Err(halt(error("this command needs some text")));
    }
    if kind == Denotate && text.trim().is_empty() {
        return Err(halt(error("this command needs some text")));
    }
    Ok(Words { mods, text, extra })
}

/// Whether the command does anything to this task.
fn acts(kind: Kind, f: &Facts) -> bool {
    match kind {
        Done => f.status == "pending",
        Delete => f.status != "deleted",
        // Only a deleted task can be purged: it has to be marked deleted first.
        Purge => f.status == "deleted",
        Start => f.status != "recurring" && f.start.is_none(),
        Stop => f.status != "recurring" && f.start.is_some(),
        _ => true,
    }
}

/// The tasks this command would change: those are the ones Taskwarrior asks about. A change that can't be made
/// (`done due:nonsense`) is refused before anything is asked.
fn would_change<'a>(
    kind: Kind,
    sel: &[&'a Facts],
    words: &Words,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
) -> Step<Vec<&'a Facts>> {
    let mut out: Vec<&Facts> = Vec::new();
    for f in sel {
        if let Some(x) = &words.extra {
            modify::plan(x, Mode::Modify, Some(f), ctx, all).map_err(|e| halt(error(e.0)))?;
        }
        let changes = match kind {
            Modify => !modify_plan(words.mods.as_ref(), f, ctx, all)
                .map_err(|m| halt(error(m)))?
                .is_empty(),
            _ => acts(kind, f),
        };
        if changes {
            out.push(f);
        }
    }
    Ok(out)
}

/// Permission, task by task (Taskwarrior's yes / no / all / quit, as a table): deleting asks when `confirmation` is on,
/// and any change asks once it reaches `bulk` tasks. The tasks declined; when that is all of them, the answer.
fn permission<'a>(
    kind: Kind,
    sel: &[&Facts],
    would_change: &[&'a Facts],
    opts: &Options,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
) -> Step<Vec<&'a Facts>> {
    let mut declined: Vec<&Facts> = Vec::new();
    if !would_change.is_empty() && asks(kind, sel.len(), cfg) {
        match &opts.approved {
            None => {
                let items: Vec<ConfirmItem> = would_change
                    .iter()
                    .map(|f| confirm_item(f.uuid.to_string(), f, ctx, permission_question(kind, f, ctx, all)))
                    .collect();
                let message = if items.len() == 1 {
                    items[0].question.clone()
                } else {
                    format!(
                        "This will {} {}. Choose which ones to go ahead with.",
                        verb_of(kind),
                        plural(items.len(), "task")
                    )
                };
                return Err(halt(ok(CliResult::Confirm {
                    message,
                    ask: Ask::Permission,
                    items,
                })));
            }
            Some(yes) => {
                declined = would_change
                    .iter()
                    .filter(|f| !yes.contains(&f.uuid.to_string()))
                    .copied()
                    .collect();
            }
        }
    }
    if !declined.is_empty() && declined.len() == would_change.len() {
        return Err(halt(ok(CliResult::Text {
            lines: declined.iter().map(|_| declined_line(kind).to_owned()).collect(),
        })));
    }
    Ok(declined)
}

/// `tasks` without those declined.
fn without<'a>(tasks: &[&'a Facts], declined: &[&Facts]) -> Vec<&'a Facts> {
    tasks
        .iter()
        .filter(|f| !declined.iter().any(|d| d.uuid == f.uuid))
        .copied()
        .collect()
}

/// What the follow-up questions settled.
struct Agreed<'a> {
    /// The tasks whose recurring series the change is carried to (`recurrence.confirmation`).
    propagate: BTreeSet<Uuid>,
    /// The dependency chains to repair (`dependency.confirmation`).
    repairs: Vec<Repair>,
    /// The instances a purged template takes with it.
    kids: Vec<&'a Facts>,
}

/// What only comes up once permission is settled, asked in one more table: carrying a change to the rest of a recurring
/// series (`recurrence.confirmation`), repairing a dependency chain that finishing or deleting a task in the middle of
/// it breaks (`dependency.confirmation`), and purging a template's deleted instances with it.
fn agree<'a>(
    kind: Kind,
    changing: &[&Facts],
    opts: &Options,
    cfg: &Config,
    ctx: &EvalCtx<'_>,
    all: &'a [Facts],
) -> Step<Agreed<'a>> {
    let asking = opts.extras.is_none();
    let said_yes = |key: &str| opts.extras.as_ref().is_some_and(|y| y.iter().any(|k| k == key));
    let mut items: Vec<ConfirmItem> = Vec::new();
    let mut propagate: BTreeSet<Uuid> = BTreeSet::new();
    if matches!(kind, Modify | Delete | Append | Prepend | Annotate) {
        let mode = recur::confirmation(cfg);
        let verb = match kind {
            Delete => "delete",
            Append => "append to",
            Prepend => "prepend to",
            Annotate => "annotate",
            _ => "modify",
        };
        for f in changing {
            // `modify` and `delete` reach a template's instances too; the others only go from an instance to its series.
            if series_of(f, all).is_empty() || (!matches!(kind, Modify | Delete) && f.parent.is_none()) {
                continue;
            }
            match mode {
                recur::Confirmation::Yes => {
                    propagate.insert(f.uuid);
                }
                recur::Confirmation::No => {}
                recur::Confirmation::Prompt => {
                    let key = format!("rec:{}", f.uuid);
                    if asking {
                        items.push(confirm_item(
                            key,
                            f,
                            ctx,
                            format!(
                                "This is a recurring task.  Do you want to {verb} all pending recurrences of this same task?"
                            ),
                        ));
                    } else if said_yes(&key) {
                        propagate.insert(f.uuid);
                    }
                }
            }
        }
    }
    let mut repairs: Vec<Repair> = Vec::new();
    if matches!(kind, Done | Delete) {
        let mut ask = |f: &Facts| -> bool {
            if !cfg.dependency_confirmation() {
                return true;
            }
            let key = format!("dep:{}", f.uuid);
            if asking {
                // Asked on the assumption that the earlier ones are repaired; a later one that
                // then turns out not to arise is simply never used.
                items.push(confirm_item(
                    key,
                    f,
                    ctx,
                    "Would you like the dependency chain fixed?".into(),
                ));
                true
            } else {
                said_yes(&key)
            }
        };
        repairs = chain_repairs(changing, all, &mut ask);
    }
    // `purge`: a recurring template can only go when all its instances are deleted already, and it takes them
    // with it (asking first, as `recurrence.confirmation` says; "no" refuses).
    let mut kids: Vec<&Facts> = Vec::new();
    if kind == Purge {
        for f in changing.iter().filter(|f| f.mask.is_some()) {
            let children: Vec<&Facts> = all.iter().filter(|c| c.parent == Some(f.uuid)).collect();
            if let Some(live) = children.iter().find(|c| c.status != "deleted") {
                return Err(halt(error(format!(
                    "Task '{}' is a recurrence template. Its child task {} must be deleted before it can be purged.",
                    f.description,
                    ident(ctx, live)
                ))));
            }
            if children.is_empty() {
                continue;
            }
            let key = format!("rec:{}", f.uuid);
            let go = match recur::confirmation(cfg) {
                recur::Confirmation::Yes => true,
                recur::Confirmation::No => false,
                recur::Confirmation::Prompt if asking => {
                    let question = format!(
                        "Task '{}' is a recurrence template. All its {} deleted children tasks will be purged as well. Continue?",
                        f.description,
                        children.len()
                    );
                    items.push(confirm_item(key, f, ctx, question));
                    true
                }
                recur::Confirmation::Prompt => said_yes(&key),
            };
            if !go {
                return Err(halt(error("Purge operation aborted.")));
            }
            kids.extend(children);
        }
    }
    if asking && !items.is_empty() {
        let message = if items.len() == 1 {
            items[0].question.clone()
        } else {
            "A few more questions about these tasks.".to_owned()
        };
        return Err(halt(ok(CliResult::Confirm {
            message,
            ask: Ask::Extras,
            items,
        })));
    }
    Ok(Agreed {
        propagate,
        repairs,
        kids,
    })
}

/// The tasks the command writes to: the ones chosen; with a delete carried to a series, the rest of it (the pending
/// instances of a template, or the template and the other pending instances of an instance); with `purge`, only the
/// deleted tasks and the instances of a template that goes.
fn targets<'a>(
    kind: Kind,
    chosen: &[&'a Facts],
    changing: &[&'a Facts],
    agreed: &Agreed<'a>,
    all: &'a [Facts],
) -> Vec<&'a Facts> {
    let mut targets: Vec<&Facts> = chosen.to_vec();
    if kind == Delete {
        for f in changing.iter().filter(|f| agreed.propagate.contains(&f.uuid)) {
            for c in series_of(f, all) {
                if c.status != "deleted" && !targets.iter().any(|t| t.uuid == c.uuid) {
                    targets.push(c);
                }
            }
        }
    }
    if kind == Purge {
        targets.clear();
        for f in changing.iter().chain(&agreed.kids).copied() {
            if !targets.iter().any(|t| t.uuid == f.uuid) {
                targets.push(f);
            }
        }
    }
    targets
}

/// `purge`: what waited on the tasks that go no longer does, so nothing points at a task that is gone.
async fn unlink<S: Storage>(
    replica: &mut Replica<S>,
    targets: &[&Facts],
    all: &[Facts],
    ops: &mut Operations,
) -> Step<()> {
    let gone: BTreeSet<Uuid> = crate::ordered::set_of(targets.iter().map(|f| f.uuid));
    for d in all
        .iter()
        .filter(|d| !gone.contains(&d.uuid) && d.depends.iter().any(|u| gone.contains(u)))
    {
        let Some(mut t) = replica.get_task(d.uuid).await.map_err(|e| halt(error(e.to_string())))? else {
            continue;
        };
        for u in d.depends.iter().filter(|u| gone.contains(u)) {
            t.remove_dependency(*u, ops).map_err(|e| halt(error(e.to_string())))?;
        }
    }
    Ok(())
}

/// What the command does to every task, settled before the first is changed.
struct Plan<'a> {
    kind: Kind,
    words: &'a Words,
    propagate: &'a BTreeSet<Uuid>,
    targets: &'a [&'a Facts],
    /// `journal.time`'s notes for starting and stopping, when it is on.
    journal: Option<(String, String)>,
}

/// The changes gathered task by task, committed together at the end.
struct Batch {
    ops: Operations,
    /// The tasks the command changed (or, for `duplicate`, made).
    touched: Vec<Uuid>,
    /// The members of a series the change was carried to.
    instances_touched: Vec<Uuid>,
    /// What Taskwarrior says about the tasks it passed over.
    passed: Vec<String>,
    /// What `duplicate` has to say about the copies it made.
    notes: Vec<String>,
    /// Parents' masks as they will be after this command; each is written once at the end.
    masks: BTreeMap<Uuid, String>,
    mask_dirty: Vec<Uuid>,
}

impl Batch {
    fn new() -> Self {
        Batch {
            ops: Operations::new(),
            touched: Vec::new(),
            instances_touched: Vec::new(),
            passed: Vec::new(),
            notes: Vec::new(),
            masks: BTreeMap::new(),
            mask_dirty: Vec::new(),
        }
    }
}

/// The command, done to one task.
async fn change<S: Storage>(
    replica: &mut Replica<S>,
    f: &Facts,
    plan: &Plan<'_>,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    hk: &crate::hooks::Runner,
    batch: &mut Batch,
) -> Step<()> {
    let kind = plan.kind;
    if kind == Purge {
        if let Some(mut data) = replica
            .get_task_data(f.uuid)
            .await
            .map_err(|e| halt(error(e.to_string())))?
        {
            data.delete(&mut batch.ops);
            batch.touched.push(f.uuid);
        }
        return Ok(());
    }
    if kind == Duplicate {
        let (copy, note) = duplicate(
            replica,
            Copied {
                f,
                extra: plan.words.extra.as_ref(),
                text: &plan.words.text,
            },
            ctx,
            all,
            hk,
            &mut batch.ops,
        )
        .await
        .map_err(|m| halt(error(m)))?;
        batch.touched.push(copy);
        batch.notes.extend(note);
        return Ok(());
    }
    if change_task(replica, f, plan, ctx, all, hk, batch)
        .await
        .map_err(|m| halt(error(m)))?
    {
        batch.touched.push(f.uuid);
    } else if let Some(why) = passed_over(kind) {
        // Taskwarrior says which tasks it passed over, when it did something to others.
        batch
            .passed
            .push(format!("Task {} '{}' {why}.", ident(ctx, f), f.description));
    }
    Ok(())
}

/// Every command but `purge` and `duplicate`, done to one task: whether it changed it.
async fn change_task<S: Storage>(
    replica: &mut Replica<S>,
    f: &Facts,
    plan: &Plan<'_>,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    hk: &crate::hooks::Runner,
    batch: &mut Batch,
) -> Result<bool, String> {
    let Plan {
        kind,
        words,
        propagate,
        targets,
        journal,
    } = plan;
    let (kind, text, ops) = (*kind, &words.text, &mut batch.ops);
    let mut changed = true;
    let mut task = replica
        .get_task(f.uuid)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("task {} disappeared", f.uuid))?;
    let e = |e: taskchampion::Error| e.to_string();
    // The modifications come first, as in Taskwarrior: `done end:-2h` sets the end time
    // before completing (which only fills it in when it is missing).
    if acts(kind, f) && matches!(kind, Done | Delete | Start | Stop | Annotate | Append | Prepend) {
        if let Some(x) = &words.extra {
            let changes = modify::plan(x, Mode::Modify, Some(f), ctx, all).map_err(|e| e.0)?;
            apply_changes(&mut task, &changes, ops)?;
        }
        if matches!(kind, Done | Delete | Start | Stop) && !text.trim().is_empty() {
            add_note(&mut task, text, ctx.clock.now, ops)?;
        }
    }
    match kind {
        // Only pending tasks can be completed (not templates, not finished ones).
        Done if f.status != "pending" => changed = false,
        Delete if f.status == "deleted" => changed = false,
        Start | Stop if f.status == "recurring" => changed = false,
        Done => {
            task.set_status(Status::Completed, ops).map_err(e)?;
            // Completing an active task stops it, like `task done` does.
            if f.start.is_some() {
                task.stop(ops).map_err(e)?;
                if let Some((_, stop)) = journal {
                    add_note(&mut task, stop, ctx.clock.now, ops)?;
                }
            }
        }
        Delete => task.set_status(Status::Deleted, ops).map_err(e)?,
        Start => {
            if f.start.is_some() {
                changed = false; // already active: nothing to do (and no second journal entry)
            } else {
                // Starting a completed or deleted task reopens it, as in Taskwarrior.
                if f.status == "completed" || f.status == "deleted" {
                    task.set_status(Status::Pending, ops).map_err(e)?;
                }
                task.start(ops).map_err(e)?;
                if let Some((start, _)) = journal {
                    add_note(&mut task, start, ctx.clock.now, ops)?;
                }
            }
        }
        Stop => {
            if f.start.is_none() {
                changed = false;
            } else {
                task.stop(ops).map_err(e)?;
                if let Some((_, stop)) = journal {
                    add_note(&mut task, stop, ctx.clock.now, ops)?;
                }
            }
        }
        Annotate if !text.trim().is_empty() => add_note(&mut task, text, ctx.clock.now, ops)?,
        Annotate => {}
        Denotate => {
            let anns: Vec<Annotation> = task.get_annotations().collect();
            let hit = anns
                .iter()
                .find(|a| &a.description == text)
                .or_else(|| anns.iter().find(|a| a.description.starts_with(text.as_str())));
            match hit {
                Some(a) => task.remove_annotation(a.entry, ops).map_err(e)?,
                None => return Err(format!("no annotation matches '{text}'")),
            }
        }
        Append | Prepend if text.trim().is_empty() => {}
        Append | Prepend => {
            let new = joined(kind, task.get_description(), text);
            task.set_description(new, ops).map_err(e)?;
        }
        Modify => {
            let changes = modify_plan(words.mods.as_ref(), f, ctx, all)?;
            apply_changes(&mut task, &changes, ops)?;
        }
        _ => return Err(format!("{} does not change a task", verb_of(kind))),
    }
    // The rest of the series gets the same command when that was agreed to (`recurrence.confirmation`): each
    // pending member and, from an instance, the template. All of it, dates and `recur` too, as `task` does.
    let wrote_text = !text.trim().is_empty() || kind == Modify;
    if changed && propagate.contains(&f.uuid) && matches!(kind, Modify | Append | Prepend | Annotate) && wrote_text {
        for c in series_of(f, all) {
            // A task that is itself being changed got the change already.
            if targets.iter().any(|t| t.uuid == c.uuid) || batch.instances_touched.contains(&c.uuid) {
                continue;
            }
            let Some(mut other) = replica.get_task(c.uuid).await.map_err(|x| x.to_string())? else {
                continue;
            };
            let ops = &mut batch.ops;
            match kind {
                Modify => {
                    let changes = modify_plan(words.mods.as_ref(), c, ctx, all)?;
                    apply_changes(&mut other, &changes, ops)?;
                }
                Annotate => add_note(&mut other, text, ctx.clock.now, ops)?,
                _ => {
                    let new = joined(kind, other.get_description(), text);
                    other.set_description(new, ops).map_err(e)?;
                }
            }
            let hooked = hk.modify(c, Facts::from_task(&other)).await?;
            apply_changes(&mut other, &hooked, &mut batch.ops)?;
            if kind == Modify {
                mirror_mask(&mut batch.masks, &mut batch.mask_dirty, all, c, &other);
            }
            batch.instances_touched.push(c.uuid);
        }
    }
    // `on-modify`: the hook sees the task as it was and as the command leaves it, and may
    // change it or refuse.
    if changed {
        let hooked = hk.modify(f, Facts::from_task(&task)).await?;
        apply_changes(&mut task, &hooked, &mut batch.ops)?;
    }
    // An instance's state is mirrored in its parent's mask (`+` done, `X` deleted, ...), by the commands that
    // change a task's state; not by the ones that only add to it or take from it.
    if changed && !matches!(kind, Annotate | Denotate | Append | Prepend) {
        mirror_mask(&mut batch.masks, &mut batch.mask_dirty, all, f, &task);
    }
    Ok(changed)
}

/// A description with `text` appended or prepended.
fn joined(kind: Kind, description: &str, text: &str) -> String {
    if kind == Append {
        format!("{description} {text}")
    } else {
        format!("{text} {description}")
    }
}

/// The parents' masks, each written once.
async fn write_masks<S: Storage>(replica: &mut Replica<S>, batch: &mut Batch) -> Step<()> {
    batch.mask_dirty.sort_unstable();
    batch.mask_dirty.dedup();
    for pu in &batch.mask_dirty {
        if let (Some(mask), Ok(Some(mut parent))) = (batch.masks.get(pu), replica.get_task(*pu).await) {
            parent
                .set_value("mask", Some(mask.clone()), &mut batch.ops)
                .map_err(|e| halt(error(e.to_string())))?;
        }
    }
    Ok(())
}

/// The dependency chains agreed to be repaired; the tasks whose dependencies changed.
async fn repair<S: Storage>(replica: &mut Replica<S>, repairs: &[Repair], ops: &mut Operations) -> Step<Vec<Uuid>> {
    let mut repaired: Vec<Uuid> = Vec::new();
    for r in repairs {
        let Ok(Some(mut t)) = replica.get_task(r.task).await else {
            continue;
        };
        let mut changes: Vec<Change> = r.removed.iter().map(|u| Change::RemoveDep(*u)).collect();
        changes.extend(r.added.iter().map(|u| Change::AddDep(*u)));
        apply_changes(&mut t, &changes, ops).map_err(|m| halt(error(m)))?;
        repaired.push(r.task);
    }
    Ok(repaired)
}

/// The answer when no task changed: Taskwarrior names each task it passed over and why, then counts none done.
fn nothing_done(kind: Kind, sel: &[&Facts], ctx: &EvalCtx<'_>) -> Done {
    if kind == Purge {
        return ok(CliResult::Text {
            lines: vec![
                "Purged 0 tasks.".into(),
                "No deleted tasks specified. Maybe you forgot to delete tasks first?".into(),
            ],
        });
    }
    let past = match kind {
        Start => "Started",
        Stop => "Stopped",
        Done => "Completed",
        Delete => "Deleted",
        _ => "Modified",
    };
    let mut lines: Vec<String> = Vec::new();
    if let Some(why) = passed_over(kind) {
        lines.extend(
            sel.iter()
                .map(|f| format!("Task {} '{}' {why}.", ident(ctx, f), f.description)),
        );
    }
    lines.push(format!("{past} 0 tasks."));
    ok(CliResult::Text { lines })
}

/// The task `duplicate` copies, with what followed the command: its modifications, and its words as a note.
struct Copied<'a> {
    f: &'a Facts,
    extra: Option<&'a modify::Mods>,
    text: &'a str,
}

/// `duplicate`: a copy of `f` as a new task, as in `CmdDuplicate`. It keeps everything but its identity,
/// `start`, `end` and `entry`, and starts over as pending; a copy of a recurring instance is a plain task,
/// a copy of a template is a template. What followed the command is applied to the copy, its words as a
/// note. Returns the copy, and what Taskwarrior notes about it.
async fn duplicate<S: Storage>(
    replica: &mut Replica<S>,
    src: Copied<'_>,
    ctx: &EvalCtx<'_>,
    all: &[Facts],
    hk: &crate::hooks::Runner,
    ops: &mut Operations,
) -> Result<(Uuid, Option<String>), String> {
    let Copied { f, extra, text } = src;
    let e = |e: taskchampion::Error| e.to_string();
    let data = replica.get_task_data(f.uuid).await.map_err(e)?;
    let data = data.ok_or_else(|| format!("task {} disappeared", f.uuid))?;
    let template = f.status == "recurring";
    let instance = f.parent.is_some();
    let uuid = crate::crypto::new_uuid().map_err(|e| e.to_string())?;
    let mut task = replica.create_task(uuid, ops).await.map_err(e)?;
    for (k, v) in data.iter() {
        let skip = matches!(k.as_str(), "status" | "start" | "end" | "entry" | "modified")
            || (instance && matches!(k.as_str(), "parent" | "recur" | "until" | "imask"))
            || (template && k == "mask");
        if !skip {
            task.set_value(k.clone(), Some(v.clone()), ops).map_err(e)?;
        }
    }
    task.set_status(
        if template && !instance {
            Status::Recurring
        } else {
            Status::Pending
        },
        ops,
    )
    .map_err(e)?;
    task.set_entry(Utc.timestamp_opt(ctx.clock.now, 0).single(), ops)
        .map_err(e)?;
    if let Some(x) = extra {
        let changes = modify::plan(x, Mode::Modify, Some(f), ctx, all).map_err(|e| e.0)?;
        apply_changes(&mut task, &changes, ops)?;
    }
    if !text.trim().is_empty() {
        add_note(&mut task, text, ctx.clock.now, ops)?;
    }
    let hooked = hk.add(Facts::from_task(&task)).await?;
    apply_changes(&mut task, &hooked, ops)?;
    let note = match (instance, template) {
        (true, _) => Some("was a recurring task.  The duplicated task is not."),
        (_, true) => Some("was a parent recurring task.  The duplicated task is too."),
        _ => None,
    };
    Ok((uuid, note.map(|n| format!("Note: task {} {n}", ident(ctx, f)))))
}

/// The rest of `f`'s recurring series: for a template its pending instances; for an instance its
/// pending siblings and the template. Empty for an ordinary task.
fn series_of<'a>(f: &Facts, all: &'a [Facts]) -> Vec<&'a Facts> {
    if f.status == "recurring" {
        all.iter()
            .filter(|c| c.parent == Some(f.uuid) && c.status == "pending")
            .collect()
    } else if let Some(parent) = f.parent {
        all.iter()
            .filter(|c| c.uuid != f.uuid && (c.uuid == parent || (c.parent == Some(parent) && c.status == "pending")))
            .collect()
    } else {
        Vec::new()
    }
}

/// Write an instance's state (`+` done, `X` deleted, `W` waiting, `-` pending) into its slot of its parent's mask,
/// to be saved with the other changes. `before` is how it was, `task` how the command leaves it.
fn mirror_mask(masks: &mut BTreeMap<Uuid, String>, dirty: &mut Vec<Uuid>, all: &[Facts], before: &Facts, task: &Task) {
    let (Some(parent), Some(index)) = (before.parent, before.imask) else {
        return;
    };
    let ch = recur::mask_char(&model::status_str(&task.get_status()), task.is_waiting());
    let cur = masks.entry(parent).or_insert_with(|| {
        all.iter()
            .find(|x| x.uuid == parent)
            .and_then(|x| x.mask.clone())
            .unwrap_or_default()
    });
    let next = recur::set_mask(cur, index, ch);
    if next != *cur {
        *cur = next;
        dirty.push(parent);
    }
}
