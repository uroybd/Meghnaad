package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.OutlinedTextField
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The words offered as buttons under the field, besides the ones the typed text begins. */
private val DATE_KEYWORDS = listOf("now", "today", "tomorrow", "eod", "sow", "eow", "som", "eom")

/**
 * A date, shown as a button that opens a panel to choose it in (DateField.svelte). The panel has a field to type it (a date as Taskwarrior
 * reads one: `sow`, `eod`, `eow-2d`, `2026-10-01`; the engine's date words complete as it is typed and a tap takes one) and, under it, a
 * calendar day and an optional time. What is typed is kept as typed; the calendar writes `YYYY-MM-DD`, or `YYYY-MM-DDTHH:MM` with a time.
 * Empty is no date.
 */
@Composable
fun DateTimeInput(
    value: String,
    onChange: (String) -> Unit,
    label: String,
    now: Long,
    offset: Int,
    modifier: Modifier = Modifier,
    placeholder: String = "Set date",
    invalid: Boolean = false,
) {
    var open by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    OutlinedButton(
        onClick = { open = true },
        modifier,
        contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp),
    ) {
        Text(
            value.ifEmpty { placeholder },
            fontSize = 14.sp,
            fontFamily = if (value.isEmpty()) null else FontFamily.Monospace,
            color = if (invalid) colors.error else if (value.isEmpty()) colors.onSurfaceVariant else colors.onSurface,
            maxLines = 1,
        )
        Spacer(Modifier.width(8.dp))
        LucideIcon("calendar", size = 14.dp)
    }
    if (open) DatePanel(value, onChange, label, now, offset, invalid) { open = false }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun DatePanel(value: String, onChange: (String) -> Unit, label: String, now: Long, offset: Int, invalid: Boolean, onClose: () -> Unit) {
    val date = if (Regex("^\\d{4}-\\d{2}-\\d{2}").containsMatchIn(value)) value.take(10) else ""
    val time = if (date.isNotEmpty() && Regex("^.{10}[T ]\\d{2}:\\d{2}").containsMatchIn(value)) value.substring(11, 16) else ""
    val words = remember(value) { runCatching { dateWords(value) }.getOrDefault(emptyList()) }
    var pickDate by remember { mutableStateOf(false) }
    var pickTime by remember { mutableStateOf(false) }
    val tight = androidx.compose.foundation.layout.PaddingValues(horizontal = 10.dp, vertical = 0.dp)

    AlertDialog(
        onDismissRequest = onClose,
        title = { Text(label, fontSize = 16.sp) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                OutlinedTextField(
                    value, onChange, Modifier.fillMaxWidth(),
                    singleLine = true, isError = invalid,
                    textStyle = LocalTextStyle.current.copy(fontFamily = FontFamily.Monospace, fontSize = 14.sp),
                    placeholder = { Text("sow, eod, 2026-10-01 …", fontSize = 13.sp) },
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.None, autoCorrectEnabled = false, imeAction = ImeAction.Done),
                    keyboardActions = KeyboardActions(onDone = { onClose() }),
                )
                // What the typed text could become: a tap takes it.
                if (words.isNotEmpty()) {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        words.forEach { w ->
                            OutlinedButton({ onChange(w.value) }, contentPadding = tight) {
                                Text(w.value, fontSize = 13.sp, fontFamily = FontFamily.Monospace)
                                w.hint?.let { Text("  $it", fontSize = 11.sp, color = MaterialTheme.colorScheme.onSurfaceVariant) }
                            }
                        }
                    }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                    DATE_KEYWORDS.forEach { w ->
                        TextButton({ onChange(w) }, contentPadding = tight) {
                            Text(w, fontSize = 13.sp, fontFamily = FontFamily.Monospace, fontWeight = if (value.trim() == w) FontWeight.Bold else FontWeight.Normal)
                        }
                    }
                }
                FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(2.dp), itemVerticalAlignment = Alignment.CenterVertically) {
                    OutlinedButton(onClick = { pickDate = true }, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
                        LucideIcon("calendar", size = 14.dp)
                        Text("  " + date.ifEmpty { "Pick a day" }, fontSize = 14.sp)
                    }
                    OutlinedButton(onClick = { pickTime = true }, enabled = date.isNotEmpty(), contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
                        LucideIcon("clock", size = 14.dp)
                        Text("  " + time.ifEmpty { "Time (optional)" }, fontSize = 14.sp)
                    }
                    if (time.isNotEmpty()) TextButton({ onChange(date) }, contentPadding = tight) { Text("no time", fontSize = 13.sp) }
                }
                if (value.isNotBlank() && date.isEmpty()) Dim("Typed as a date expression; pick a day to replace it.", size = 12)
            }
        },
        confirmButton = { TextButton(onClose) { Text("Done") } },
        dismissButton = { TextButton({ onChange("") }, enabled = value.isNotEmpty()) { Text("Clear") } },
    )

    if (pickDate) {
        val initial = (if (isValidDate(date)) date else isoDate(now, offset)).split('-').map { it.toInt() }
        val state = rememberDatePickerState(initialSelectedDateMillis = daysFromCivil(initial[0], initial[1], initial[2]) * 86_400_000L)
        DatePickerDialog(
            onDismissRequest = { pickDate = false },
            confirmButton = {
                TextButton(onClick = {
                    state.selectedDateMillis?.let { ms -> onChange(fromParts(DateParts(isoDate(ms / 1000, 0), time))) }
                    pickDate = false
                }) { Text("OK") }
            },
            dismissButton = { TextButton(onClick = { pickDate = false }) { Text("Cancel") } },
        ) { DatePicker(state) }
    }
    if (pickTime) {
        val state = rememberTimePickerState(
            initialHour = time.take(2).toIntOrNull() ?: 9,
            initialMinute = time.drop(3).toIntOrNull() ?: 0,
            is24Hour = true,
        )
        AlertDialog(
            onDismissRequest = { pickTime = false },
            text = { TimePicker(state) },
            confirmButton = {
                TextButton(onClick = {
                    onChange(fromParts(DateParts(date, "${state.hour.toString().padStart(2, '0')}:${state.minute.toString().padStart(2, '0')}")))
                    pickTime = false
                }) { Text("OK") }
            },
            dismissButton = { TextButton(onClick = { pickTime = false }) { Text("Cancel") } },
        )
    }
}

