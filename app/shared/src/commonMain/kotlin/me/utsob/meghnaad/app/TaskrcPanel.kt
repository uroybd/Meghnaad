package me.utsob.meghnaad.app

import androidx.compose.runtime.remember
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay

/** The taskrc tab's edits: kept while another tab is shown (TaskrcPanel.svelte). */
class TaskrcState(private val store: Store) {
    var text by mutableStateOf("")
    /** What is saved right now, to detect edits and to revert to. */
    var saved by mutableStateOf("")
    var loading by mutableStateOf(true)
    var busy by mutableStateOf(false)
    var result by mutableStateOf<TaskrcResponse?>(null)
    var error by mutableStateOf<String?>(null)
    var confirmRestore by mutableStateOf(false)
    var link by mutableStateOf("")

    val edited get() = text != saved
    val coloursOff get() = colourOff(text)

    /** Start from what is saved, so an import edits it rather than replacing it with a blank page. */
    suspend fun load() {
        loading = true
        try {
            val body = store.request("GET", "/api/config/taskrc")
            saved = body
            text = body
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            loading = false
        }
    }

    // The engine returns what it actually saved: show that, not what was pasted, so secrets that were in a pasted file are gone
    // from the screen as well as from storage.
    private fun applied(body: String) {
        val r = engineJson.decodeFromString(TaskrcResponse.serializer(), body)
        result = r
        saved = r.text
        text = r.text
    }

    /** A theme: its colour lines replace the box's colour lines (Apply keeps them). */
    fun pickTheme(id: String) {
        error = null
        try {
            text = applyTheme(text, id, if (id == "meghnaad") "" else loadTheme(id))
            result = null
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        }
    }

    private suspend fun run(block: suspend () -> Unit) {
        busy = true
        error = null
        try {
            block()
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }

    suspend fun save() = run {
        // `include dark-256.theme`, as a desktop taskrc has it, becomes that theme's lines.
        text = expandIncludes(text) { loadTheme(it) }
        applied(store.request("PUT", "/api/config/taskrc", text))
        store.settingsChanged()
    }

    /** The engine fetches the link and saves what it keeps, as a paste does. */
    suspend fun fromLink() = run {
        applied(store.request("POST", "/api/config/taskrc/url", link.trim()))
        link = ""
        store.settingsChanged()
    }

    suspend fun restore() {
        if (!confirmRestore) {
            confirmRestore = true
            delay(4000)
            confirmRestore = false
            return
        }
        confirmRestore = false
        run {
            applied(store.request("POST", "/api/config/taskrc/restore"))
            store.settingsChanged()
        }
    }
}

@Composable
fun TaskrcPanel(store: Store, st: TaskrcState) {
    val colors = MaterialTheme.colorScheme
    LaunchedEffect(Unit) { st.load() }
    // The saved settings changed somewhere else (the console's `config`, a sync): follow them, unless there are edits here.
    LaunchedEffect(store.config) { if (!st.loading && !st.edited && !st.busy) st.load() }

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("taskrc", fontSize = 18.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
        Dim(
            "UDAs, custom reports, contexts and journal settings live in your ~/.taskrc, not in your synced data, so this app keeps a copy. " +
                "They are saved with your tasks and stay until you change them. Edit below, or paste or choose your taskrc, then Apply. Only those settings are kept: " +
                "sync settings and anything that looks like a credential are blocked; they are never stored, logged or shown.",
            size = 13,
        )
        store.config?.configError?.let {
            Text("The saved settings couldn't be read ($it). Tasks still work with the defaults. Importing below replaces them; the unreadable copy is kept aside.", color = colors.error, fontSize = 13.sp)
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Dim("Colour theme")
            Choice("choose…", THEMES.map { it.label }, hints = THEMES.map { it.note }) { st.pickTheme(THEMES[it].id) }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Switch(checked = !st.coloursOff, onCheckedChange = { st.text = withColour(st.text, it) })
            Text("Colours", fontSize = 14.sp)
        }
        Dim("A theme replaces the colour lines below; Apply to keep it. The Meghnaad theme is what applies when there are none.", size = 12)

        if (store.files != null) {
            OutlinedButton(onClick = { store.go { files?.pick()?.text?.let { st.text = it; st.result = null } } }) { Text("Choose a file") }
        }
        Dim("Or import from a link")
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                st.link, { st.link = it }, Modifier.weight(1f), singleLine = true,
                placeholder = { Text("https://raw.githubusercontent.com/you/dotfiles/main/.taskrc", fontSize = 12.sp, maxLines = 1) },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
            )
            OutlinedButton(enabled = !st.busy && !st.loading && st.link.isNotBlank(), onClick = { store.go { st.fromLink() } }) { Text("Import") }
        }
        Dim("Fetched and saved straight away (Restore previous undoes it). GitHub and GitLab file pages work too. A public link; keep it free of passwords.", size = 12)

        OutlinedTextField(
            st.text, { st.text = it }, Modifier.fillMaxWidth(), enabled = !st.loading, minLines = 12,
            textStyle = androidx.compose.ui.text.TextStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp),
            placeholder = { Text(if (st.loading) "Loading saved settings…" else "Nothing saved yet. Paste your taskrc here.", fontSize = 13.sp) },
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
        )
        st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        st.result?.let { r ->
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text("Saved ${r.udas} UDA${if (r.udas == 1) "" else "s"}, ${r.reports} report${if (r.reports == 1) "" else "s"}, ${r.contexts} context${if (r.contexts == 1) "" else "s"}.", color = extras.ok, fontSize = 13.sp)
                if (r.blocked.isNotEmpty()) Text("Blocked for safety (${r.blocked.size}): ${r.blocked.joinToString(", ")}", fontSize = 12.sp, fontFamily = FontFamily.Monospace)
                r.warnings.forEach { Text(it, color = extras.warn, fontSize = 13.sp) }
                if (r.ignored.isNotEmpty()) {
                    // Closed until asked for, as the web's: the list can be long.
                    var showIgnored by remember(r) { mutableStateOf(false) }
                    Row(Modifier.clickable { showIgnored = !showIgnored }.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        LucideIcon(if (showIgnored) "chevron-down" else "chevron-right", size = 13.dp)
                        Dim("${r.ignored.size} other settings not used", size = 12)
                    }
                    if (showIgnored) Text(r.ignored.joinToString(", "), fontSize = 12.sp, fontFamily = FontFamily.Monospace, color = colors.onSurfaceVariant)
                }
            }
        }
        Row(Modifier.fillMaxWidth().padding(bottom = 24.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.End)) {
            if (store.config?.hasPrevious == true) {
                TextButton(onClick = { store.go { st.restore() } }, enabled = !st.busy) {
                    LucideIcon("rotate-ccw", size = 14.dp)
                    Text("  " + if (st.confirmRestore) "Tap again to restore" else "Restore previous", fontSize = 13.sp)
                }
            }
            if (st.edited) TextButton(onClick = { st.text = st.saved }) { Text("Revert edits") }
            Button(onClick = { store.go { st.save() } }, enabled = !st.busy && !st.loading && st.edited) { Text("Apply") }
        }
    }
}
