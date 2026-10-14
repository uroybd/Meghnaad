package me.utsob.meghnaad.app

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

// What the engine answers, in Kotlin: mirrors web/src/lib/types.ts (itself a mirror of tc-core's run, cli and model).
// Only the kinds a screen draws so far have a class; the rest arrive as [Unsupported] with their JSON.

val engineJson = Json { ignoreUnknownKeys = true }

@Serializable
data class Note(val entry: Long, val text: String)

/** A colour style from the engine: palette indexes (0-15 basic and bright, 16-231 cube, 232-255 grays). */
@Serializable
data class Resolved(
    val bold: Boolean = false,
    val underline: Boolean = false,
    val inverse: Boolean = false,
    val fg: Int? = null,
    val bg: Int? = null,
)

@Serializable
data class Session(val start: Long, val end: Long? = null, val seconds: Long = 0)

@Serializable
data class Row(
    val uuid: String,
    /** pending | completed | deleted | recurring */
    val status: String,
    val description: String,
    val project: String? = null,
    val priority: String? = null,
    val tags: List<String> = emptyList(),
    val annotations: List<Note> = emptyList(),
    val entry: Long? = null,
    val modified: Long? = null,
    val start: Long? = null,
    val end: Long? = null,
    val due: Long? = null,
    val wait: Long? = null,
    val scheduled: Long? = null,
    val until: Long? = null,
    val depends: List<String> = emptyList(),
    val blocked: Boolean = false,
    val blocking: Boolean = false,
    val recur: String? = null,
    val parent: String? = null,
    val mask: String? = null,
    val imask: Int? = null,
    val extra: Map<String, String> = emptyMap(),
    val style: Resolved? = null,
    val urgency: Double = 0.0,
    /** Working-set number: specific to this client, and usually differs from a desktop's. */
    val id: Int? = null,
    @SerialName("virtual_tags") val virtualTags: List<String> = emptyList(),
    val waiting: Boolean = false,
    val orphans: List<String> = emptyList(),
    @SerialName("active_seconds") val activeSeconds: Long? = null,
    val sessions: List<Session> = emptyList(),
    @SerialName("pending_deps") val pendingDeps: Int = 0,
)

@Serializable
data class Column(
    val spec: String,
    val name: String,
    val format: String? = null,
    val label: String,
    /** id | string | description | project | priority | status | tags | date | number | duration | uuids | notes */
    val kind: String,
)

@Serializable
data class ReportResult(
    val report: String,
    val description: String? = null,
    val columns: List<Column> = emptyList(),
    val rows: List<Row> = emptyList(),
    val breaks: List<Boolean> = emptyList(),
    val matched: Int = 0,
    val sort: String? = null,
)

@Serializable
data class TableResult(
    val title: String? = null,
    val footer: List<String> = emptyList(),
    val highlight: List<Int> = emptyList(),
    val right: List<Int> = emptyList(),
    val headers: List<String> = emptyList(),
    val rows: List<List<String>> = emptyList(),
)

@Serializable
data class TextResult(val lines: List<String> = emptyList())

@Serializable
data class ChangedTask(val uuid: String, val id: Int? = null, val description: String)

@Serializable
data class ChangedResult(val message: String, val tasks: List<ChangedTask> = emptyList())

/** One question about one task. */
@Serializable
data class ConfirmItem(
    /** What to send back to say yes. */
    val key: String,
    val uuid: String,
    val id: Int? = null,
    val description: String,
    val question: String,
)

/** `plain` is one yes/no; `permission` has a question per task; `extras` are follow-ups that come once those are answered. */
@Serializable
data class ConfirmResult(val message: String, val ask: String, val items: List<ConfirmItem> = emptyList())

@Serializable
data class ErrorResult(val message: String)

/** `summary`: how far along each project is, with its sub-projects indented beneath it. */
@Serializable
data class SummaryRow(
    /** The full project name; empty for tasks without a project. */
    val project: String,
    /** `(none)`, or the last part of the name. */
    val label: String,
    /** How many levels down (`Home.Kitchen` is 1). */
    val depth: Int = 0,
    val remaining: Long = 0,
    val completed: Long = 0,
    @SerialName("avg_age") val avgAge: String = "",
    val complete: String = "",
    /** Filled cells of a 30-cell bar. */
    val bar: Int = 0,
)

@Serializable
data class SummaryResult(val rows: List<SummaryRow> = emptyList())

