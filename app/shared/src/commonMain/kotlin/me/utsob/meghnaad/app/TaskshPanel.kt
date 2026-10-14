package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The tasksh tab's edits: kept while another tab is shown (TaskshPanel.svelte). */
class TaskshState(private val store: Store) {
    val status get() = taskshStatus(store.config)
    val saved get() = ReviewWanted(status.filter ?: REVIEW_FILTER, status.sort ?: REVIEW_SORT)

    var filter by mutableStateOf(saved.filter)
    var sort by mutableStateOf(saved.sort)
    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var done by mutableStateOf(false)

    val want get() = ReviewWanted(filter.trim(), sort.trim())
    val valid get() = want.filter.isNotEmpty() && want.sort.isNotEmpty() && '\n' !in filter + sort && '\r' !in filter + sort
    /** What Apply would write: nothing when the taskrc already is as wanted. */
    val lines get() = setupLines(store.config, want)
    val edited get() = want != saved
    val blocked get() = store.config?.configError != null

    fun revert() {
        filter = saved.filter
        sort = saved.sort
    }

    fun defaults() {
        filter = REVIEW_FILTER
        sort = REVIEW_SORT
        done = false
    }

    // One `config` line at a time: each reads the saved settings and writes them back.
    suspend fun apply() {
        busy = true
        error = null
        done = false
        try {
            for (line in lines) store.query(configArgs(line))
            store.settingsChanged()
            done = true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
            store.settingsChanged()
        } finally {
            busy = false
        }
    }
}

@Composable
fun TaskshPanel(store: Store, st: TaskshState) {
    val colors = MaterialTheme.colorScheme
    // The saved report changed somewhere else (the taskrc tab, the console): follow it, unless there are edits here.
    var before by remember { mutableStateOf(st.saved) }
    LaunchedEffect(st.saved) {
        val now = st.saved
        val untouched = st.want == before
        val changed = now != before
        before = now
        if (changed && untouched) st.revert()
    }
    val status = st.status

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("tasksh", fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
        Dim(
            "tasksh's review walks the tasks you have not looked at lately and marks each as reviewed. It needs two things in your taskrc: a reviewed date " +
                "attribute (UDA) that records when, and a _reviewed report that lists the tasks due. Apply writes whichever is missing, with the same lines " +
                "tasksh writes the first time it runs, and any change you make to the filter or order below.",
        )
        if (st.blocked) Text("The saved settings couldn't be read, so nothing can be applied until that is fixed in the taskrc tab.", color = colors.error, fontSize = 13.sp)

        PartRow("reviewed attribute, type date", status.uda, if (status.uda == Part.Different) "It is a ${status.udaType} now. Applying changes its type to date." else null)
        PartRow("_reviewed report with one uuid column", status.report, if (status.report == Part.Different) "It has the columns ${status.columns.joinToString(", ")} now. Applying replaces them with uuid." else null)

        Text("Tasks due for review", fontWeight = FontWeight.Medium)
        Text("report._reviewed.filter", fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.onSurfaceVariant)
        OutlinedTextField(
            st.filter, { st.filter = it; st.done = false }, Modifier.fillMaxWidth(),
            textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp),
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
        )
        Dim("By default: never reviewed, or not for 6 days, and still pending or waiting. For a month, use reviewed.before:now-30days.", size = 12)
        Text("Order", fontWeight = FontWeight.Medium)
        Text("report._reviewed.sort", fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.onSurfaceVariant)
        OutlinedTextField(
            st.sort, { st.sort = it; st.done = false }, Modifier.fillMaxWidth(), singleLine = true,
            textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 13.sp),
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
        )
        Dim("By default the longest-ago first, then the least recently changed.", size = 12)

        val lines = st.lines
        if (lines.isNotEmpty()) {
            Column {
                Text("Apply will write ${lines.size} setting${if (lines.size == 1) "" else "s"}:", fontWeight = FontWeight.SemiBold, fontSize = 13.sp)
                lines.forEach { (n, v) -> Text("$n=$v", fontFamily = FontFamily.Monospace, fontSize = 11.sp) }
            }
        } else if (status.enabled) Dim("The taskrc already has all of this.")

        st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        if (st.done && !st.edited) Text("Applied. review can use it now.", color = extras.ok, fontSize = 13.sp)
        Row(Modifier.fillMaxWidth().padding(bottom = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
            if (st.edited) TextButton(onClick = st::revert) { Text("Revert edits") }
            TextButton(onClick = st::defaults, enabled = st.filter != REVIEW_FILTER || st.sort != REVIEW_SORT) { Text("Use tasksh's defaults") }
            Button(onClick = { store.go { st.apply() } }, enabled = !st.busy && !st.blocked && st.valid && lines.isNotEmpty()) { Text("Apply") }
        }
    }
}

@Composable
private fun PartRow(what: String, part: Part, hint: String?) {
    Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.Top) {
        LucideIcon(if (part == Part.Ok) "check" else "triangle-alert", size = 16.dp, tint = if (part == Part.Ok) extras.ok else extras.warn)
        Column {
            Text("$what: ${when (part) { Part.Ok -> "defined"; Part.Missing -> "not defined"; Part.Different -> "defined differently" }}", fontSize = 14.sp)
            hint?.let { Dim(it, size = 12) }
        }
    }
}
