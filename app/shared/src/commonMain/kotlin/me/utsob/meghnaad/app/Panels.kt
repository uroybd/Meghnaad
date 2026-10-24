package me.utsob.meghnaad.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.widthIn
import androidx.compose.material3.HorizontalDivider
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay

/** A colour the settings give by name (`calendar.today`, `burndown.done`), as the style it resolves to. */
private fun Store.styleOf(name: String): Resolved? = config?.colors?.get(name)

/** A page in the style of the Tasks page: a filter box and, under it, what the same command prints in the console, drawn by the same component (PanelPage.svelte). */
@Composable
fun PanelPage(
    store: Store,
    which: Panel,
    filter: String,
    onFilter: (String) -> Unit,
    command: String,
    placeholder: String,
    controls: @Composable (Entry?) -> Unit = {},
) {
    val entry = store.panels[which]
    // Ask again when the filter or the command changes, and when a task changes anywhere. Only typing waits (a moment); opening
    // the page and tapping a button do not.
    var lastFilter by remember { mutableStateOf<String?>(null) }
    val choice = store.contextChoice(Scope.panel(which))
    LaunchedEffect(filter, command, store.rev, choice) {
        val typing = lastFilter != null && filter != lastFilter
        if (typing) delay(250)
        lastFilter = filter
        store.runPanel(which, CliInput(args = splitWords(filter) + command))
    }
    LazyColumn(Modifier.fillMaxSize().padding(horizontal = 12.dp), contentPadding = androidx.compose.foundation.layout.PaddingValues(bottom = 96.dp)) {
        item {
            Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                controls(entry)
                FilterField(store, filter, onFilter, placeholder = placeholder)
                ContextPicker(store, Scope.panel(which))
                FilterChips(filter, onFilter)
            }
        }
        if (entry != null) item { ResultBlock(store, entry, Modifier.padding(top = 10.dp)) }
    }
}

@Composable
fun SummaryPage(store: Store) = PanelPage(store, Panel.Summary, store.summaryFilter, { store.summaryFilter = it }, "summary", "Filters: project:Home +work status:pending")

@Composable
fun CalendarPage(store: Store) {
    PanelPage(store, Panel.Calendar, store.calendarFilter, { store.calendarFilter = it }, "calendar", "Months and filters: march 2027 · y · due · project:Work") { entry ->
        val r = (entry?.result as? CliResult.Calendar)?.value
        val at = r?.months?.firstOrNull()?.let { YearMonth(it.year, it.month) }
        val count = r?.months?.size ?: 0
        fun go(months: List<String>) {
            store.calendarFilter = withMonths(store.calendarFilter, months)
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(2.dp), itemVerticalAlignment = Alignment.CenterVertically) {
            IconAction("chevron-left", "Earlier months", { at?.let { go(stepWords(it, count, -1)) } }, enabled = at != null)
            OutlinedButton(onClick = { go(emptyList()) }) { Text("Today") }
            IconAction("chevron-right", "Later months", { at?.let { go(stepWords(it, count, 1)) } }, enabled = at != null)
            OutlinedButton(onClick = { go(listOf("y")) }) { Text("Year") }
            OutlinedButton(onClick = { go(listOf("due")) }) { Text("First due") }
        }
    }
}

@Composable
fun BurndownPage(store: Store) {
    val periods = listOf("daily", "weekly", "monthly", "annual")
    PanelPage(store, Panel.Burndown, store.burndownFilter, { store.burndownFilter = it }, "burndown.${store.burndownPeriod}", "Filters: project:Home +work") {
        Choice(store.burndownPeriod.replaceFirstChar { it.uppercase() }, periods.map { it.replaceFirstChar { c -> c.uppercase() } }) { store.burndownPeriod = periods[it] }
    }
}

private const val CELLS = 30 // the bar's width in Taskwarrior, in cells

