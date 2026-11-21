package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.serialization.json.JsonObject

/** One line of the commands list while it is edited. */
class CmdRow(val key: Int, name: String, command: String, icon: String) {
    var name by mutableStateOf(name)
    var command by mutableStateOf(command)
    var icon by mutableStateOf(icon)
}

/** The commands tab's edits: kept while another tab is shown (CommandsPanel.svelte). */
class CommandsState(private val store: Store) {
    private var next = 0
    val rows = mutableStateListOf<CmdRow>().also { it.addAll(toRows(store.taskCommands)) }
    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var done by mutableStateOf(false)

    private fun toRows(cs: List<TaskCommand>) = cs.map { CmdRow(next++, it.name, it.command, it.icon ?: "") }

    val commands: List<TaskCommand>
        get() = rows.map { r -> TaskCommand(r.name.trim(), r.command.trim(), r.icon.takeIf { it.isNotEmpty() }) }

    val saved: List<TaskCommand> get() = store.taskCommands

    /** What the rows were last made from: when the saved commands change and the rows were not touched, they are made again. */
    private var base = store.taskCommands

    /** The saved commands changed (read late at start, applied here, or changed by a sync): follow them, unless there are edits. */
    fun follow() {
        val now = store.taskCommands
        if (now == base) return
        val untouched = commands == base
        base = now
        if (untouched) reset()
    }

    /** The icons picked here for the taskrc's aliases, by alias ("" takes one off). */
    val aliasEdits = mutableStateMapOf<String, String>()

    /** The taskrc's own aliases, for the Aliases list. */
    val aliases: List<Alias> get() = taskrcAliases(store.config?.config?.settings.orEmpty())

    val savedIcons: Map<String, String> get() = store.appSettings?.aliasIcons.orEmpty()

    /** The icons to save: the saved ones (an alias the taskrc does not have keeps its icon), with the edits here. */
    val icons: Map<String, String>
        get() = LinkedHashMap(savedIcons).apply {
            for ((name, icon) in aliasEdits) if (icon.isEmpty()) remove(name) else put(name, icon)
        }

    /** The icon an alias shows: the one picked here, else the saved one, else none ("" for the generic one). */
    fun iconOf(alias: String): String = aliasEdits[alias] ?: savedIcons[alias] ?: ""

    /** Icons for an alias the taskrc does not have: kept in the file, and not used. */
    val stray: List<String> get() = strayAliasIcons(savedIcons, store.config?.config?.settings.orEmpty())

    /** An icon picked for an alias that Lucide does not have, by alias. */
    val aliasProblems: Map<String, String>
        get() = aliases.mapNotNull { a ->
            val icon = iconOf(a.name)
            if (icon.isNotEmpty() && LucideLib.loaded && !LucideLib.has(icon)) a.name to "no Lucide icon is called “$icon”" else null
        }.toMap()

    val edited get() = commands != saved || icons != savedIcons

    /** What is wrong with each row, by key. */
    val problems: Map<Int, String>
        get() {
            val out = mutableMapOf<Int, String>()
            val seen = HashSet<String>()
            for (r in rows) {
                val name = r.name.trim()
                val why = nameProblem(name) ?: commandProblem(r.command)
                val dup = if (name in seen) "there is already a command named $name" else null
                seen += name
                val bad = why ?: dup ?: if (r.icon.isNotEmpty() && LucideLib.loaded && !LucideLib.has(r.icon)) "no Lucide icon is called “${r.icon}”" else null
                if (bad != null) out[r.key] = bad
            }
            return out
        }

    val text: String get() = serializeSettings(store.appSettings?.raw ?: JsonObject(emptyMap()), commands, icons)
    val tooLarge get() = text.encodeToByteArray().size > MAX_SETTINGS_BYTES
    val valid get() = problems.isEmpty() && aliasProblems.isEmpty() && !tooLarge

    /** Entries in the saved file that were not understood: applying drops them. */
    val unread get() = store.appSettings?.errors ?: emptyList()

    fun add() {
        rows.add(CmdRow(next++, "", "", ""))
        done = false
    }

    fun reset() {
        rows.clear()
        rows.addAll(toRows(saved))
        base = saved
        aliasEdits.clear()
        done = false
    }

