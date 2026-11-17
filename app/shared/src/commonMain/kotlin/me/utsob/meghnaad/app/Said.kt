package me.utsob.meghnaad.app

// What a command printed besides its result, and a question with what `task` prints before it, as the screens show them
// outside the Console: in a message, and in the box that asks (web/src/lib/said.ts).

/** One question, with what `task` prints before it (`dependency.reminder`: the tasks it waits on, and that wait on it). */
fun questionText(item: ConfirmItem): String = (item.note + item.question).joinToString("\n")

/** What a box asks for a whole confirmation: the one question, or the heading and each task's lines. */
fun confirmText(r: ConfirmResult): String =
    if (r.items.size == 1) questionText(r.items[0]) else (listOf(r.message) + r.items.flatMap { it.note }).joinToString("\n")

/** The lines a command printed (its reminders, then what the hooks said), a line each. */
fun saidText(lines: List<HookLine>): String = lines.joinToString("\n") { it.text.trim() }

/** Whether they make the message a warning: one of them is. */
fun saidBad(lines: List<HookLine>): Boolean = lines.any { it.kind == "warn" }
