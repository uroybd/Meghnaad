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
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

// Changing a board on the phone (BoardsPage.svelte's edit mode, SectionForm.svelte, ColorPicker.svelte). The phone has one column, so
// a section is moved up or down in the order rather than dragged on a grid; everything else is the web's: the name, description,
// filter and address of the board, and for each section its title, type, filter, measure, grouping, order, series, colours and, for a
// burndown, the state a series counts. The engine says whether the draft is valid (BoardsEdit.kt).

@Composable
private fun TextBox(label: String, value: String, onChange: (String) -> Unit, modifier: Modifier = Modifier, mono: Boolean = false, error: String? = null, prefix: String? = null, number: Boolean = false, placeholder: String? = null) {
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        modifier = modifier.fillMaxWidth(),
        label = { Text(label, fontSize = 12.sp) },
        placeholder = placeholder?.let { { Text(it, fontSize = 13.sp) } },
        prefix = prefix?.let { { Text(it, fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant) } },
        textStyle = MaterialTheme.typography.bodyMedium.let { if (mono) it.copy(fontFamily = FontFamily.Monospace) else it },
        isError = error != null,
        supportingText = error?.let { { Text(it, fontSize = 11.sp) } },
        singleLine = true,
        keyboardOptions = if (number) KeyboardOptions(keyboardType = KeyboardType.Number) else KeyboardOptions.Default,
    )
}

/** A field with a label above it, for the dropdowns. */
@Composable
private fun Labeled(label: String, content: @Composable () -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Dim(label, size = 12)
        content()
    }
}

/** A dropdown of labelled values. */
@Composable
private fun Pick(label: String, options: List<Pair<String?, String>>, selected: String?, onPick: (String?) -> Unit) {
    Labeled(label) {
        Choice(options.firstOrNull { it.first == selected }?.second ?: selected.orEmpty(), options.map { it.second }) { onPick(options[it].first) }
    }
}

