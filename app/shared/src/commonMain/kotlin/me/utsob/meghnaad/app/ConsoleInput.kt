package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** Commands with no page of their own: they only ever answer in the Console. */
private val CONSOLE_ONLY = setOf("show", "config", "export", "import", "timesheet", "information", "colors", "color")

private fun consoleOnly(line: String): Boolean {
    val words = line.replace(Regex("^task\\s+"), "").split(Regex("\\s+")).filter { !it.startsWith("rc.") }
    return words.any { it in CONSOLE_ONLY || Regex("^g?history\\.").containsMatchIn(it) }
}

/** A cheap guess, only to avoid flashing the Console for a report. */
private fun looksLikeReport(store: Store, line: String): Boolean {
    val words = line.replace(Regex("^task\\s+"), "").split(Regex("\\s+"))
    return store.reports.any { it.name in words }
}

/**
 * The prompt: a line of Taskwarrior syntax with completion and the last commands, under every page but the Console (which has
 * its own). The last command run, from here or from a button, shows above it to be edited or copied. Mirrors ConsoleInput.svelte.
 */
@Composable
fun ConsoleInput(store: Store, modifier: Modifier = Modifier) {
    val scope = rememberCoroutineScope()
    val completer = remember { Completer() }
    val focus = remember { FocusRequester() }
    val clipboard = LocalClipboardManager.current
    val focusManager = LocalFocusManager.current
    var cursor by remember { mutableStateOf(-1) } // -1 = the line being typed
    var saved by remember { mutableStateOf("") }
    var copied by remember { mutableStateOf(false) }
    var completeSignal by remember { mutableStateOf(0) }
    val line = store.promptLine

    // Something outside asked for the prompt (the bulk bar's Command): take its text, and the focus.
    var asked by remember { mutableStateOf(store.promptRequest?.second ?: 0) }
    LaunchedEffect(store.promptRequest) {
        val r = store.promptRequest ?: return@LaunchedEffect
        if (r.second == asked) return@LaunchedEffect
        asked = r.second
        store.promptLine = r.first
        cursor = -1
        completer.close()
        runCatching { focus.requestFocus() }
    }
    // `/` from anywhere that is not a text field (a hardware keyboard): the prompt, with its line selected.
    var focusAsked by remember { mutableStateOf(store.promptFocus) }
    var selectAll by remember { mutableStateOf(0) }
    LaunchedEffect(store.promptFocus) {
        if (store.promptFocus == focusAsked) return@LaunchedEffect
        focusAsked = store.promptFocus
        selectAll++
        runCatching { focus.requestFocus() }
    }
    LaunchedEffect(copied) {
        if (copied) {
            delay(1500)
            copied = false
        }
    }

    // The prompt is off while a line of several statements runs; it gets the focus back after the last one.
    var returnFocus by remember { mutableStateOf(false) }
    LaunchedEffect(store.chainBusy) {
        if (!store.chainBusy && returnFocus) {
            returnFocus = false
            runCatching { focus.requestFocus() }
        }
    }

    fun submit() {
        val l = line.trim()
        if (l.isEmpty() || store.chainBusy) return
        store.promptLine = "" // the command moves to the "last command" line above the prompt
        cursor = -1
        completer.close()
        if (l == "clear" || l == "cls") {
            store.clearEntries()
            return
        }
        // tasksh's `review` is the app's own: it is not sent, it drives a session of ordinary commands, drawn as a card.
        val asked = parseReview(l)
        if (asked != null) {
            store.go { store.review.start(asked) }
            return
        }
        // A task command from settings.json stands for the command line it names (what runs, and what the checks below look
        // at, is that); the history keeps what was typed. A line may be several statements, joined by the shell's `&&`, `||` and
        // `;` (see Compound.kt): they run one after the other, and the prompt is back after the last.
        val steps = try {
            planChain(l, store.taskCommands)
        } catch (e: ChainError) {
            store.view = View.Console
            store.addEntry(l, CliResult.Error(ErrorResult(e.message ?: "")))
            return
        }
        if (steps.isEmpty()) return
        if (steps.size > 1) {
            store.view = View.Console
            if (steps.any { parseReview(it.line) != null }) {
                store.addEntry(l, CliResult.Error(ErrorResult("‘review’ can only be run on its own, not in a line of several commands.")))
                return
            }
            returnFocus = true
            store.go { store.runChain(steps, l) }
            return
        }
        val cmd = steps[0].line
        // Reports focus the Tasks view; everything else lands in the console scrollback. Show the console first so a write or
        // an error is visible; `run` switches back for reports.
        if (store.view != View.Console && (consoleOnly(cmd) || !looksLikeReport(store, cmd))) store.view = View.Console
        store.go { run(CliInput(line = cmd), l) }
    }

    fun older(): Boolean {
        val h = store.history
        if (h.isEmpty()) return false
        if (cursor == -1) saved = line
        cursor = minOf(h.size - 1, cursor + 1)
        store.promptLine = h[h.size - 1 - cursor]
        return true
    }

    fun newer(): Boolean {
        if (cursor == -1) return false
        cursor -= 1
        store.promptLine = if (cursor == -1) saved else store.history[store.history.size - 1 - cursor]
        return true
    }

    Column(modifier.fillMaxWidth().padding(horizontal = 10.dp, vertical = 6.dp)) {
        // The history line: the last command, and the buttons that stand for keys a phone has not (↑ and Tab) beside the ones for it.
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (store.lastCommand.isNotEmpty()) {
                Text("last", color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 12.sp)
                Text(
                    store.lastCommand,
                    modifier = Modifier.weight(1f),
                    fontFamily = FontFamily.Monospace,
                    fontSize = 12.sp,
                    maxLines = 1,
                    overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                )
            } else Spacer(Modifier.weight(1f))
            // The active context, with the way to clear it (`context none`): the console runs in it, and so does every page that follows it.
            store.activeContext?.let { name ->
                Text("context", color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 12.sp)
                Text(name, color = MaterialTheme.colorScheme.primary, fontFamily = FontFamily.Monospace, fontWeight = FontWeight.SemiBold, fontSize = 12.sp, maxLines = 1)
                IconAction("x", "Clear the context", { store.go { run(CliInput(line = "context none"), "context none") } }, size = 14.dp)
            }
            IconAction("chevron-up", "Previous command", { older() }, size = 16.dp)
            IconAction("arrow-right-to-line", "Complete", { completeSignal++ }, size = 16.dp)
            if (store.lastCommand.isNotEmpty()) {
                IconAction("pencil", "Edit last command", {
                    store.promptLine = store.lastCommand.replace(Regex("^task\\s+"), "")
                    cursor = -1
                    runCatching { focus.requestFocus() }
                }, size = 14.dp)
                IconAction(if (copied) "check" else "copy", "Copy last command", {
                    clipboard.setText(AnnotatedString(store.lastCommand))
                    copied = true
                }, size = 14.dp)
            }
        }
        // The line is as tall as its text needs; what stands beside it is level with its last line, where the menu is not.
        var oneLine by remember { mutableStateOf(0) }
        val dark = extras.dark
        val commands = remember(store.reports, store.taskCommands) {
            COMMAND_NAMES.toSet() + store.reports.map { it.name } + store.taskCommands.map { it.name }
        }
        val colouring = remember(commands, dark) { HighlightTransformation(commands, HlPalette(dark)) }
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
            BesideField(oneLine) {
                Text("task>", fontFamily = FontFamily.Monospace, color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = 13.sp)
            }
            CompletingField(
                value = line,
                onValueChange = {
                    store.promptLine = it
                    cursor = -1
                },
                vocab = store.vocab,
                completer = completer,
                modifier = Modifier.weight(1f),
                placeholder = if (store.chainBusy) "Running…" else "next · add Buy milk due:tomorrow",
                enabled = !store.chainBusy,
                onSubmit = ::submit,
                focusRequester = focus,
                completeSignal = completeSignal,
                selectAllSignal = selectAll,
                menuAbove = true,
                singleLine = false,
                visualTransformation = colouring,
                onSize = { if (oneLine == 0 || it.height < oneLine) oneLine = it.height },
                onKey = { e, _ ->
                    when {
                        e.key == Key.L && e.isCtrlPressed -> {
                            store.clearEntries()
                            true
                        }
                        completer.menu != null -> false
                        e.key == Key.DirectionUp -> older()
                        e.key == Key.DirectionDown -> newer()
                        // With no menu to close, Escape leaves the prompt.
                        e.key == Key.Escape -> {
                            focusManager.clearFocus()
                            true
                        }
                        else -> false
                    }
                },
            )
            BesideField(oneLine) {
                // With a context in force the button wears the colour of the pill in the top bar.
                val inContext = store.activeContext != null
                Button(
                    onClick = ::submit,
                    colors = if (inContext) ButtonDefaults.buttonColors(containerColor = extras.warn, contentColor = contextText()) else ButtonDefaults.buttonColors(),
                ) {
                    LucideIcon("corner-down-left", size = 15.dp, tint = if (inContext) contextText() else MaterialTheme.colorScheme.onPrimary)
                }
            }
        }
    }
}
