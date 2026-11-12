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
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject

/** The sections' titles, and where each is in `settings.json`. */
val KEY_SECTION_TITLES = mapOf("general" to ("General" to "keys"), "web" to ("Web" to "web.keys"), "mobile" to ("Mobile" to "mobile.keys"), "tui" to ("Terminal" to "tui.keys"))

/** [b] matches what is typed: its name, what it does, or one of its keys (KeysPanel.svelte `matches`). */
fun bindingMatches(b: Binding, query: String): Boolean {
    val t = query.trim().lowercase()
    if (t.isEmpty()) return true
    val keys = b.keys ?: b.defaults
    return t in b.action.lowercase() || t in b.about.lowercase() || keys.any { it.lowercase() == t || keyLabel(it).lowercase() == t }
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

    // Adding: the section, its search, the action picked and the key for it.
    var adding by mutableStateOf<String?>(null)
    var find by mutableStateOf("")
    var picked by mutableStateOf<Binding?>(null)
    var typed by mutableStateOf("")
    var pressed by mutableStateOf<String?>(null)
    var recording by mutableStateOf(false)

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

    fun change(section: String, action: String, keys: List<String>?) {
        done = false
        runCatching { withBinding(draft, section, action, keys) }.onSuccess { draft = it }.onFailure { error = it.message }
    }

    fun openAdd(section: String) {
        adding = section
        find = ""
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
        resetKey()
    }

    fun addKey() {
        val section = adding ?: return
        val b = picked ?: return
        val key = newKey ?: return
        val now = b.keys.orEmpty()
        change(section, b.action, if (key in now) now else now + key)
        closeAdd()
    }

    fun noKey() {
        val section = adding ?: return
        val b = picked ?: return
        change(section, b.action, emptyList())
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
                            Text(b.action, fontSize = 13.sp, fontFamily = FontFamily.Monospace, color = if (b.keys == null) colors.onSurfaceVariant else colors.onSurface)
                        }
                        if (b.about.isNotEmpty()) Dim(b.about, size = 12) else Text("no such action", color = colors.error, fontSize = 12.sp)
                        if (b.keys != null && store.showDefaultKeys && b.defaults.isNotEmpty()) {
                            Row(verticalAlignment = Alignment.CenterVertically) { Dim("default ", size = 12); Keys(b.defaults, true) }
                        }
                    }
                    if (b.keys != null) TextButton({ state.change(section, b.action, null) }) { LucideIcon("x", size = 16.dp) }
                }
            }
        }
        bindings.problems.forEach { Text(it, color = colors.error, fontSize = 13.sp) }
        Dim("Kept in settings.json: keys for every front end, the others for one alone. A key you set replaces the action's defaults, and is taken from any action that had it.")
        state.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            Button({ scope.launch { state.apply() } }, enabled = !state.busy && state.edited) { Text("Apply") }
            if (state.done) Dim("Saved")
        }
    }
}

/** The search for an action to add a key to, then the key: pressed on a hardware keyboard, or typed. */
@Composable
private fun AddKey(state: KeysState) {
    val colors = MaterialTheme.colorScheme
    Card(Modifier.fillMaxWidth().padding(vertical = 4.dp)) {
        Column(Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            val picked = state.picked
            if (picked == null) {
                OutlinedTextField(state.find, { state.find = it }, Modifier.fillMaxWidth(), singleLine = true, placeholder = { Text("Search actions") })
                val found = state.found
                if (found.isEmpty()) Dim("No action matches.")
                for (b in found.take(40)) {
                    Row(Modifier.fillMaxWidth().clickable { state.picked = b }.padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                        Column(Modifier.weight(1f)) {
                            Text(b.action, fontSize = 13.sp, fontFamily = FontFamily.Monospace)
                            if (b.about.isNotEmpty()) Dim(b.about, size = 12)
                        }
                        Keys(b.keys ?: b.defaults, b.keys == null)
                    }
                }
                TextButton(state::closeAdd) { Text("Cancel") }
            } else {
                Text(picked.action, fontSize = 13.sp, fontFamily = FontFamily.Monospace)
                if (picked.about.isNotEmpty()) Dim(picked.about, size = 12)
                if (picked.keys == null && picked.defaults.isNotEmpty()) {
                    Row(verticalAlignment = Alignment.CenterVertically) { Dim("Replaces its default ", size = 12); Keys(picked.defaults, true) }
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
                    Button(state::addKey, enabled = state.newKey != null) { Text("Add key") }
                    TextButton(state::noKey) { Text("No key") }
                    TextButton({
                        state.picked = null
                        state.resetKey()
                    }) { Text("Back") }
                    TextButton(state::closeAdd) { Text("Cancel") }
                }
            }
        }
    }
}
