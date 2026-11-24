package me.utsob.meghnaad.app

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlin.test.Test
import kotlinx.serialization.json.put
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

// Ported from web/src/lib/kanban.test.ts.
class KanbanTest {
    private fun config(vararg udas: UdaDef) = ConfigResponse(config = ConfigBody(udas = udas.associateBy { it.name }))

    private fun row(uuid: String) = Row(uuid = uuid, status = "pending", description = uuid)

    private val u = PROGRESS_UDA

    @Test
    fun definesTheUdaWhenItIsMissing() {
        assertEquals(Part.Missing, udaStatus(null, u).part)
        assertEquals(
            listOf("uda.progressState.type" to "string", "uda.progressState.label" to "Progress", "uda.progressState.values" to "inprogress,review"),
            kanbanSetupLines(config(), u),
        )
        assertEquals("uda.stage.type" to "string", kanbanSetupLines(config(), "stage")[0])
    }

    @Test
    fun leavesAloneAUdaThatAlreadyFits() {
        for (values in listOf(emptyList(), listOf("inprogress", "review"), listOf("blocked", "inprogress", "review"))) {
            val c = config(UdaDef(PROGRESS_UDA, "string", "P", values))
            assertEquals(Part.Ok, udaStatus(c, u).part)
            assertEquals(emptyList(), kanbanSetupLines(c, u))
        }
    }

    @Test
    fun addsOurValuesToTheOnesThereAndFixesTheType() {
        val c = config(UdaDef(PROGRESS_UDA, "string", "Stage", listOf("blocked")))
        assertEquals(Part.Different, udaStatus(c, u).part)
        assertEquals(listOf("uda.progressState.values" to "blocked,inprogress,review"), kanbanSetupLines(c, u))
        val d = config(UdaDef(PROGRESS_UDA, "numeric", "Stage"))
        assertEquals(listOf("uda.progressState.type" to "string", "uda.progressState.values" to "inprogress,review"), kanbanSetupLines(d, u))
    }

    @Test
    fun takesAUdaNameTaskwarriorWould() {
        assertEquals(null, udaNameProblem("progressState"))
        assertEquals(null, udaNameProblem("stage_2"))
        assertEquals("needs a UDA name", udaNameProblem(""))
        assertTrue("starting with a letter" in udaNameProblem("2stage")!!)
        assertTrue("letters, digits and _" in udaNameProblem("my stage")!!)
        assertTrue("Taskwarrior's own" in udaNameProblem("project")!!)
    }

    @Test
    fun makesTheAutomaticColumnsAndReviewClearsReviewedWhenTaskshIsSetUp() {
        val cols = automaticColumns(false)
        assertEquals(listOf("To do", "In progress", "Review", "Done"), cols.map { it.name })
        assertEquals("modify status:pending progressState:", cols[0].move)
        assertEquals("status:completed", cols[3].filter)
        assertEquals("done", cols[3].move)
        assertEquals("modify status:pending progressState:review", cols[2].move)
        assertEquals("modify status:pending progressState:review reviewed:", automaticColumns(true)[2].move)
        assertEquals("status:pending stage:inprogress", automaticColumns(false, "stage")[1].filter)
        assertTrue(hasReview(config(UdaDef("reviewed", "date"))))
        assertFalse(hasReview(config()))
    }

    @Test
    fun movesWithTaskCommandsAddedUnlessOneOfThatNameStandsForSomethingElse() {
        val moves = moveCommands(true)
        assertEquals(
            listOf(
                TaskCommand("pending", "modify status:pending progressState:", "circle-dashed"),
                TaskCommand("inprogress", "modify status:pending progressState:inprogress", "loader-circle"),
                TaskCommand("review", "modify status:pending progressState:review reviewed:", "eye"),
            ),
            moves,
        )
        assertEquals(listOf("pending", "inprogress", "review", "done"), automaticColumns(true, PROGRESS_UDA, moves).map { it.move })
        // Someone's own review (same command, another icon: still usable) and pending (something else: left alone).
        val mine = listOf(TaskCommand("pending", "modify +later"), TaskCommand("review", "modify status:pending progressState:review reviewed:", "x"))
        val usable = usableMoves(mine, true)
        assertEquals(listOf("inprogress", "review"), usable.map { it.name })
        assertEquals(
            listOf("modify status:pending progressState:", "inprogress", "review", "done"),
            automaticColumns(true, PROGRESS_UDA, usable).map { it.move },
        )
        assertEquals(listOf("pending", "review", "inprogress"), withMoveCommands(mine, true).map { it.name })
    }