/** A labelled field for a [FieldGrid]: its share of the row is [weight]. */
class FieldCell(val label: String, val weight: Float = 1f, val content: @Composable () -> Unit)

/**
 * Labelled fields side by side, as many to a row as fit the width at [minCell] each (the rest go to the next row); a row's cells share it
 * by their weights. It is the form's compact layout (TaskEditor.svelte: `.pair`).
 */
@Composable
fun FieldGrid(minCell: androidx.compose.ui.unit.Dp, cells: List<FieldCell>, modifier: Modifier = Modifier) {
    androidx.compose.foundation.layout.BoxWithConstraints(modifier.fillMaxWidth().padding(top = 14.dp)) {
        val columns = (maxWidth / minCell).toInt().coerceIn(1, cells.size)
        // Rows of as even a count as there is: four cells at three columns are two and two, not three and one.
        val rows = (cells.size + columns - 1) / columns
        val per = (cells.size + rows - 1) / rows
        Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
            cells.chunked(per).forEach { row ->
                androidx.compose.foundation.layout.Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    row.forEach { c ->
                        Column(Modifier.weight(c.weight)) {
                            Text(c.label, Modifier.padding(bottom = 4.dp), fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            c.content()
                        }
                    }
                }
            }
        }
    }
}

@Composable
fun FieldLabel(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier.padding(top = 14.dp, bottom = 4.dp), fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
}
