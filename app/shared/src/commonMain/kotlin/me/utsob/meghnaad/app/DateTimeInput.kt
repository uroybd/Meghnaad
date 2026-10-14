package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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

/**
 * A date and an optional time. The value is `""`, `YYYY-MM-DD` (a whole day) or `YYYY-MM-DDTHH:MM` (a moment), in the user's own
 * time zone: the form the engine's date parser takes (DateTimeInput.svelte).
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun DateTimeInput(value: String, onChange: (String) -> Unit, label: String, now: Long, offset: Int, modifier: Modifier = Modifier) {
    val date = value.take(10)
    val time = if (value.length > 10) value.substring(11, 16) else ""
    var pickDate by remember { mutableStateOf(false) }
    var pickTime by remember { mutableStateOf(false) }
    fun dayOffset(days: Int) = isoDate(now + days * 86_400L, offset)

    androidx.compose.foundation.layout.Column(modifier) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(2.dp), itemVerticalAlignment = Alignment.CenterVertically) {
            OutlinedButton(onClick = { pickDate = true }, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
                LucideIcon("calendar", size = 14.dp)
                Text("  " + date.ifEmpty { "Set date" }, fontSize = 14.sp)
            }
            OutlinedButton(onClick = { pickTime = true }, enabled = date.isNotEmpty(), contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 12.dp, vertical = 4.dp)) {
                LucideIcon("clock", size = 14.dp)
                Text("  " + time.ifEmpty { "Time (optional)" }, fontSize = 14.sp)
            }
        }
        // The shortcuts, close together under the two buttons.
        val tight = androidx.compose.foundation.layout.PaddingValues(horizontal = 10.dp, vertical = 0.dp)
        androidx.compose.foundation.layout.Row(verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { onChange(fromParts(DateParts(dayOffset(0), time))) }, contentPadding = tight) { Text("today", fontSize = 13.sp) }
            TextButton(onClick = { onChange(fromParts(DateParts(dayOffset(1), time))) }, contentPadding = tight) { Text("tmrw", fontSize = 13.sp) }
            TextButton(onClick = { onChange(fromParts(DateParts(dayOffset(7), time))) }, contentPadding = tight) { Text("+1w", fontSize = 13.sp) }
            if (time.isNotEmpty()) TextButton(onClick = { onChange(date) }, contentPadding = tight) { Text("no time", fontSize = 13.sp) }
            if (value.isNotEmpty()) IconAction("x", "Clear $label", { onChange("") }, size = 14.dp)
        }
    }

    if (pickDate) {
        val initial = (if (isValidDate(date)) date else dayOffset(0)).split('-').map { it.toInt() }
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

@Composable
fun FieldLabel(text: String, modifier: Modifier = Modifier) {
    Text(text, modifier.padding(top = 14.dp, bottom = 4.dp), fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
}