    @Test
    fun turnsKanbanOnWithItsSettingsOnTheUdaAndAFirstKanban() {
        val s = automaticSettings(KanbanSettings(), false, "stage")
        assertEquals(true, s.enabled)
        assertEquals("automatic", s.setup)
        assertEquals("stage", s.uda)
        assertEquals("all", s.report)
        assertEquals("( +PENDING or +COMPLETED )", s.filter)
        assertEquals("today-14d", s.completedSince)
        assertEquals(listOf(KanbanEntry("kanban", "Kanban")), s.kanbans)
        assertFalse(s.columns.any { Regex("\\bstart\\b").containsMatchIn(it.move) })
        val mine = KanbanEntry("mine", "Mine", report = "list")
        assertEquals(listOf(mine), automaticSettings(KanbanSettings(kanbans = listOf(mine)), false, u).kanbans)
    }

    @Test
    fun canDrawThemOnAKanbanReportOfTheirOwnAddedUnlessThereIsOne() {
        val s = automaticSettings(KanbanSettings(), false, u, ownReport = true)
        assertEquals("_kanban", s.report)
        assertEquals("", s.filter)
        assertEquals("today-14d", s.completedSince)
        val lines = kanbanReportLines(config())
        assertEquals(listOf("description", "columns", "labels", "filter", "sort").map { "report._kanban.$it" }, lines.map { it.first })
        assertEquals("( +PENDING or +COMPLETED )", lines.toMap()["report._kanban.filter"])
        assertEquals(emptyList(), kanbanReportLines(ConfigResponse(config = ConfigBody(reports = mapOf("_kanban" to ReportCfg())))))
    }

    @Test
    fun choosesACardsIconByTheFieldsNameAndNoneForAUda() {
        assertEquals("clock", fieldIcon("due"))
        assertEquals("flag", fieldIcon("priority"))
        assertEquals(null, fieldIcon("progressState"))
    }

    @Test
    fun showsAFreeTextUdaAsAParagraphAndAnyOtherUdaAsAPill() {
        val udas = mapOf(
            "notes" to UdaDef("notes", "string"),
            "size" to UdaDef("size", "string", values = listOf("S", "M", "L")),
            "estimate" to UdaDef("estimate", "numeric"),
            "review" to UdaDef("review", "date"),
        )
        assertEquals(CardShape.Text, cardShape("notes", udas, "a long note, past twenty"))
        // A few words on one line, plain, is a pill too; several lines or Markdown (boxes to tick) a paragraph.
        assertEquals(CardShape.Pill, cardShape("notes", udas, "call back"))
        assertEquals(CardShape.Text, cardShape("notes", udas, "one\ntwo"))
        assertEquals(CardShape.Text, cardShape("notes", udas, "- [ ] call", markdown = true))
        assertEquals(CardShape.Pill, cardShape("size", udas))
        assertEquals(CardShape.Pill, cardShape("estimate", udas))
        assertEquals(CardShape.Pill, cardShape("review", udas))
        assertEquals(CardShape.Field, cardShape("due", udas))
    }

    @Test
    fun makesALaneOfEachGroupHeadingInTheReportsOrder() {
        assertEquals(Lanes(), lanesOf(listOf(row("a")), emptyList()))
        val l = lanesOf(listOf("a", "b", "c", "d", "e").map(::row), listOf("Project: Home", null, "Project: Work", "Project: Home", "Project: (none)"))
        assertEquals(listOf("Project: Home", "Project: Work", "Project: (none)"), l.lanes)
        assertEquals(mapOf("a" to 0, "b" to 0, "c" to 1, "d" to 0, "e" to 2), l.laneOf)
    }