    suspend fun apply() {
        busy = true
        error = null
        done = false
        try {
            store.request("PUT", "/api/settings", text)
            store.loadAppSettings()
            done = true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }
}

@Composable
fun CommandsPanel(store: Store, st: CommandsState) {
    val colors = MaterialTheme.colorScheme
    // settings.json is read a moment after the app starts, and this page may be made before: follow what is saved, unless edited.
    LaunchedEffect(store.taskCommands) { st.follow() }
    val problems = st.problems

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("Commands", fontSize = 18.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
        Dim(
            "A command is a word that stands for a whole command line: fail for done outcome:fail, and you can type fail, task fail, 3 fail or project:Home fail: " +
                "what comes before it selects the tasks. They also appear under the ⋯ menu on a task, and on the tasks you have selected. They are kept in this " +
                "app's own settings.json, not in your taskrc. A command wins over a Taskwarrior command or report with the same name.",
            size = 13,
        )
        if (st.unread.isNotEmpty()) {
            Column {
                Text("${if (st.unread.size == 1) "One entry" else "${st.unread.size} entries"} in settings.json can't be used and are kept in it as they are:", color = colors.error, fontSize = 13.sp)
                st.unread.forEach { Text("• $it", color = colors.error, fontSize = 13.sp) }
            }
        }
        if (st.rows.isEmpty()) Dim("No commands yet.", size = 14)
        for (r in st.rows.toList()) {
            val why = problems[r.key]
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    OutlinedTextField(
                        r.name, { r.name = it; st.done = false }, Modifier.weight(0.45f), singleLine = true, isError = why != null,
                        placeholder = { Text("fail", fontSize = 13.sp) },
                        textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp),
                        keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                    )
                    IconPicker(r.icon, { r.icon = it; st.done = false }, "Icon for ${r.name.ifEmpty { "the new command" }}")
                    IconAction("x", "Remove ${r.name.ifEmpty { "this command" }}", { st.rows.remove(r); st.done = false }, size = 15.dp)
                }
                OutlinedTextField(
                    r.command, { r.command = it; st.done = false }, Modifier.fillMaxWidth(), singleLine = true, isError = why != null,
                    placeholder = { Text("done outcome:fail", fontSize = 13.sp) },
                    textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp),
                    keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                )
                if (why != null) Text(why, color = colors.error, fontSize = 12.sp)
            }
        }
        OutlinedButton(onClick = st::add) { LucideIcon("plus", size = 14.dp); Text("  Add a command") }

        Text("Aliases", fontSize = 15.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold, modifier = Modifier.padding(top = 12.dp))
        Dim(
            "The aliases in your taskrc (alias.<name>=…) are in the same menus. Pick the icon each shows there; it is kept in " +
                "settings.json, and the alias itself stays in the taskrc.",
            size = 13,
        )
        val stray = st.stray
        if (stray.isNotEmpty()) {
            Column {
                Text(
                    "${if (stray.size == 1) "One icon is" else "${stray.size} icons are"} for an alias your taskrc doesn't have, and are kept in settings.json, unused:",
                    color = extras.warn, fontSize = 13.sp,
                )
                stray.forEach { Text("• $it", color = extras.warn, fontSize = 13.sp) }
            }
        }
        val aliases = st.aliases
        if (aliases.isEmpty()) Dim("Your taskrc has no aliases.", size = 14)
        val names = st.commands.map { it.name }.toSet()
        val aliasProblems = st.aliasProblems
        for (a in aliases) {
            Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Column(Modifier.weight(1f)) {
                        Text(a.name, fontFamily = FontFamily.Monospace, fontSize = 14.sp)
                        Text(a.command, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = colors.onSurfaceVariant, maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
                    }
                    IconPicker(st.iconOf(a.name), { st.aliasEdits[a.name] = it; st.done = false }, "Icon for the alias ${a.name}")
                }
                val why = aliasProblems[a.name]
                if (why != null) Text(why, color = colors.error, fontSize = 12.sp)
                else if (a.name in names) Dim("The command ${a.name} has this name and wins in the menus.", size = 12)
            }
        }

        var showJson by remember { mutableStateOf(false) }
        TextButton(onClick = { showJson = !showJson }) { Text("settings.json", fontSize = 13.sp, color = colors.onSurfaceVariant) }
        if (showJson) Text(st.text, fontFamily = FontFamily.Monospace, fontSize = 11.sp)

        if (st.tooLarge) Text("That is more than ${MAX_SETTINGS_BYTES / 1024} KB, which is the most that can be saved.", color = colors.error, fontSize = 13.sp)
        st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        if (st.done && !st.edited) Text("Applied.", color = extras.ok, fontSize = 13.sp)
        Row(Modifier.fillMaxWidth().padding(bottom = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
            if (st.edited) TextButton(onClick = st::reset) { Text("Revert edits") }
            Button(onClick = { store.go { st.apply() } }, enabled = !st.busy && st.edited && st.valid) { Text("Apply") }
        }
    }
}
