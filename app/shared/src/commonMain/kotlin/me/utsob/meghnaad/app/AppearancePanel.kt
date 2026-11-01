package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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

/** How the app draws things, kept in `settings.json` (AppearancePanel.svelte): the numeric UDAs shown as progress bars. */
class AppearanceState(private val store: Store) {
    private val saved get() = store.appearance

    /** The UDAs shown as bars, with the min and max as typed (so a half-typed number is not lost). */
    val draft = mutableStateMapOf<String, Pair<String, String>>().also { it.putAll(toDraft(saved)) }

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

    private val current get() = Appearance(draft.keys.mapNotNull { n -> range(n)?.let { n to it } }.toMap())
    val invalid get() = draft.keys.any { problem(it) != null }
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
        }
    }

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

@Composable
fun AppearancePanel(store: Store, state: AppearanceState) {
    state.follow()
    val scope = rememberCoroutineScope()
    val colors = MaterialTheme.colorScheme
    Column(Modifier.padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("Progress bars", fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
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
        state.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            Button({ scope.launch { state.apply() } }, enabled = !state.busy && state.edited && !state.invalid) { Text("Apply") }
            if (state.done) Dim("Saved")
        }
    }
}