@Serializable
data class CalendarDay(
    val day: Int,
    val today: Boolean = false,
    val weekend: Boolean = false,
    val holiday: Boolean = false,
    val scheduled: Boolean = false,
    /** overdue | due-today | due */
    val due: String? = null,
)

@Serializable
data class CalendarWeek(val number: Int? = null, val days: List<CalendarDay?> = emptyList())

@Serializable
data class CalendarMonth(val year: Int, val month: Int, val name: String, val weeks: List<CalendarWeek> = emptyList())

@Serializable
data class Holiday(val date: Long, val name: String)

@Serializable
data class CalendarResult(
    val months: List<CalendarMonth> = emptyList(),
    /** Two-letter column headings, from the first day of the week. */
    val weekdays: List<String> = emptyList(),
    @SerialName("week_numbers") val weekNumbers: Boolean = false,
    val legend: Boolean = false,
    @SerialName("due_colours") val dueColours: Boolean = false,
    @SerialName("holiday_colours") val holidayColours: Boolean = false,
    val holidays: List<Holiday>? = null,
    val details: ReportResult? = null,
)

@Serializable
data class BurndownBar(
    /** Start of the period, in epoch seconds. */
    val epoch: Long,
    val major: String,
    val minor: String,
    val pending: Long = 0,
    val started: Long = 0,
    val done: Long = 0,
)

@Serializable
data class BurndownEnd(val epoch: Long, @SerialName("in_secs") val inSecs: Long = 0, val vague: String = "")

@Serializable
data class BurndownResult(
    val period: String = "daily",
    val title: String = "",
    /** Oldest first. */
    val bars: List<BurndownBar> = emptyList(),
    /** Tasks finished before the first bar; they are on every bar. */
    @SerialName("carryover_done") val carryoverDone: Long = 0,
    /** The y axis: 0, half, top. */
    @SerialName("y_labels") val yLabels: List<Long> = emptyList(),
    @SerialName("net_fix_rate") val netFixRate: Double? = null,
    val completion: BurndownEnd? = null,
    @SerialName("no_convergence") val noConvergence: Boolean = false,
)

/** `info`: one or more tasks in full (the detail view reads these). */
@Serializable
data class InfoResult(val tasks: List<Row> = emptyList())

sealed interface CliResult {
    data class Report(val value: ReportResult) : CliResult
    data class Table(val value: TableResult) : CliResult
    data class Text(val value: TextResult) : CliResult
    data class Changed(val value: ChangedResult) : CliResult
    data class Confirm(val value: ConfirmResult) : CliResult
    data class Error(val value: ErrorResult) : CliResult
    data class Info(val value: InfoResult) : CliResult
    data class Summary(val value: SummaryResult) : CliResult
    data class Calendar(val value: CalendarResult) : CliResult
    data class Burndown(val value: BurndownResult) : CliResult

    /** Anything a command answers with as plain JSON (`_names`, `_remind`, `_uuids`): the value as it is. */
    data class Json(val value: JsonElement) : CliResult

    /** A kind no screen draws yet (info, summary, calendar, burndown, styled, file, import, json). */
    data class Unsupported(val kind: String, val json: JsonObject) : CliResult
}

/** How the engine understood the command line. */
@Serializable
data class CommandInfo(val name: String, val report: Boolean = false, val filter: List<String> = emptyList())

/** A line a hook printed. */
@Serializable
data class HookLine(val kind: String, val text: String)

class CliResponse(
    val wrote: Boolean,
    val feedback: List<HookLine>,
    val result: CliResult,
    val command: CommandInfo?,
    /** Present when `config` changed the settings: the new ones, to save and give back to the engine. */
    val config: JsonElement?,
)

/** The engine refused the request (it answered `{ "error": … }`). */
class EngineError(message: String, val status: Int = 400) : Exception(message)

