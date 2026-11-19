package me.utsob.meghnaad.app

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject

/** The sections' titles, and where each is in `settings.json`. */
val KEY_SECTION_TITLES = mapOf("general" to ("General" to "keys"), "web" to ("Web" to "web.keys"), "mobile" to ("Mobile" to "mobile.keys"), "tui" to ("Terminal" to "tui.keys"))

/** [b] matches what is typed: its name, its command, what it does, or one of its keys (KeysPanel.svelte `matches`). */
fun bindingMatches(b: Binding, query: String): Boolean {
    val t = query.trim().lowercase()
    if (t.isEmpty()) return true
    val keys = b.keys ?: b.defaults
    return t in b.action.lowercase() || t in b.command.lowercase() || t in b.about.lowercase() ||
        keys.any { it.lowercase() == t || keyLabel(it).lowercase() == t }
}

/** The rows a section shows: the user's, and with [showDefaults] the actions that have only their defaults, that match [query]. */
fun bindingRows(actions: List<Binding>, showDefaults: Boolean, query: String): List<Binding> =
    actions.filter { (it.keys != null || (showDefaults && it.defaults.isNotEmpty())) && bindingMatches(it, query) }

/** The Keybindings tab (KeysPanel.svelte): the whole of `settings.json` as a draft, changed a key at a time, then applied. */
class KeysState(private val store: Store) {
    private val saved: JsonObject get() = store.appSettings?.raw ?: JsonObject(emptyMap())
    var draft by mutableStateOf(saved)
    private var base = saved

    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var done by mutableStateOf(false)
    var query by mutableStateOf("")

    // Adding: the section, its search, the preset picked (none for a command of one's own), the command it runs as
    // typed, where it acts when it can act in several places, and the key for it (spec 0025).
    var adding by mutableStateOf<String?>(null)
    var find by mutableStateOf("")
    var picked by mutableStateOf<Binding?>(null)
    var writing by mutableStateOf(false)
    var runs by mutableStateOf("")
    var where by mutableStateOf<String?>(null)
    var typed by mutableStateOf("")
    var pressed by mutableStateOf<String?>(null)
    var recording by mutableStateOf(false)

    /** The command as typed, checked by the engine: what it runs, or why it is no command; null while it is empty. */
    val checked: CheckedCommand? get() = runs.trim().takeIf { writing && it.isNotEmpty() }?.let { checkCommand(it) }

    /** Where it is bound: the place chosen, else the preset's own, else the first it acts in. */
    fun placeOf(c: CheckedCommand): KeyPlace? =
        c.places.firstOrNull { it.group == where } ?: c.places.firstOrNull { it.action == picked?.action } ?: c.places.firstOrNull()

    /** What it is listed as: the action alone, or the command as typed when it has words. */
    fun rowOf(c: CheckedCommand, place: KeyPlace): String = if (c.args.isEmpty() && c.count == null) place.action else runs.trim()

    val edited get() = draft != saved
    val bindings: KeyBindings get() = keymapBindings(draft)
    val typedKey: Pair<String?, String?>? get() = typed.trim().takeIf { it.isNotEmpty() }?.let(::parseKey)
    val newKey: String? get() = pressed ?: typedKey?.first

    fun rows(section: String): List<Binding> = bindingRows(bindings.sections[section].orEmpty(), store.showDefaultKeys, query)
    val found: List<Binding> get() = adding?.let { s -> bindings.sections[s].orEmpty().filter { bindingMatches(it, find) } }.orEmpty()

    /** Saved somewhere else (another device, the console): follow it, unless there are edits here. */
    fun follow() {
        val now = saved
        if (now == base) return
        val untouched = draft == base
        base = now
        if (untouched) draft = now
    }

    fun change(section: String, runs: String, keys: List<String>?, group: String? = null) {
        done = false
        runCatching { withBinding(draft, section, runs, keys, group = group) }.onSuccess { draft = it }.onFailure { error = it.message }
    }

    /** A key that runs no command, taken out of the draft. */
    fun takeOut(bad: BadKey) {
        done = false
        runCatching { withoutKey(draft, bad) }.onSuccess { draft = it }.onFailure { error = it.message }
    }

    fun openAdd(section: String) {
        adding = section
        find = ""
        picked = null
        writing = false
        resetKey()
    }

    /** The form for a preset, its command to start from; or, with none, for a command of one's own. */
    fun pick(b: Binding?) {
        picked = b
        writing = true
        runs = b?.command?.ifEmpty { null } ?: b?.action ?: ""
        where = null
    }

    fun back() {
        writing = false
        picked = null
        resetKey()
    }

    fun resetKey() {
        typed = ""
        pressed = null
        recording = false
    }

    fun closeAdd() {
        adding = null
        picked = null
        writing = false
        resetKey()
    }

