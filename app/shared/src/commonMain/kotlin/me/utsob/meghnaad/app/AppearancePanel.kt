package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

/** A transformation as typed, with a key of its own for the list. */
data class RuleDraft(val key: Int, val match: String, val replace: String, val fields: List<String>)

/**
 * How the app draws things, kept in `settings.json` (AppearancePanel.svelte): the numeric UDAs shown as progress bars, and
 * the field transformations.
 */
class AppearanceState(private val store: Store) {
    private val saved get() = store.appearance

    /** The UDAs shown as bars, with the min and max as typed (so a half-typed number is not lost). */
    val draft = mutableStateMapOf<String, Pair<String, String>>().also { it.putAll(toDraft(saved)) }

    private var nextKey = 0
    private fun toRules(a: Appearance) = a.transformations.map { RuleDraft(nextKey++, it.match, it.replace, it.fields) }

    /** The transformations, in the order they run. */
    val rules = mutableStateListOf<RuleDraft>().also { it.addAll(toRules(saved)) }

    /** A task with no ID shows its short uuid rather than `-`. */
    var idFallback by mutableStateOf(saved.idFallback)

    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var done by mutableStateOf(false)

    private var base = saved

    private fun toDraft(a: Appearance) = a.progress.mapValues { (_, p) -> numberText(p.min) to numberText(p.max) }

    /** The numeric UDAs the taskrc defines, and any saved as bars that are not defined (any more), so they can be turned off. */
    val names: List<String>
        get() = (store.config?.config?.udas?.values?.filter { it.type == "numeric" }?.map { it.name } ?: emptyList()).let { defined ->
            (defined + draft.keys).distinct()
        }

    fun range(name: String): Progress? {
        val (min, max) = draft[name] ?: return null
        return Progress(min.trim().toDoubleOrNull() ?: Double.NaN, max.trim().toDoubleOrNull() ?: Double.NaN)
    }

    fun problem(name: String): String? {
        val (min, max) = draft[name] ?: return null
        if (min.isBlank() || max.isBlank()) return "min and max are needed"
        return range(name)?.problem()
    }

    /** The string UDAs the taskrc defines: the fields a transformation may name besides the description and the notes. */
    val stringUdas: List<String>
        get() = store.config?.config?.udas?.values?.filter { it.type == "string" }?.map { it.name } ?: emptyList()

    /** The fields to offer a rule: the text fields, and any it names that are not (any more), so they can be taken off. */
    fun fieldsOf(r: RuleDraft): List<String> = (listOf("description", "annotations") + stringUdas + r.fields).distinct()

    private val current
        get() = Appearance(
            draft.keys.mapNotNull { n -> range(n)?.let { n to it } }.toMap(),
            rules.map { Transformation(it.match, it.replace, it.fields) },
            idFallback,
        )

    /** What the engine says is wrong with the rules (a pattern that does not compile, no fields, a field that is not text). */
    val ruleProblems get() = transformProblems(current.transformations, stringUdas)
    val invalid get() = draft.keys.any { problem(it) != null } || ruleProblems.isNotEmpty()
    val edited get() = current != saved

    /** Saved somewhere else (the console, a sync): follow it, unless there are edits here. */
    fun follow() {
        val now = saved
        if (now == base) return
        val untouched = current == base
        base = now
        if (untouched) {
            draft.clear()
            draft.putAll(toDraft(now))
            rules.clear()
            rules.addAll(toRules(now))
            idFallback = now.idFallback
        }
    }

    fun addRule() {
        done = false
        rules += RuleDraft(nextKey++, "", "", listOf("description"))
    }

    fun moveRule(i: Int, by: Int) {
        done = false
        rules.add(i + by, rules.removeAt(i))
    }

    fun edit(i: Int, change: (RuleDraft) -> RuleDraft) {
        done = false
        rules[i] = change(rules[i])
    }

    fun pickField(i: Int, field: String, on: Boolean) = edit(i) { r -> r.copy(fields = if (on) r.fields + field else r.fields - field) }

    fun toggle(name: String, on: Boolean) {
        done = false
        if (on) draft[name] = numberText(DEFAULT_PROGRESS.min) to numberText(DEFAULT_PROGRESS.max) else draft.remove(name)
    }