/** `summary`: how far along each project is, with its sub-projects indented beneath it (SummaryView.svelte). */
@Composable
fun SummaryView(store: Store, result: SummaryResult) {
    val colors = MaterialTheme.colorScheme
    val track = store.styleOf("summary.background")?.tintSource(extras.dark)?.copy(alpha = 0.3f) ?: colors.surfaceVariant
    val fill = store.styleOf("summary.bar")?.tintSource(extras.dark) ?: extras.ok
    val show = { row: SummaryRow -> store.focusReport("list", listOf(if (row.project.isNotEmpty()) "project:${row.project}" else "project:")) }
    BoxWithConstraints(Modifier.fillMaxWidth()) {
    if (maxWidth > TABLE_FROM) {
        // The web's table: the project, three numbers and the bar with its 0% and 100% at the ends.
        Column(Modifier.widthIn(max = 760.dp)) {
            Row(Modifier.fillMaxWidth().padding(vertical = 6.dp), verticalAlignment = Alignment.CenterVertically) {
                val head = @Composable { text: String, w: Float, align: androidx.compose.ui.text.style.TextAlign -> Text(text, Modifier.weight(w), color = colors.onSurfaceVariant, fontSize = 13.sp, fontWeight = FontWeight.Medium, textAlign = align) }
                head("Project", 3f, androidx.compose.ui.text.style.TextAlign.Start)
                head("Remaining", 1.2f, androidx.compose.ui.text.style.TextAlign.End)
                head("Avg age", 1.2f, androidx.compose.ui.text.style.TextAlign.End)
                head("Complete", 1.2f, androidx.compose.ui.text.style.TextAlign.End)
                Row(Modifier.weight(3f).padding(start = 16.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text("0%", color = colors.onSurfaceVariant, fontSize = 13.sp); Text("100%", color = colors.onSurfaceVariant, fontSize = 13.sp)
                }
            }
            HorizontalDivider(color = colors.outline)
            for (row in result.rows) {
                Row(Modifier.fillMaxWidth().padding(vertical = 5.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text(row.label, Modifier.weight(3f).padding(start = (row.depth * 20).dp).clickable { show(row) }, color = colors.primary, fontSize = 14.sp)
                    Text(row.remaining.toString(), Modifier.weight(1.2f), textAlign = androidx.compose.ui.text.style.TextAlign.End, fontSize = 14.sp)
                    Text(row.avgAge, Modifier.weight(1.2f), textAlign = androidx.compose.ui.text.style.TextAlign.End, fontSize = 14.sp)
                    Text(row.complete, Modifier.weight(1.2f), textAlign = androidx.compose.ui.text.style.TextAlign.End, fontSize = 14.sp)
                    Box(Modifier.weight(3f).padding(start = 16.dp).height(8.dp).clip(CircleShape).background(track)) {
                        Box(Modifier.fillMaxWidth(row.bar.coerceIn(0, CELLS) / CELLS.toFloat()).height(8.dp).background(fill))
                    }
                }
            }
            Dim("${result.rows.size} ${if (result.rows.size == 1) "project" else "projects"}", Modifier.padding(top = 6.dp))
        }
    } else
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        for (row in result.rows) {
            Column(Modifier.fillMaxWidth().padding(start = (row.depth * 16).dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    // A project's name shows that project's tasks (and its sub-projects') in the built-in `list` report, which is always there.
                    Text(
                        row.label,
                        Modifier.weight(1f).clickable { show(row) },
                        color = colors.primary, fontSize = 15.sp, fontWeight = FontWeight.Medium,
                    )
                    Text(row.complete, fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                }
                Dim("${row.remaining} remaining · avg age ${row.avgAge}", size = 12)
                Box(Modifier.fillMaxWidth().padding(top = 4.dp).height(8.dp).clip(CircleShape).background(track)) {
                    Box(Modifier.fillMaxWidth(row.bar.coerceIn(0, CELLS) / CELLS.toFloat()).height(8.dp).background(fill))
                }
            }
        }
        Dim("${result.rows.size} ${if (result.rows.size == 1) "project" else "projects"}")
    }
    }
}

/** The holiday a tap named: the day it is on, how to say it, and whether the day has tasks too (then the next tap opens them). */
private class HolidayPick(val at: String, val label: String, val names: List<String>, val tasks: Boolean)

/** `calendar`: months laid out the way `task calendar` prints them, drawn rather than typed (CalendarView.svelte). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun CalendarView(store: Store, entry: Entry?, result: CalendarResult) {
    val colors = MaterialTheme.colorScheme
    val dark = extras.dark
    val settings = store.config?.config?.settings
    val dateFmt = formatFor(FormatKind.Report, settings)

    // A tap on a day with something due shows those tasks in the Tasks view, always in the built-in `list` report (the report in
    // focus might filter or order them differently, or not exist any more).
    // A tap on a holiday says which holiday it is. On a day that also has something due that is the first tap, and the tasks open on
    // the next; a holiday with nothing due is shown, and hidden by the next.
    var picked by remember { mutableStateOf<HolidayPick?>(null) }

    fun show(m: CalendarMonth, d: CalendarDay) {
        val tasks = d.due != null || d.scheduled
        val day = "${m.year}-${m.month.toString().padStart(2, '0')}-${d.day.toString().padStart(2, '0')}"
        if (d.holidays.isNotEmpty() && picked?.at != day) {
            picked = HolidayPick(day, "${m.name} ${d.day}", d.holidays, tasks)
            return
        }
        picked = null
        if (tasks) store.focusReport("list", listOf(if (d.due != null) "due:$day" else "scheduled:$day"))
    }

    fun describe(m: CalendarMonth, d: CalendarDay): String {
        val bits = buildList {
            if (d.today) add("today")
            if (d.holiday) add(if (d.holidays.isEmpty()) "holiday" else d.holidays.joinToString(", "))
            if (d.due == "overdue") add("overdue")
            if (d.due == "due-today") add("due later today")
            if (d.due == "due") add("something is due")
            if (d.scheduled) add("scheduled")
        }
        return "${m.name} ${d.day}" + if (bits.isEmpty()) "" else ": " + bits.joinToString(", ")
    }

    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        BoxWithConstraints(Modifier.fillMaxWidth()) {
        // The web's grid: as many 17.5rem columns as fit, the months side by side.
        val cols = maxOf(1, ((maxWidth.value + 24f) / (280f + 24f)).toInt())
        Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(16.dp)) {
        for (group in result.months.chunked(cols)) Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(24.dp)) {
        for (m in group) {
            Column(Modifier.weight(1f)) {
                Text("${m.name} ${m.year}", fontWeight = FontWeight.SemiBold, fontSize = 15.sp, modifier = Modifier.padding(bottom = 6.dp))
                Row(Modifier.fillMaxWidth()) {
                    if (result.weekNumbers) Box(Modifier.weight(0.7f))
                    for (w in result.weekdays) Text(w, Modifier.weight(1f), fontSize = 12.sp, color = colors.onSurfaceVariant, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
                }
                for (week in m.weeks) {
                    Row(Modifier.fillMaxWidth().padding(vertical = 1.dp), verticalAlignment = Alignment.CenterVertically) {
                        if (result.weekNumbers) Text(week.number?.toString() ?: "", Modifier.weight(0.7f), fontSize = 11.sp, color = colors.onSurfaceVariant)
                        for (d in week.days) {
                            if (d == null) {
                                Box(Modifier.weight(1f))
                                continue
                            }
                            // Later wins when several apply: today, overdue, due today, due, scheduled, holiday, weekend.
                            val name = when {
                                d.today -> "calendar.today"
                                d.due == "overdue" -> "calendar.overdue"
                                d.due == "due-today" -> "calendar.due.today"
                                d.due == "due" -> "calendar.due"
                                d.scheduled -> "calendar.scheduled"
                                d.holiday -> "calendar.holiday"
                                d.weekend -> "calendar.weekend"
                                else -> null
                            }
                            val style = name?.let { store.styleOf(it) }
                            val wash = style.tint(dark, colors.surface) ?: when {
                                d.today -> colors.primary.copy(alpha = 0.2f)
                                d.due == "overdue" -> colors.error.copy(alpha = 0.16f)
                                else -> null
                            }
                            val strong = d.due != null || d.today || style?.bold == true
                            val clickable = d.due != null || d.scheduled || d.holidays.isNotEmpty()
                            Box(
                                Modifier.weight(1f).aspectRatio(1f).padding(2.dp).clip(RoundedCornerShape(8.dp)).background(wash ?: Color.Transparent)
                                    .semantics { contentDescription = describe(m, d) }
                                    .let { if (clickable) it.clickable { show(m, d) } else it },
                                contentAlignment = Alignment.Center,
                            ) {
                                Text(
                                    d.day.toString(), fontSize = 14.sp,
                                    fontWeight = if (strong) FontWeight.SemiBold else FontWeight.Normal,
                                    color = if (d.weekend && name == "calendar.weekend") colors.onSurfaceVariant else colors.onSurface,
                                )
                            }
                        }
                    }
                }
            }
        }
        repeat(cols - group.size) { Box(Modifier.weight(1f)) }
        }
        }
        }
        picked?.let { p ->
            Text("${p.label}: ${p.names.joinToString(", ")}" + if (p.tasks) " · tap the day again for what is due" else "", fontSize = 13.5.sp)
        }
        if (result.legend) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(4.dp), itemVerticalAlignment = Alignment.CenterVertically) {
                @Composable
                fun Key(label: String, name: String, fallback: Color = Color.Transparent) {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        Box(Modifier.width(14.dp).height(14.dp).clip(RoundedCornerShape(4.dp)).background(store.styleOf(name).tint(dark, colors.surface) ?: fallback))
                        Dim(label, size = 12)
                    }
                }
                Key("today", "calendar.today", colors.primary.copy(alpha = 0.2f))
                Key("weekend", "calendar.weekend", colors.surfaceVariant)
                if (result.dueColours) {
                    Key("due", "calendar.due"); Key("due today", "calendar.due.today"); Key("overdue", "calendar.overdue", colors.error.copy(alpha = 0.16f)); Key("scheduled", "calendar.scheduled")
                }
                if (result.holidayColours) Key("holiday", "calendar.holiday")
                if (result.weekNumbers) Dim("week number", size = 12)
            }
        }
        result.holidays?.let { holidays ->
            Text("Holidays", fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
            if (holidays.isEmpty()) Dim("No holidays in these months.") else for (h in holidays) {
                Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(formatMoment(h.date, store.offsetAt(h.date), dateFmt), fontSize = 13.sp, modifier = Modifier.width(110.dp))
                    Text(h.name, fontSize = 13.sp)
                }
            }
        }
        result.details?.let { details ->
            Text("Due in these months", fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
            if (details.rows.isEmpty()) Dim("Nothing is due in these months.") else if (entry != null) ReportSection(store, entry, details)
        }
    }
}

/** `burndown.*`: pending, started and done over time, as stacked bars (BurndownView.svelte). */
@Composable
fun BurndownView(store: Store, result: BurndownResult) {
    val colors = MaterialTheme.colorScheme
    val dark = extras.dark
    val dateFmt = formatFor(FormatKind.Report, store.config?.config?.settings)
    val measurer = rememberTextMeasurer()
    val pending = store.styleOf("burndown.pending")?.tintSource(dark) ?: colors.error.copy(alpha = 0.8f)
    val started = store.styleOf("burndown.started")?.tintSource(dark) ?: extras.warn.copy(alpha = 0.85f)
    val done = store.styleOf("burndown.done")?.tintSource(dark) ?: extras.ok.copy(alpha = 0.75f)
    val dim = colors.onSurfaceVariant
    val line = colors.outlineVariant

    // The drawing, in dp: a plot area with the y axis on its left and two lines of labels below.
    val step = 30f
    val bar = 22f
    val plotH = 240f
    val left = 46f
    val padTop = 12f
    val labelsH = 46f
    val top = maxOf(result.yLabels.getOrElse(2) { 1 }, 1L).toFloat()
    val width = left + result.bars.size * step + 8f
    val height = padTop + plotH + labelsH
    fun doneOf(b: BurndownBar) = b.done + result.carryoverDone
    fun h(n: Long) = maxOf(n, 0L) / top * plotH
    val base = padTop + plotH
    val empty = result.bars.all { it.pending + it.started + doneOf(it) == 0L }
    val majors = result.bars.mapIndexed { i, b -> if (i == 0 || b.major != result.bars[i - 1].major) b.major else "" }

    Column(Modifier.fillMaxWidth()) {
        Text(result.title, fontWeight = FontWeight.SemiBold, fontSize = 15.sp, modifier = Modifier.padding(bottom = 6.dp))
        if (empty) {
            Dim("No matches.")
            return@Column
        }
        // The newest bars are on the right: start there, as a phone's screen shows only the end of a long chart.
        val scroll = rememberScrollState()
        LaunchedEffect(scroll.maxValue) { scroll.scrollTo(scroll.maxValue) }
        val last = result.bars.last()
        val summary = "${result.title}: ${result.bars.size} bars. Latest: ${last.major} ${last.minor}: ${last.pending} pending, ${last.started} started, ${doneOf(last)} done."
        // With room to spare the drawing grows to fill it, up to 1100, as the web's does; otherwise it keeps its size and scrolls.
        BoxWithConstraints(Modifier.fillMaxWidth()) {
        val grow = if (maxWidth.value > width) minOf(maxWidth.value, 1100f) / width else 1f
        Box(Modifier.horizontalScroll(scroll)) {
            Canvas(Modifier.width((width * grow).dp).height((height * grow).dp).semantics { contentDescription = summary }) {
              scale(grow, Offset.Zero) {
                val axis = TextStyle(fontSize = 11.sp, color = dim)
                for (v in result.yLabels) {
                    val y = (base - h(v)).dp.toPx()
                    drawLine(line, Offset(left.dp.toPx(), y), Offset((width - 4).dp.toPx(), y), 1f)
                    val t = measurer.measure(v.toString(), axis)
                    drawText(t, topLeft = Offset((left - 8).dp.toPx() - t.size.width, y - t.size.height / 2f))
                }
                drawLine(dim, Offset(left.dp.toPx(), base.dp.toPx()), Offset((width - 4).dp.toPx(), base.dp.toPx()), 1.5f)
                result.bars.forEachIndexed { i, b ->
                    val x = (left + i * step + (step - bar) / 2f)
                    val p = h(b.pending); val s = h(b.started); val d = h(doneOf(b))
                    drawRect(pending, Offset(x.dp.toPx(), (base - p).dp.toPx()), Size(bar.dp.toPx(), p.dp.toPx()))
                    drawRect(started, Offset(x.dp.toPx(), (base - p - s).dp.toPx()), Size(bar.dp.toPx(), s.dp.toPx()))
                    drawRect(done, Offset(x.dp.toPx(), (base - p - s - d).dp.toPx()), Size(bar.dp.toPx(), d.dp.toPx()))
                    val minor = measurer.measure(b.minor, TextStyle(fontSize = 10.sp, color = dim))
                    drawText(minor, topLeft = Offset((x + bar / 2f).dp.toPx() - minor.size.width / 2f, (base + 4).dp.toPx()))
                    if (majors[i].isNotEmpty()) drawText(measurer.measure(majors[i], TextStyle(fontSize = 10.sp, color = colors.onSurface)), topLeft = Offset(x.dp.toPx(), (base + 22).dp.toPx()))
                }
              }
            }
        }
        }
        FlowRow(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(12.dp), itemVerticalAlignment = Alignment.CenterVertically) {
            for ((label, c) in listOf("Done" to done, "Started" to started, "Pending" to pending)) {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    Box(Modifier.width(12.dp).height(12.dp).clip(RoundedCornerShape(3.dp)).background(c)); Dim(label, size = 12)
                }
            }
        }
        Row(Modifier.padding(top = 8.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dim("Net fix rate"); Text(result.netFixRate?.let { "${fixed(it, 1)}/d" } ?: "-", fontSize = 13.sp)
        }
        val end = result.completion
        if (end != null) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Dim("Estimated completion"); Text(formatMoment(end.epoch, store.offsetAt(end.epoch), dateFmt), fontSize = 13.sp); Dim("(${end.vague})")
            }
        } else if (result.noConvergence) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) { Dim("Estimated completion"); Text("No convergence", fontSize = 13.sp) }
        }
    }
}