    fun addKey() {
        val section = adding ?: return
        val c = checked?.takeIf { it.problem == null } ?: return
        val place = placeOf(c) ?: return
        val key = newKey ?: return
        val row = rowOf(c, place)
        val now = bindings.sections[section].orEmpty().firstOrNull { it.action == row }?.keys.orEmpty()
        change(section, runs.trim(), if (key in now) now else now + key, place.group)
        closeAdd()
    }

    fun noKey() {
        val section = adding ?: return
        val c = checked?.takeIf { it.problem == null } ?: return
        val place = placeOf(c) ?: return
        change(section, runs.trim(), emptyList(), place.group)
        closeAdd()
    }

    suspend fun apply() {
        busy = true
        error = null
        done = false
        try {
            store.saveKeys(draft)
            base = saved
            done = true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }
}

@Composable
private fun Keys(keys: List<String>, muted: Boolean) {
    val colors = MaterialTheme.colorScheme
    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        for (k in keys) {
            Pill(
                keyLabel(k),
                background = if (muted) Color.Transparent else colors.surfaceVariant,
                color = if (muted) colors.onSurfaceVariant else colors.onSurface,
                border = if (muted) BorderStroke(1.dp, colors.outlineVariant) else null,
                mono = true,
            )
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun KeysPanel(store: Store, state: KeysState) {
    state.follow()
    val scope = rememberCoroutineScope()
    val colors = MaterialTheme.colorScheme
    val bindings = state.bindings
    Column(Modifier.padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (bindings.bad.isNotEmpty()) {
            Card(Modifier.fillMaxWidth()) {
                Column(Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Text("These keys run no command, so they do nothing. Take them out, or fix them in settings.json:", fontSize = 13.sp)
                    for (b in bindings.bad) {
                        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                            Column(Modifier.weight(1f)) {
                                Text("${keyLabel(b.key)} in ${b.at} runs ${b.runs}", fontSize = 13.sp, fontFamily = FontFamily.Monospace)
                                Dim(b.why, size = 12)
                            }
                            TextButton({ state.takeOut(b) }) { Text("Take out") }
                        }
                    }
                }
            }
        }
        OutlinedTextField(state.query, { state.query = it }, Modifier.fillMaxWidth(), singleLine = true, placeholder = { Text("Search keys") })
        Row(Modifier.clickable { store.showDefaultKeys(!store.showDefaultKeys) }, verticalAlignment = Alignment.CenterVertically) {
            Checkbox(store.showDefaultKeys, onCheckedChange = null, modifier = Modifier.padding(end = 8.dp))
            Text("Show defaults", fontSize = 14.sp)
        }
        for (section in KEY_SECTIONS) {
            val (title, at) = KEY_SECTION_TITLES.getValue(section)
            Row(Modifier.fillMaxWidth().padding(top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
                Text(title, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                Dim("  $at", Modifier.weight(1f), 12)
                OutlinedButton({ state.openAdd(section) }) { LucideIcon("plus", size = 14.dp); Text(" Add") }
            }
            if (state.adding == section) AddKey(state)
            val rows = bindings.sections[section].orEmpty().let { bindingRows(it, store.showDefaultKeys, state.query) }
            if (rows.isEmpty()) Dim(if (state.query.isBlank()) "None of your own." else "Nothing matches.")
            for (b in rows) {
                Row(Modifier.fillMaxWidth().heightIn(min = 40.dp), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                            when {
                                b.keys == null -> Keys(b.defaults, true)
                                b.keys.isEmpty() -> Dim("no key")
                                else -> Keys(b.keys, false)
                            }
                            Text(b.command.ifEmpty { b.action }, fontSize = 13.sp, fontFamily = FontFamily.Monospace, color = if (b.keys == null) colors.onSurfaceVariant else colors.onSurface)
                            if (b.command.isNotEmpty() && b.command != b.action) Dim(b.action, size = 12)
                        }
                        if (b.about.isNotEmpty()) Dim(b.about, size = 12)
                        if (b.keys != null && store.showDefaultKeys && b.defaults.isNotEmpty()) {
                            Row(verticalAlignment = Alignment.CenterVertically) { Dim("default ", size = 12); Keys(b.defaults, true) }
                        }
                    }
                    if (b.keys != null) TextButton({ state.change(section, b.action, null) }) { LucideIcon("x", size = 16.dp) }
                }
            }
        }
        Text("Commands", Modifier.padding(top = 10.dp), fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
        Dim("A key can run one of these, with its words, in place of an action: \"g h\": \"filter +home\" under tasks runs :filter +home. The part in brackets may be left out, as in vim.", size = 12)
        val uri = LocalUriHandler.current
        val commands = commandRows(remember { keyCommands() }, state.query)
        if (commands.isEmpty()) Dim("Nothing matches.")
        for (c in commands) {
            Row(Modifier.fillMaxWidth().padding(vertical = 2.dp), verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(c.written, fontSize = 13.sp, fontFamily = FontFamily.Monospace)
                    Dim(c.what, size = 12)
                }
                if (c.link.isNotEmpty()) TextButton({ runCatching { uri.openUri(c.link) } }) { Text("More") }
            }
        }
        bindings.problems.forEach { Text(it, color = colors.error, fontSize = 13.sp) }
        Dim("Kept in settings.json: keys for every front end, the others for one alone, each saved as the command it runs. A key you set replaces the defaults of what it runs, and is taken from any action that had it.")
        state.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            Button({ scope.launch { state.apply() } }, enabled = !state.busy && state.edited) { Text("Apply") }
            if (state.done) Dim("Saved")
        }
    }
}

/** The search for an action to add a key to, then the key: pressed on a hardware keyboard, or typed. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AddKey(state: KeysState) {
    val colors = MaterialTheme.colorScheme
    Card(Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
        Column(Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            if (!state.writing) {
                OutlinedTextField(state.find, { state.find = it }, Modifier.fillMaxWidth(), singleLine = true, placeholder = { Text("Search actions") })
                val found = state.found
                if (found.isEmpty()) Dim("No action matches.")
                for (b in found.take(40)) {
                    Row(Modifier.fillMaxWidth().clickable { state.pick(b) }.padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(b.command.ifEmpty { b.action }, fontSize = 13.sp, fontFamily = FontFamily.Monospace)
                            if (b.command.isNotEmpty() && b.command != b.action) Dim(b.action, size = 12)
                            if (b.about.isNotEmpty()) Dim(b.about, size = 12)
                        }
                        Keys(b.keys ?: b.defaults, b.keys == null)
                    }
                }
                Row(Modifier.fillMaxWidth().clickable { state.pick(null) }.padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    LucideIcon("plus", size = 14.dp, tint = colors.primary)
                    Text(" Add a command", color = colors.primary, fontSize = 14.sp)
                    Dim("  of your own, with its words", size = 12)
                }
                TextButton(state::closeAdd) { Text("Cancel") }
            } else {
                OutlinedTextField(
                    state.runs,
                    { state.runs = it },
                    Modifier.fillMaxWidth(),
                    singleLine = true,
                    label = { Text("Command") },
                    placeholder = { Text("filter +home") },
                    textStyle = androidx.compose.ui.text.TextStyle(fontFamily = FontFamily.Monospace),
                )
                val checked = state.checked
                val place = checked?.takeIf { it.problem == null }?.let(state::placeOf)
                when {
                    checked == null -> Dim("A command, with its words: done, filter +home, sort due-.", size = 12)
                    checked.problem != null -> Text(checked.problem, color = colors.error, fontSize = 13.sp)
                    place != null -> {
                        val words = if (checked.args.isEmpty()) "" else " with ${checked.args.joinToString(" ")}"
                        Dim("Runs ${place.action}$words" + if (checked.about.isNotEmpty()) ": ${checked.about}" else "", size = 12)
                        if (checked.places.size > 1) {
                            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                                Dim("Where", size = 12)
                                for (p in checked.places) {
                                    if (p == place) Button({ state.where = p.group }) { Text(p.group) }
                                    else OutlinedButton({ state.where = p.group }) { Text(p.group) }
                                }
                            }
                        }
                        val now = state.adding?.let { s -> state.bindings.sections[s].orEmpty().firstOrNull { it.action == state.rowOf(checked, place) } }
                        if (now != null && now.keys == null && now.defaults.isNotEmpty()) {
                            Row(verticalAlignment = Alignment.CenterVertically) { Dim("Replaces its default ", size = 12); Keys(now.defaults, true) }
                        }
                    }
                }
                val focus = remember { FocusRequester() }
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(
                        {
                            state.recording = true
                            focus.requestFocus()
                        },
                        Modifier.focusRequester(focus).onFocusChanged { if (!it.isFocused) state.recording = false }.focusable().onPreviewKeyEvent { e ->
                            if (!state.recording || e.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                            if (e.key == Key.Escape) {
                                state.recording = false
                            } else {
                                chordOf(e)?.let {
                                    state.pressed = it
                                    state.typed = ""
                                    state.recording = false
                                }
                            }
                            true
                        },
                    ) {
                        Text(
                            when {
                                state.recording -> "Press a key…"
                                state.pressed != null -> keyLabel(state.pressed!!)
                                else -> "Press a key"
                            },
                        )
                    }
                    OutlinedTextField(
                        state.typed,
                        {
                            state.typed = it
                            state.pressed = null
                        },
                        Modifier.width(150.dp),
                        singleLine = true,
                        placeholder = { Text("ctrl+k") },
                    )
                }
                state.typedKey?.second?.let { Text(it, color = colors.error, fontSize = 13.sp) }
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    Button(state::addKey, enabled = state.newKey != null && place != null) { Text("Add key") }
                    TextButton(state::noKey, enabled = place != null) { Text("No key") }
                    TextButton(state::back) { Text("Back") }
                    TextButton(state::closeAdd) { Text("Cancel") }
                }
            }
        }
    }
}