/** Read what the engine answered to a command. */
fun parseResponse(text: String): CliResponse {
    val root = engineJson.parseToJsonElement(text).jsonObject
    root["error"]?.let { throw EngineError(it.jsonPrimitive.content) }
    val result = root.getValue("result").jsonObject
    val kind = result.getValue("kind").jsonPrimitive.content
    val parsed: CliResult = when (kind) {
        "report" -> CliResult.Report(engineJson.decodeFromJsonElement(result))
        "table" -> CliResult.Table(engineJson.decodeFromJsonElement(result))
        "text" -> CliResult.Text(engineJson.decodeFromJsonElement(result))
        "changed" -> CliResult.Changed(engineJson.decodeFromJsonElement(result))
        "confirm" -> CliResult.Confirm(engineJson.decodeFromJsonElement(result))
        "error" -> CliResult.Error(engineJson.decodeFromJsonElement(result))
        "info" -> CliResult.Info(engineJson.decodeFromJsonElement(result))
        "summary" -> CliResult.Summary(engineJson.decodeFromJsonElement(result))
        "calendar" -> CliResult.Calendar(engineJson.decodeFromJsonElement(result))
        "burndown" -> CliResult.Burndown(engineJson.decodeFromJsonElement(result))
        "json" -> CliResult.Json(result["value"] ?: kotlinx.serialization.json.JsonNull)
        else -> CliResult.Unsupported(kind, result)
    }
    return CliResponse(
        wrote = root["wrote"]?.jsonPrimitive?.content == "true",
        feedback = root["feedback"]?.let { engineJson.decodeFromJsonElement<List<HookLine>>(it) } ?: emptyList(),
        result = parsed,
        command = root["command"]?.takeIf { it !is kotlinx.serialization.json.JsonNull }
            ?.let { engineJson.decodeFromJsonElement<CommandInfo>(it) },
        config = root["config"],
    )
}

/** One change to a task, from its operation log (`_history`): what `task info` words as a sentence. */
@Serializable
data class HistoryChange(
    /** set | changed | deleted | note_added | note_changed | note_deleted | tag_added | tag_deleted | dep_added | dep_deleted */
    val kind: String,
    /** The property; for the tag and dependency kinds, the tag or the uuid. */
    val prop: String,
    val old: String? = null,
    val value: String? = null,
    /** The values are epoch seconds to show in the date format. */
    val date: Boolean = false,
    /** For a deleted `start`: how long the task ran, already formatted. */
    val duration: String? = null,
)

/** The changes made at one moment. */
@Serializable
data class HistoryEntry(val at: Long, val changes: List<HistoryChange> = emptyList())

/** The report names the settings define, for choosing one. */
@Serializable
data class ReportMeta(
    val name: String,
    val description: String? = null,
    val columns: List<Column> = emptyList(),
    val filter: String? = null,
    val sort: String? = null,
)

@Serializable
data class UdaDef(
    val name: String,
    /** string | numeric | date | duration | uuid */
    val type: String = "string",
    val label: String? = null,
    val values: List<String> = emptyList(),
    val default: String? = null,
    val indicator: String? = null,
)

@Serializable
data class ContextDef(
    val name: String,
    val read: String? = null,
    val write: String? = null,
    val rc: Map<String, String> = emptyMap(),
)

/** What a report's definition carries that a screen reads: its own `dateformat`. */
@Serializable
data class ReportCfg(val dateformat: String? = null)

/** The `config` of a [ConfigResponse]: what the taskrc settings come to. */
@Serializable
data class ConfigBody(
    val udas: Map<String, UdaDef> = emptyMap(),
    val reports: Map<String, ReportCfg> = emptyMap(),
    val contexts: Map<String, ContextDef> = emptyMap(),
    @SerialName("active_context") val activeContext: String? = null,
    val urgency: Map<String, Double> = emptyMap(),
    val settings: Map<String, String> = emptyMap(),
)

/** The annotation texts `journal.time` writes on start and stop. */
@Serializable
data class Journal(val start: String, val stop: String)

@Serializable
data class ConfigResponse(
    val config: ConfigBody = ConfigBody(),
    val reports: List<ReportMeta> = emptyList(),
    /** Null when journalling is off. */
    val journal: Journal? = null,
    /** There are earlier saved settings to restore. */
    @SerialName("has_previous") val hasPrevious: Boolean = false,
    /** Whether tasks are coloured (`color`). */
    val color: Boolean = false,
    /** Every colour in force, by name without `color.` (`calendar.today`): palette indexes, for the charts. */
    val colors: Map<String, Resolved> = emptyMap(),
    @SerialName("urgency_defaults") val urgencyDefaults: Map<String, Double> = emptyMap(),
    @SerialName("urgency_inherit") val urgencyInherit: Boolean = false,
    /** Saved settings exist but could not be read (they are left untouched). */
    @SerialName("config_error") val configError: String? = null,
)
