package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.serialization.json.JsonPrimitive

// A board's controls (BoardFilter.svelte and ControlField.svelte): each set above the board, over the board's own value while the app
// is open, and the same field for the value a control starts with in the board's settings.

/**
 * One control as it is set (ControlField.svelte): a date range's two dates, a date, a number, a least and a most, a selection of the
 * values its source has (one, or several), or a switch. What is set goes to [onChange] as the control with that value.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ControlField(store: Store, control: Control, onChange: (Control) -> Unit, invalid: Boolean = false) {
    val label = controlLabel(control)
    when (control.type) {
        "daterange" -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            DateTimeInput(control.start.orEmpty(), { onChange(control.copy(start = it)) }, "Start of $label", store.now, store.utcOffset, invalid = invalid, placeholder = "start")
            Dim("–")
            DateTimeInput(control.end.orEmpty(), { onChange(control.copy(end = it)) }, "End of $label", store.now, store.utcOffset, invalid = invalid, placeholder = "end")
        }
        "date" -> DateTimeInput(control.text, { onChange(control.copy(value = JsonPrimitive(it))) }, label, store.now, store.utcOffset, invalid = invalid)
        "number" -> NumberBox(label, control.text, invalid) { onChange(control.copy(value = JsonPrimitive(it))) }
        "numrange" -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            NumberBox("Least of $label", control.min.orEmpty(), invalid) { onChange(control.copy(min = it)) }
            Dim("–")
            NumberBox("Most of $label", control.max.orEmpty(), invalid) { onChange(control.copy(max = it)) }
        }
        "select" -> {
            var choices by remember { mutableStateOf(control.values.orEmpty()) }
            LaunchedEffect(control.source, control.completed, control.values, store.rev) {
                choices = runCatching { store.selectChoices(control) }.getOrDefault(control.values.orEmpty())
            }
            val picked = control.values.orEmpty()
            if (control.multiple == true) {
                var open by remember { mutableStateOf(false) }
                Box {
                    OutlinedButton({ open = true }, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
                        Text(picked.joinToString(", ").ifEmpty { "Any" }, fontSize = 13.sp, maxLines = 1)
                        LucideIcon("chevron-down", Modifier.padding(start = 4.dp), size = 14.dp)
                    }
                    DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
                        if (choices.isEmpty()) Dim("Nothing to choose yet.", Modifier.padding(12.dp))
                        choices.forEach { c ->
                            Row(
                                Modifier.clickable { onChange(control.copy(values = if (c in picked) picked - c else picked + c)) }.padding(horizontal = 12.dp, vertical = 4.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Checkbox(c in picked, onCheckedChange = null)
                                Text(c, fontSize = 14.sp, modifier = Modifier.padding(start = 6.dp))
                            }
                        }
                    }
                }
            } else {
                val options = listOf("Any") + choices
                Choice(picked.firstOrNull() ?: "Any", options) { i -> onChange(control.copy(values = if (i == 0) null else listOf(choices[i - 1]))) }
            }
        }
        "boolean" -> Switch(control.isOn, { onChange(control.copy(value = JsonPrimitive(it))) })
    }
}

@Composable
private fun NumberBox(label: String, value: String, invalid: Boolean, onChange: (String) -> Unit) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        modifier = Modifier.width(96.dp),
        placeholder = { Text(label, fontSize = 12.sp, maxLines = 1) },
        textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
        isError = invalid,
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
    )
}

/** The board's controls above it (BoardFilter.svelte): each to change, what a date is now, and a way back to the board's own value. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BoardControls(store: Store, board: Board) {
    if (board.controls.isEmpty()) return
    val typed = store.boardControls[board.id].orEmpty()
    val shown = viewedBoard(board, null, typed).controls
    val zone = store.boardZone()
    val resolved = remember(shown, store.rev) { runCatching { resolveControls(shown, zone) }.getOrDefault(emptyList()) }
    val fmt = formatFor(FormatKind.Info, store.config?.config?.settings)
    fun keep(name: String, now: Control?) {
        val mine = typed.toMutableMap()
        if (now == null) mine.remove(name) else mine[name] = now
        if (mine.isEmpty()) store.boardControls.remove(board.id) else store.boardControls[board.id] = mine
    }
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        board.controls.forEachIndexed { i, saved ->
            val now = shown[i]
            val got = resolved.firstOrNull { it.name == saved.name }
            FlowRow(verticalArrangement = Arrangement.Center, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(controlLabel(saved), Modifier.align(Alignment.CenterVertically), fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                Box(Modifier.align(Alignment.CenterVertically)) {
                    // Setting the board's own value back is no change.
                    ControlField(store, now, { keep(saved.name, if (it.sameValues(saved)) null else it) }, invalid = got?.error != null)
                }
                IconAction("rotate-ccw", "Use the board's own value for ${controlLabel(saved)}", { keep(saved.name, null) }, enabled = typed.containsKey(saved.name), size = 18.dp)
            }
            val line = got?.error ?: when {
                got?.start != null && got.end != null -> "${formatMoment(got.start, store.offsetAt(got.start), fmt)} – ${formatMoment(got.end, store.offsetAt(got.end), fmt)}"
                got?.start != null -> formatMoment(got.start, store.offsetAt(got.start), fmt)
                else -> ""
            }
            if (line.isNotEmpty()) {
                Text(
                    "$${saved.name}  $line", fontSize = 12.sp, fontFamily = FontFamily.Monospace,
                    color = if (got?.error != null) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (typed.isNotEmpty()) Dim("Values set here replace the board's own while you look at it; nothing is saved.", size = 12)
    }
}