    @Test
    fun bracketsEachFilterThenNamesTheReport() {
        val view = KanbanView("all", "( +PENDING or +COMPLETED )", "today-14d")
        assertEquals(
            listOf(
                "(", "(", "+PENDING", "or", "+COMPLETED", ")", ")",
                "(", "-COMPLETED", "or", "end.after:today-14d", ")",
                "(", "status:pending", "progressState:", ")", "all",
            ),
            columnArgs(view, KanbanColumn("To do", "status:pending progressState:", "")),
        )
        assertEquals(listOf("next"), columnArgs(KanbanView("next", "", ""), null))
        assertEquals(listOf("(", "project:Home", ")", "list"), columnArgs(KanbanView("list", "project:Home", ""), null))
    }

    @Test
    fun putsATaskInTheFirstColumnThatHasIt() {
        val placed = placeTasks(listOf(listOf(row("a"), row("b")), listOf(row("b"), row("c")), emptyList()), listOf(row("a"), row("b"), row("c"), row("d")))
        assertEquals(listOf(listOf("a", "b"), listOf("c"), emptyList()), placed.columns.map { c -> c.map { it.uuid } })
        assertEquals(listOf("d"), placed.other.map { it.uuid })
        assertEquals(1, columnOf(placed.columns, "c"))
        assertEquals(-1, columnOf(placed.columns, "d"))
    }

    private val uuid = "1a2b3c4d-0000-4000-8000-000000000000"

    @Test
    fun runsTheMoveOnThatTaskTheUuidInFrontOfEachStatement() {
        assertEquals(listOf(CommandStep(listOf(ONLY_THIS_TASK, uuid, "done"), null)), moveSteps("done", uuid, emptyList()))
        assertEquals(
            listOf(CommandStep(listOf(ONLY_THIS_TASK, uuid, "modify", "progressState:review"), null), CommandStep(listOf(ONLY_THIS_TASK, uuid, "annotate", "in review"), Joiner.Then)),
            moveSteps("modify progressState:review ; annotate \"in review\"", uuid, emptyList()),
        )
    }

    @Test
    fun writesOutATaskCommandKeepingTheTaskInFront() {
        assertEquals(
            listOf(CommandStep(listOf(ONLY_THIS_TASK, uuid, "modify", "+shipped"), null), CommandStep(listOf(ONLY_THIS_TASK, uuid, "done"), Joiner.And)),
            moveSteps("ship", uuid, listOf(TaskCommand("ship", "modify +shipped && done"))),
        )
    }

    private val doing = listOf(KanbanColumn("Doing", "+doing", "modify +doing"))
    private val file = """{"enabled":true,"setup":"manual","uda":"stage","report":"all","filter":"","completedSince":"today-14d",
        "columns":[{"name":"Doing","filter":"+doing","move":"modify +doing"}]"""
    private val settings = KanbanSettings(true, "manual", "stage", "all", "", "today-14d", doing)

    @Test
    fun drawsAKanbanWithItsOwnSettingsElseKanbans() {
        val work = KanbanEntry("work", "Work", filter = "project:Work")
        assertEquals(Kanban("work", "Work", "all", "project:Work", "today-14d", doing), resolveKanban(settings, work))
        val own = KanbanEntry("mine", "Mine", "list", completedSince = "", columns = emptyList())
        assertEquals(Kanban("mine", "Mine", "list", "", "", emptyList()), resolveKanban(settings, own))
    }

    @Test
    fun readsSettingsLeavingOutTheKanbansWithAProblem() {
        val w = """{"id":"work","name":"Work"}"""
        val text = """{"kanban":$file,"kanbans":[$w,
            {"id":"Bad Slug","name":"Work"}, $w,
            {"id":"two","name":"Two","columns":[{"name":"","filter":"x","move":""}]},
            {"id":"three","name":"Three","columns":[{"name":"A","filter":"x","move":"done &&"}]},
            {"id":"four","name":"Four","report":""}, {"id":"five","name":"Five","filter":3}, "nope"]}}"""
        val parsed = parseSettings(text)
        assertEquals(settings.copy(kanbans = listOf(KanbanEntry("work", "Work"))), parsed.kanban)
        assertEquals(7, parsed.kanbanErrors.size, parsed.kanbanErrors.toString())
        assertTrue("there is already a kanban at /kanban/work" in parsed.kanbanErrors[1])
        assertTrue("column 1: a column needs a name" in parsed.kanbanErrors[2])
        assertTrue("needs a report, its own or Kanban's" in parsed.kanbanErrors[4])
        assertEquals("kanban.kanbans[6] (five): filter must be text", parsed.kanbanErrors[5])
    }

