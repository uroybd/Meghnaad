package me.utsob.meghnaad.app

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * An iterative task's rule (`iter`), as a button that opens a dialog to choose it in, as a date field does. Two modes:
 * Taskwarrior (its periods as pills, or anything `task` takes, typed), and Calendar, which builds an RRULE as calendar
 * apps do: repeat every N days, weeks, months or years; on which weekdays or which day of the month; ending never, on a
 * day, or after a number of times. `anchor` is the day a new rule counts from (the task's due date, else today).
 * The web's is IterField.svelte.
 */
@Composable
fun IterInput(value: String, onChange: (String) -> Unit, anchor: String, modifier: Modifier = Modifier) {
    var open by remember { mutableStateOf(false) }
    val colors = MaterialTheme.colorScheme
    val words = remember(value) { if (value.isBlank()) "" else runCatching { describeIter(value) }.getOrDefault(value) }
    OutlinedButton(
        onClick = { open = true },
        modifier.fillMaxWidth(),
        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
    ) {
        Text(
            words.ifEmpty { "Set rule" },
            Modifier.weight(1f),
            fontSize = 14.sp,
            color = if (words.isEmpty()) colors.onSurfaceVariant else colors.onSurface,
            maxLines = 2,
        )
        Spacer(Modifier.width(8.dp))
        LucideIcon("repeat", size = 14.dp)
    }
    if (open) IterPanel(value, onChange, anchor) { open = false }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun IterPanel(value: String, onChange: (String) -> Unit, anchor: String, onClose: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    val start = remember { parseRule(value) }
    var calendar by remember { mutableStateOf(start != null) }
    var rule by remember { mutableStateOf(start) }
    // What the Taskwarrior mode had, kept while the Calendar mode is tried, so switching back restores it.
    var typed by remember { mutableStateOf(if (start == null) value else "") }
    var pickUntil by remember { mutableStateOf(false) }
    fun edit(r: CalendarRule) {
        rule = r
        onChange(buildRule(r))
    }

    AlertDialog(
        onDismissRequest = onClose,
        title = { Text("Iter", fontSize = 16.sp) },
        text = {
            Column(Modifier.verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(10.dp)) {
                // The two modes, side by side.
                Row(Modifier.fillMaxWidth().clip(RoundedCornerShape(8.dp)).border(1.dp, colors.outlineVariant, RoundedCornerShape(8.dp))) {
                    for ((on, label) in listOf(false to "Taskwarrior", true to "Calendar (RRULE)")) {
                        Box(
                            Modifier.weight(1f)
                                .background(if (calendar == on) colors.primary else colors.surface)
                                .clickable {
                                    if (calendar == on) return@clickable
                                    calendar = on
                                    if (on) {
                                        typed = value
                                        edit(parseRule(value) ?: defaultRule(anchor))
                                    } else {
                                        onChange(typed)
                                    }
                                }
                                .semantics { selected = calendar == on }
                                .padding(vertical = 8.dp),
                            contentAlignment = Alignment.Center,
                        ) { Text(label, fontSize = 13.sp, color = if (calendar == on) colors.onPrimary else colors.onSurface) }
                    }
                }
                val r = rule
                if (!calendar) {
                    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        PRESETS.forEach { p ->
                            val on = value.trim() == p.value
                            OutlinedButton(
                                onClick = { onChange(p.value) },
                                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 0.dp),
                                border = BorderStroke(1.dp, if (on) colors.primary else colors.outlineVariant),
                            ) { Text(p.label, fontSize = 13.sp, color = if (on) colors.primary else colors.onSurface) }
                        }
                    }
                    OutlinedTextField(
                        value, onChange, Modifier.fillMaxWidth(), singleLine = true,
                        label = { Text("Or as task takes it", fontSize = 12.sp) },
                        placeholder = { Text("3d, 2wk, P2W, every other day…", fontSize = 13.sp) },
                        textStyle = androidx.compose.material3.LocalTextStyle.current.copy(fontFamily = FontFamily.Monospace, fontSize = 14.sp),
                        keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                    )
                } else if (r != null) {
                    Dim("Repeats every", size = 12)
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedTextField(
                            r.interval.toString(),
                            { t -> t.filter(Char::isDigit).toIntOrNull()?.takeIf { it >= 1 }?.let { edit(r.copy(interval = it)) } },
                            Modifier.width(76.dp), singleLine = true,
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        )
                        val units = listOf("daily" to "day", "weekly" to "week", "monthly" to "month", "yearly" to "year")
                        val names = units.map { (_, u) -> if (r.interval == 1) u else "${u}s" }
                        Choice(names[units.indexOfFirst { it.first == r.freq }.coerceAtLeast(0)], names) { i ->
                            val freq = units[i].first
                            edit(
                                r.copy(
                                    freq = freq,
                                    days = if (freq == "weekly") r.days.ifEmpty { defaultRule(anchor).days } else emptyList(),
                                    monthDay = if (freq == "monthly") monthChoices(anchor)?.day else null,
                                    nth = null, weekday = null,
                                ),
                            )
                        }
                    }
                    if (r.freq == "weekly") {
                        Dim("Repeats on", size = 12)
                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            WEEKDAYS.forEachIndexed { i, d ->
                                val on = d in r.days
                                Box(
                                    Modifier.size(36.dp).clip(CircleShape)
                                        .background(if (on) colors.primary else colors.surface)
                                        .border(1.dp, if (on) colors.primary else colors.outline, CircleShape)
                                        .clickable {
                                            // At least one day stays on: none is the start date's anyway.
                                            if (on && r.days.size == 1) return@clickable
                                            edit(r.copy(days = WEEKDAYS.filter { x -> if (x == d) !on else x in r.days }))
                                        }
                                        .semantics { contentDescription = DAY_NAMES[i]; selected = on },
                                    contentAlignment = Alignment.Center,
                                ) { Text(DAY_NAMES[i].take(1), fontSize = 13.sp, color = if (on) colors.onPrimary else colors.onSurface) }
                            }
                        }
                    }
                    if (r.freq == "monthly") {
                        val options = monthOptions(r, anchor)
                        if (options.isNotEmpty()) {
                            val at = options.indexOfFirst { it.key == monthKey(r) }.coerceAtLeast(0)
                            Choice(options[at].label, options.map { it.label }) { i -> edit(options[i].apply(r)) }
                        }
                    }
                    Dim("Ends", size = 12)
                    val ends = when {
                        r.until != null -> "on"
                        r.count != null -> "after"
                        else -> "never"
                    }
                    EndRow(ends == "never", { edit(r.copy(until = null, count = null)) }) { Text("Never", fontSize = 14.sp) }
                    EndRow(ends == "on", { edit(r.copy(until = anchor, count = null)) }) {
                        Text("On", fontSize = 14.sp)
                        Spacer(Modifier.width(8.dp))
                        OutlinedButton(onClick = { pickUntil = true }, enabled = ends == "on", contentPadding = PaddingValues(horizontal = 12.dp, vertical = 0.dp)) {
                            LucideIcon("calendar", size = 14.dp)
                            Text("  " + (r.until ?: anchor), fontSize = 14.sp)
                        }
                    }
                    EndRow(ends == "after", { edit(r.copy(count = 10, until = null)) }) {
                        Text("After", fontSize = 14.sp)
                        Spacer(Modifier.width(8.dp))
                        OutlinedTextField(
                            (r.count ?: 10).toString(),
                            { t -> t.filter(Char::isDigit).toIntOrNull()?.takeIf { it >= 1 }?.let { edit(r.copy(count = it, until = null)) } },
                            Modifier.width(76.dp), singleLine = true, enabled = ends == "after",
                            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                        )
                        Spacer(Modifier.width(8.dp))
                        Text("times", fontSize = 14.sp)
                    }
                    Text(value, fontSize = 12.sp, fontFamily = FontFamily.Monospace, color = colors.onSurfaceVariant)
                }
                if (value.isNotBlank()) Text(remember(value) { runCatching { describeIter(value) }.getOrDefault(value) }, fontSize = 13.sp)
            }
        },
        confirmButton = { TextButton(onClose) { Text("Done") } },
        dismissButton = {
            TextButton({
                onChange("")
                typed = ""
                rule = null
                calendar = false
            }, enabled = value.isNotEmpty()) { Text("Clear") }
        },
    )
    val r = rule
    if (pickUntil && r != null) {
        val initial = (r.until ?: anchor).split('-').map { it.toInt() }
        val state = rememberDatePickerState(initialSelectedDateMillis = daysFromCivil(initial[0], initial[1], initial[2]) * 86_400_000L)
        DatePickerDialog(
            onDismissRequest = { pickUntil = false },
            confirmButton = {
                TextButton(onClick = {
                    state.selectedDateMillis?.let { ms -> edit(r.copy(until = isoDate(ms / 1000, 0), count = null)) }
                    pickUntil = false
                }) { Text("OK") }
            },
            dismissButton = { TextButton(onClick = { pickUntil = false }) { Text("Cancel") } },
        ) { DatePicker(state) }
    }
}

/** One way the rule ends: a radio button and what goes with it. */
@Composable
private fun EndRow(selected: Boolean, onSelect: () -> Unit, content: @Composable () -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 44.dp).clickable(onClick = onSelect), verticalAlignment = Alignment.CenterVertically) {
        RadioButton(selected, onClick = onSelect)
        content()
    }
}