    suspend fun apply() {
        busy = true
        error = null
        done = false
        try {
            store.saveAppearance(current)
            base = saved
            done = true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }
}

/** A number as it is typed: `100`, not `100.0`. */
fun numberText(d: Double): String = if (d == kotlin.math.floor(d) && kotlin.math.abs(d) < 1e15) d.toLong().toString() else d.toString()

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun AppearancePanel(store: Store, state: AppearanceState) {
    state.follow()
    val scope = rememberCoroutineScope()
    val colors = MaterialTheme.colorScheme
    Column(Modifier.padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("IDs", fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
        Row(
            Modifier.fillMaxWidth().heightIn(min = 44.dp).clickable { state.idFallback = !state.idFallback; state.done = false },
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Checkbox(state.idFallback, onCheckedChange = null, modifier = Modifier.padding(horizontal = 8.dp))
            Text("Fall back to UUID.short when ID is missing (e.g. completed or deleted)", fontSize = 15.sp)
        }
        Dim("Off, a task with no ID shows - in the ID column, as task all does. Either way, copying the cell gives the whole uuid.")

        Text("Progress bars", Modifier.padding(top = 16.dp), fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
        Dim(
            "A numeric UDA shown as a bar in reports: empty at its minimum, full at its maximum. A value outside the range fills or " +
                "empties the bar; a task with no value shows nothing. It is kept in this app's own settings.json, not in your taskrc.",
        )
        store.appSettings?.appearanceErrors?.takeIf { it.isNotEmpty() }?.let { Text(it.joinToString(" "), color = colors.error, fontSize = 13.sp) }
        if (state.names.isEmpty()) Dim("No numeric UDAs yet. Define one in the taskrc (uda.progress.type=numeric).")
        for (name in state.names) {
            val on = name in state.draft
            Row(Modifier.fillMaxWidth().heightIn(min = 44.dp).clickable { state.toggle(name, !on) }, verticalAlignment = Alignment.CenterVertically) {
                Checkbox(on, onCheckedChange = null, modifier = Modifier.padding(horizontal = 8.dp))
                Text(name, fontSize = 15.sp, fontFamily = FontFamily.Monospace, fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal)
            }
            if (on) {
                val (min, max) = state.draft.getValue(name)
                Row(Modifier.padding(start = 16.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    for ((label, value, set) in listOf(
                        Triple("min", min) { v: String -> state.draft[name] = v to max },
                        Triple("max", max) { v: String -> state.draft[name] = min to v },
                    )) {
                        OutlinedTextField(
                            value, { set(it); state.done = false }, Modifier.weight(1f), singleLine = true, label = { Text(label) },
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                        )
                    }
                }
                state.problem(name)?.let { Text(it, Modifier.padding(start = 16.dp), color = colors.error, fontSize = 13.sp) }
            }
        }

        Text("Transformations", Modifier.padding(top = 16.dp), fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
        Dim(
            "Find and replace in a text field before it is drawn, so BOR-23434 can become a link: the result is shown as Markdown. " +
                "Match is a regular expression, Replace may use \$1 or \${name}, and each runs only on the fields ticked. They run in " +
                "order, on what the one before left. Only how a field is shown changes: the task, filters and exports keep what you typed.",
        )
        if (state.rules.isEmpty()) Dim("No transformations yet.")
        state.rules.forEachIndexed { i, r ->
            Column(Modifier.padding(top = 4.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text("${i + 1}.", Modifier.weight(1f), fontWeight = FontWeight.SemiBold)
                    IconAction("arrow-up", "Move up", { state.moveRule(i, -1) }, size = 16.dp, enabled = i > 0)
                    IconAction("arrow-down", "Move down", { state.moveRule(i, 1) }, size = 16.dp, enabled = i < state.rules.size - 1)
                    IconAction("x", "Remove transformation ${i + 1}", { state.rules.removeAt(i); state.done = false }, size = 16.dp)
                }
                OutlinedTextField(
                    r.match, { v -> state.edit(i) { it.copy(match = v) } }, Modifier.fillMaxWidth(), singleLine = true,
                    label = { Text("Match") }, placeholder = { Text("\\b(BOR-\\d+)\\b") },
                    textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                )
                OutlinedTextField(
                    r.replace, { v -> state.edit(i) { it.copy(replace = v) } }, Modifier.fillMaxWidth(), singleLine = true,
                    label = { Text("Replace") }, placeholder = { Text("[\$1](https://jira.example.com/browse/\$1)") },
                    textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
                )
                FlowRow {
                    for (f in state.fieldsOf(r)) {
                        val on = f in r.fields
                        Row(Modifier.heightIn(min = 40.dp).clickable { state.pickField(i, f, !on) }, verticalAlignment = Alignment.CenterVertically) {
                            Checkbox(on, onCheckedChange = null, modifier = Modifier.padding(horizontal = 8.dp))
                            Text(f, Modifier.padding(end = 8.dp), fontSize = 14.sp, fontFamily = FontFamily.Monospace)
                        }
                    }
                }
            }
        }
        state.ruleProblems.forEach { Text(it, color = colors.error, fontSize = 13.sp) }
        OutlinedButton(onClick = state::addRule, modifier = Modifier.padding(bottom = 10.dp)) { LucideIcon("plus", size = 14.dp); Text("  Add a transformation") }

        state.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            Button({ scope.launch { state.apply() } }, enabled = !state.busy && state.edited && !state.invalid) { Text("Apply") }
            if (state.done) Dim("Saved")
        }
    }
}