/** Suggestions for a field: tap one to use it. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun Suggestions(names: List<String>, onPick: (String) -> Unit) {
    FlowRow(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        names.forEach { n ->
            Box(Modifier.clip(RoundedCornerShape(4.dp)).background(MaterialTheme.colorScheme.surfaceContainerHigh).clickable { onPick(n) }.padding(horizontal = 8.dp, vertical = 4.dp)) {
                Text(n, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

/** Edit a board: its settings, then its sections, then Save. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun BoardEditScreen(store: Store, editor: BoardEditor, onDone: () -> Unit) {
    val scope = rememberCoroutineScope()
    val colors = MaterialTheme.colorScheme
    val d = editor.draft
    var confirmDelete by remember { mutableStateOf(false) }

    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(12.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            Text("Editing ${d.name}", Modifier.weight(1f), fontSize = 18.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
            TextButton(onDone, enabled = !editor.busy) { Text("Cancel") }
            Button({ scope.launch { if (editor.save()) onDone() } }, enabled = editor.valid && !editor.busy) { Text("Save") }
        }

        Card(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                TextBox("Name", d.name, { v -> editor.edit { it.copy(name = v) } })
                TextBox("Description", d.description.orEmpty(), { v -> editor.edit { it.copy(description = v.ifEmpty { null }) } }, placeholder = "optional")
                TextBox(
                    "Address", d.id, { v -> editor.edit { it.copy(id = v.trim().lowercase()) } },
                    mono = true, prefix = "/board/",
                    error = editor.slugError,
                )
                if (editor.slugError == null) Dim("The last part of the board's link, so it can be shared: meghnaad://board/${d.id}", size = 11)
                Labeled("Filter for every section") {
                    CompletingField(d.filter.orEmpty(), { v -> editor.edit { it.copy(filter = v.ifEmpty { null }) } }, store.vocab, placeholder = "optional: project:Home")
                }
            }
        }

        Text("Sections", fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
        val ordered = stacked(d.sections)
        if (ordered.isEmpty()) Dim("Nothing here yet. Add a section.")
        ordered.forEachIndexed { i, s ->
            SectionEditCard(store, editor, s, first = i == 0, last = i == ordered.lastIndex)
        }

        Dim("Add")
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
            SECTION_TYPES.forEach { t -> OutlinedButton({ editor.add(t.id) }, contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 10.dp, vertical = 4.dp)) { Text("+ ${t.label}", fontSize = 12.sp) } }
        }

        val problems = editor.check.errors
        if (problems.isNotEmpty()) problems.forEach { Text("• $it", color = colors.error, fontSize = 12.sp) }
        editor.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }

        TextButton({ confirmDelete = true }, enabled = !editor.busy) { Text("Delete this board", color = colors.error) }
    }

    if (confirmDelete) {
        AlertDialog(
            onDismissRequest = { confirmDelete = false },
            title = { Text("Delete ${d.name}?") },
            text = { Text("The board and its sections are removed from the settings. Your tasks are not touched.") },
            confirmButton = { TextButton({ confirmDelete = false; scope.launch { if (editor.delete()) onDone() } }) { Text("Delete", color = colors.error) } },
            dismissButton = { TextButton({ confirmDelete = false }) { Text("Keep") } },
        )
    }
}

@Composable
private fun SectionEditCard(store: Store, editor: BoardEditor, s: Section, first: Boolean, last: Boolean) {
    val open = editor.selected == s.id
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(start = 12.dp, end = 4.dp, top = 2.dp, bottom = if (open) 12.dp else 2.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Column(Modifier.weight(1f).clickable { editor.selected = if (open) null else s.id }) {
                    Text(s.title.ifEmpty { "(no title)" }, fontWeight = FontWeight.SemiBold, fontSize = 14.sp, maxLines = 1)
                    Dim(SECTION_TYPES.firstOrNull { it.id == s.type }?.label ?: s.type, size = 11)
                }
                IconAction("arrow-up", "Move ${s.title} up", { editor.move(s.id, -1) }, enabled = !first, size = 16.dp, box = 40.dp)
                IconAction("arrow-down", "Move ${s.title} down", { editor.move(s.id, 1) }, enabled = !last, size = 16.dp, box = 40.dp)
                IconAction(if (open) "chevron-up" else "pencil", "Settings of ${s.title}", { editor.selected = if (open) null else s.id }, size = 16.dp, box = 40.dp)
                IconAction("trash-2", "Remove ${s.title}", { editor.remove(s.id) }, size = 16.dp, box = 40.dp, tint = MaterialTheme.colorScheme.error)
            }
            if (open) SectionForm(store, s) { editor.update(it) }
        }
    }
}

/** The settings of one section. */
@Composable
private fun SectionForm(store: Store, s: Section, onChange: (Section) -> Unit) {
    val udas = store.udas
    Column(Modifier.padding(end = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        TextBox("Title", s.title, { onChange(s.copy(title = it)) })
        Pick("Shows", SECTION_TYPES.map { it.id to it.label }, s.type) { onChange(withType(s, it ?: s.type)) }
        Labeled("Filter") {
            CompletingField(s.filter.orEmpty(), { v -> onChange(s.copy(filter = v.ifEmpty { null })) }, store.vocab, placeholder = "Which tasks: project:Home +work")
        }

        if (s.type == "table") {
            Pick("Report", store.offeredReports.map { it.name to it.name }, s.report ?: "next") { onChange(s.copy(report = it)) }
            return@Column
        }

        Pick("Measure", MEASURES, s.measure.fn) { fn ->
            onChange(s.copy(measure = if (fn == "count" || fn == null) Measure("count") else Measure(fn, s.measure.field ?: measurableFields(udas).first())))
        }
        if (s.measure.fn != "count") {
            TextBox("Field", s.measure.field.orEmpty(), { onChange(s.copy(measure = Measure(s.measure.fn, it.trim()))) })
            Suggestions(measurableFields(udas)) { onChange(s.copy(measure = Measure(s.measure.fn, it))) }
        }

        if (isChartType(s.type)) {
            val dates = dateFields(udas)
            val time = s.by?.time
            Pick("Group by", listOf("field" to "The value of", "time" to "A date, by"), if (time != null) "time" else "field") { kind ->
                onChange(s.copy(by = if (kind == "time") By(time = TimeBy("entry", "week")) else By(field = "project")))
            }
            if (time != null) {
                Pick("Date", dates.map { it to it }, time.field) { onChange(s.copy(by = By(time = TimeBy(it ?: time.field, time.step)))) }
                Pick("Per", STEPS.map { it to it }, time.step) { onChange(s.copy(by = By(time = TimeBy(time.field, it ?: time.step)))) }
            } else {
                TextBox("Attribute", s.by?.field.orEmpty(), { onChange(s.copy(by = By(field = it.trim()))) })
                Suggestions(groupableFields(udas)) { onChange(s.copy(by = By(field = it))) }
            }
            Pick("Order", ORDERS, s.sort) { onChange(s.copy(sort = it)) }
            TextBox("Keep", s.limit?.toString().orEmpty(), { onChange(s.copy(limit = it.filter(Char::isDigit).toIntOrNull()?.takeIf { n -> n > 0 })) }, number = true, placeholder = "all")
        }

        if (hasManySeries(s.type)) SeriesEditor(store, s, onChange)
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun SeriesEditor(store: Store, s: Section, onChange: (Section) -> Unit) {
    val udas = store.udas
    fun setSeries(list: List<Series>) = onChange(s.copy(series = list))
    fun patch(i: Int, f: (Series) -> Series) = setSeries(s.series.mapIndexed { j, x -> if (j == i) f(x) else x })

    Column(Modifier.fillMaxWidth().border(BorderStroke(1.dp, MaterialTheme.colorScheme.outline), RoundedCornerShape(8.dp)).padding(10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text("Series", fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
        Dim("Each series is its own line or set of bars, of the tasks its filter picks (added to the filters above). With none, the chart is the one measure above.", size = 11)
        s.series.forEachIndexed { i, x ->
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    TextBox("Label of series ${i + 1}", x.label, { v -> patch(i) { it.copy(label = v) } }, modifier = Modifier.weight(1f))
                    IconAction("x", "Remove series ${i + 1}", { setSeries(s.series.filterIndexed { j, _ -> j != i }) }, size = 16.dp, box = 40.dp)
                }
                Labeled("Filter") {
                    CompletingField(x.filter.orEmpty(), { v -> patch(i) { it.copy(filter = v.ifEmpty { null }) } }, store.vocab, placeholder = "outcome:failed")
                }
                ColorChoice(x.color, i, extras.dark) { c -> patch(i) { it.copy(color = c) } }
                if (s.by?.time != null) {
                    Pick("Counts", listOf("period" to "What happened in each period") + SPAN_PRESETS.map { it.id to it.label } + ("custom" to "Open between two dates…"), spanKind(x)) { kind ->
                        patch(i) { it.copy(span = spanFor(kind ?: "period")) }
                    }
                    val span = x.span
                    if (span != null && spanKind(x) == "custom") {
                        val dates = dateFields(udas)
                        Pick("From", dates.map { it to it }, span.from) { v -> patch(i) { it.copy(span = span.copy(from = v ?: span.from)) } }
                        Pick("Until", listOf<Pair<String?, String>>(null to "never") + dates.map { it to it }, span.to) { v -> patch(i) { it.copy(span = span.copy(to = v)) } }
                        Pick("Unless", listOf<Pair<String?, String>>(null to "nothing") + dates.map { it to "$it has passed" }, span.unless) { v -> patch(i) { it.copy(span = span.copy(unless = v)) } }
                    }
                }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton({ setSeries(s.series + Series("Series ${s.series.size + 1}")) }, enabled = s.series.size < MAX_SERIES) { Text("+ Add series", fontSize = 12.sp) }
            if (s.series.size > 1 || s.stacked == true) {
                Row(Modifier.clickable { onChange(s.copy(stacked = if (s.stacked == true) null else true)) }, verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(s.stacked == true, onCheckedChange = null)
                    Text("Stacked", fontSize = 13.sp, modifier = Modifier.padding(start = 6.dp))
                }
            }
        }
    }
}

/** A series' colour: the default for its place, Taskwarrior's basic colours by name, or any `#rrggbb`. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun ColorChoice(value: String?, position: Int, dark: Boolean, onChange: (String?) -> Unit) {
    val primary = MaterialTheme.colorScheme.primary
    var hex by remember(value) { mutableStateOf(if (value?.startsWith("#") == true) value else "") }
    Labeled("Colour") {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            @Composable
            fun Swatch(color: Color, selected: Boolean, label: String, onClick: () -> Unit) {
                Box(
                    Modifier.size(24.dp).background(color, RoundedCornerShape(4.dp))
                        .border(BorderStroke(if (selected) 2.dp else 1.dp, if (selected) primary else MaterialTheme.colorScheme.outline), RoundedCornerShape(4.dp))
                        .clickable(onClick = onClick),
                )
            }
            Swatch(defaultSeriesColor(position, dark), value == null, "Default colour (${defaultSeriesName(position)})") { onChange(null) }
            CHART_COLOR_NAMES.forEach { n -> Swatch(namedColor(n, dark) ?: Color.Gray, value?.trim()?.lowercase() == n, n) { onChange(n) } }
        }
        TextBox(
            "or any colour", hex,
            { v -> hex = v; if (v.isEmpty()) onChange(null) else if (hexColor(v) != null) onChange(v.lowercase()) },
            mono = true, placeholder = "#rrggbb", error = if (hex.isNotEmpty() && hexColor(hex) == null) "a colour is #rgb or #rrggbb" else null,
        )
    }
}