    @Test
    fun saysWhatIsWrongWithTheUdaAndKanbansColumns() {
        val (k, errors) = parseKanban(Json.parseToJsonElement("""{"uda":"project","columns":[{"name":"A","filter":"x"},{"name":"B","filter":""}]}"""))
        assertEquals(PROGRESS_UDA, k.uda)
        assertEquals(emptyList(), k.columns)
        assertEquals(listOf("kanban.uda: project is one of Taskwarrior's own attributes", "kanban.columns: column 2 (B): a column needs a filter"), errors)
    }

    @Test
    fun savesKeepingWhatThisAppDoesNotKnow() {
        val text = """{"taskCommands":[{"name":"x","command":"done"}],"kanban":$file,"later":1,"kanbans":[
            {"id":"work","name":"Work","filter":"project:Work","colour":"red"},{"id":"Bad"}]}}"""
        val parsed = parseSettings(text)
        assertEquals(listOf(KanbanEntry("work", "Work", filter = "project:Work")), parsed.kanban.kanbans)
        val next = parsed.kanban.copy(report = "list", kanbans = listOf(KanbanEntry("work", "Work 2", report = "next")))
        val saved = Json.parseToJsonElement(serializeKanban(parsed.raw, next)).jsonObject
        assertEquals("x", saved["taskCommands"]!!.jsonArray[0].jsonObject["name"]!!.jsonPrimitive.content)
        val kanban = saved["kanban"]!!.jsonObject
        assertEquals("1", kanban["later"]!!.jsonPrimitive.content)
        assertEquals("list", kanban["report"]!!.jsonPrimitive.content)
        val list = kanban["kanbans"]!!.jsonArray
        assertEquals(2, list.size)
        // Its own filter taken away, its own report given.
        assertEquals(
            Json.parseToJsonElement("""{"id":"work","name":"Work 2","report":"next","colour":"red"}"""),
            list[0],
        )
        assertEquals("Bad", list[1].jsonObject["id"]!!.jsonPrimitive.content)
    }

    @Test
    fun writesNothingForAFileWithoutKanbans() {
        val parsed = parseSettings("""{"taskCommands": []}""")
        assertEquals(Json.parseToJsonElement("""{"taskCommands": []}"""), Json.parseToJsonElement(serializeKanban(parsed.raw, parsed.kanban)))
        assertEquals(JsonObject(emptyMap()), parseSettings("").raw)
    }

    @Test
    fun lintsTheShapeAsAnErrorAndABadKanbanAsAWarning() {
        assertEquals(listOf("kanban must be an object"), lintSettings("""{"kanban": []}""").errors)
        assertEquals(listOf("kanban.kanbans must be a list"), lintSettings("""{"kanban": {"kanbans": {}}}""").errors)
        assertEquals(listOf("kanban.columns must be a list"), lintSettings("""{"kanban": {"columns": {}}}""").errors)
        val l = lintSettings("""{"kanban": {"report": "", "kanbans": [{"id": "a", "name": "A"}]}}""")
        assertEquals(emptyList(), l.errors)
        assertEquals(listOf("kanban.kanbans[0] (a): needs a report, its own or Kanban's"), l.warnings)
    }

    @Test
    fun aNewKanbansAddressFollowsItsNameUntilTypedOrSaved() {
        val fresh = newKanban("", listOf("kanban"))
        assertEquals("kanban-2", fresh.id)
        val k = renamedKanban(renamedKanban(fresh, "W", false, listOf("kanban")), "Work", false, listOf("kanban"))
        assertEquals(KanbanEntry("work", "Work"), k)
        assertEquals("mine", renamedKanban(KanbanEntry("mine", "Work"), "Work 2", false, listOf("kanban")).id)
        assertEquals("work", renamedKanban(KanbanEntry("work", "Work"), "Jobs", true, listOf("kanban")).id)
        assertEquals("kanban-2", renamedKanban(fresh, "Kanban", false, listOf("kanban")).id)
    }
}
