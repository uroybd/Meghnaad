package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

class FormatTest {
    private val now = 1_791_376_200L // 2026-10-07T12:30:00Z
    private val day = 86_400L

    private fun row(
        description: String = "Buy milk",
        status: String = "pending",
        project: String? = null,
        priority: String? = null,
        tags: List<String> = emptyList(),
        annotations: List<Note> = emptyList(),
        start: Long? = null,
        end: Long? = null,
        due: Long? = null,
        depends: List<String> = emptyList(),
        pendingDeps: Int = 0,
        recur: String? = null,
        blocked: Boolean = false,
        waiting: Boolean = false,
        extra: Map<String, String> = emptyMap(),
        urgency: Double = 4.5,
        id: Int? = 3,
    ) = Row(
        uuid = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        status = status,
        description = description,
        project = project,
        priority = priority,
        tags = tags,
        annotations = annotations,
        entry = now - 3 * day,
        start = start,
        end = end,
        due = due,
        depends = depends,
        pendingDeps = pendingDeps,
        recur = recur,
        blocked = blocked,
        waiting = waiting,
        extra = extra,
        urgency = urgency,
        id = id,
    )

    private fun col(spec: String, kind: String = "string"): Column {
        val name = spec.substringBefore('.')
        val format = spec.substringAfter('.', "").ifEmpty { null }
        return Column(spec = spec, name = name, format = format, label = name, kind = kind)
    }

    private val ctx = Ctx(now = now, tz = 0)

    @Test
    fun showsTheIdOrADimmedShortUuidWhenThereIsNone() {
        assertEquals("3", cell(col("id"), row(), ctx).text)
        assertEquals(Cell("aaaaaaaa", cls = "dim", uuid = row().uuid), cell(col("id"), row(id = null), ctx))
    }

    @Test
    fun formatsDatesAbsoluteRelativeAgeRemaining() {
        val due = now + 2 * day // exactly 12:30
        assertEquals("2026-10-09 12:30", cell(col("due"), row(due = due), ctx).text)
        assertEquals("2026-10-09", cell(col("due"), row(due = due - 12 * 3600 - 30 * 60), ctx).text)
        assertEquals("2d", cell(col("due.relative"), row(due = due), ctx).text)
        assertEquals("-3d", cell(col("due.relative"), row(due = now - 3 * day), ctx).text)
        assertEquals("", cell(col("due.remaining"), row(due = now - day), ctx).text)
        assertEquals("3d", cell(col("entry.age"), row(), ctx).text)
        assertEquals("", cell(col("due"), row(), ctx).text)
    }

    @Test
    fun startActiveIsAStarWhileStartedAndNotEnded() {
        assertEquals("*", cell(col("start.active"), row(start = now), ctx).text)
        assertEquals("", cell(col("start.active"), row(), ctx).text)
        assertEquals("", cell(col("start.active"), row(start = now, end = now + 5), ctx).text)
    }

    @Test
    fun theIndicatorSettingsChangeWhatTheIndicatorColumnsShow() {
        val custom = Ctx(now = now, tz = 0, indicators = Indicators(active = ">>", tag = "#", dependency = "DEP"))
        assertEquals(">>", cell(col("start.active"), row(start = now), custom).text)
        assertEquals("#", cell(col("tags.indicator"), row(tags = listOf("x")), custom).text)
        assertEquals("", cell(col("tags.indicator"), row(), custom).text)
        assertEquals("DEP", cell(col("depends.indicator"), row(pendingDeps = 1), custom).text)
        // Unset, they are Taskwarrior's: *, + and D.
        assertEquals("+", cell(col("tags.indicator"), row(tags = listOf("x")), ctx).text)
    }

    @Test
    fun onlyDependenciesStillOpenCount() {
        // It depends on a task that has since been finished: nothing is holding it up.
        val done = row(depends = listOf("u"))
        assertEquals("", cell(col("depends.indicator"), done, ctx).text)
        assertEquals("", cell(col("depends.count"), done, ctx).text)
        assertEquals("[2]", cell(col("depends.count"), row(depends = listOf("u", "v", "w"), pendingDeps = 2), ctx).text)
    }

    @Test
    fun anIdCarriesItsUuid() {
        val c = cell(col("id"), row(), ctx)
        assertEquals("3", c.text)
        assertEquals("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee", c.uuid)
    }

    @Test
    fun aProjectIsSplitIntoItsParts() {
        val c = cell(col("project"), row(project = "Home.Kitchen.Sink"), ctx)
        assertEquals("Home.Kitchen.Sink", c.text)
        assertEquals(listOf("Home", "Kitchen", "Sink"), c.segments)
        // The parent format shows the path above the project, still in parts; no project, no parts.
        assertEquals(listOf("Home"), cell(col("project.parent"), row(project = "Home.Kitchen"), ctx).segments)
        assertNull(cell(col("project"), row(project = null), ctx).segments)
    }

    @Test
    fun tagsArePillsExceptInTheCompactFormats() {
        val r = row(tags = listOf("errand", "next"))
        assertEquals(listOf("errand", "next"), cell(col("tags"), r, ctx).chips)
        assertEquals("errand next", cell(col("tags"), r, ctx).text)
        assertNull(cell(col("tags"), row(), ctx).chips)
        assertEquals(Cell("[2]"), cell(col("tags.count"), r, ctx))
        assertEquals(Cell("+"), cell(col("tags.indicator"), r, ctx))
        assertEquals(Cell("errand,next"), cell(col("tags.list"), r, ctx))
    }

    @Test
    fun urgencyIsColouredByHowPressingItIs() {
        val levels = listOf(0.0, 4.99, 5.0, 9.99, 10.0, 14.99, 15.0, 40.0).map(::urgencyLevel)
        assertEquals(
            listOf(UrgencyLevel.Low, UrgencyLevel.Low, UrgencyLevel.Mid, UrgencyLevel.Mid, UrgencyLevel.High, UrgencyLevel.High, UrgencyLevel.Crit, UrgencyLevel.Crit),
            levels,
        )
        assertEquals(UrgencyLevel.Low, urgencyLevel(-5.0)) // blocked tasks go negative
        assertEquals(Cell("16.20", cls = "urg-crit"), cell(col("urgency", "number"), row(urgency = 16.2), ctx))
        assertEquals(Cell("6", cls = "urg-mid"), cell(col("urgency.integer", "number"), row(urgency = 6.4), ctx))
    }

    @Test
    fun aMultiLineStringUdaKeepsItsLines() {
        val udas = mapOf("notes" to UdaDef(name = "notes"))
        val c = { value: String -> cell(col("notes"), row(extra = mapOf("notes" to value)), Ctx(now = now, tz = 0, udas = udas)) }
        assertEquals(Cell("one\ntwo", cls = "multiline"), c("one\ntwo"))
        assertEquals(Cell("just one line"), c("just one line"))
    }

    @Test
    fun descriptionFormats() {
        val r = row(description = "x".repeat(60), annotations = listOf(Note(now, "note")))
        assertEquals(listOf("2026-10-07 12:30 note"), cell(col("description"), r, ctx).lines)
        assertNull(cell(col("description.desc"), r, ctx).lines)
        assertEquals(40, cell(col("description.truncated"), r, ctx).text.length)
        assertTrue(cell(col("description.count"), r, ctx).text.endsWith(" [1]"))
        assertEquals("${r.description} 2026-10-07 12:30 note", cell(col("description.oneline"), r, ctx).text)
    }

    @Test
    fun projectTagsDependsStatusPriorityUrgency() {
        assertEquals("a.b", cell(col("project.parent"), row(project = "a.b.c"), ctx).text)
        assertEquals("  b", cell(col("project.indented"), row(project = "a.b"), ctx).text)
        assertEquals("x y", cell(col("tags"), row(tags = listOf("x", "y")), ctx).text)
        assertEquals("[2]", cell(col("tags.count"), row(tags = listOf("x", "y")), ctx).text)
        assertEquals("D", cell(col("depends.indicator"), row(depends = listOf("u"), pendingDeps = 1), ctx).text)
        assertEquals("P", cell(col("status.short"), row(), ctx).text)
        assertEquals("W", cell(col("status.short"), row(waiting = true), ctx).text)
        assertEquals("pri-h", cell(col("priority"), row(priority = "H"), ctx).cls)
        assertEquals("4.50", cell(col("urgency"), row(), ctx).text)
    }

    @Test
    fun udasRenderFromExtraDatesAsDatesAndUnsetAsEmpty() {
        val r = row(extra = mapOf("estimate" to "big", "ship" to (now + day).toString(), "legacy" to "old"))
        assertEquals("big", cell(col("estimate"), r, ctx).text)
        assertEquals("2026-10-08 12:30", cell(col("ship", "date"), r, ctx).text)
        assertEquals("1d", cell(col("ship.relative", "date"), r, ctx).text)
        assertEquals("", cell(col("missing"), r, ctx).text)
        // Orphans render too (read-only is enforced where editing happens).
        assertEquals("old", cell(col("legacy"), r, ctx).text)
    }

    @Test
    fun rowClassFlagsState() {
        assertEquals("done completed", rowClass(row(status = "completed")))
        assertEquals("done deleted", rowClass(row(status = "deleted")))
        assertEquals("blocked active", rowClass(row(start = now, blocked = true)))
        assertEquals("", rowClass(row()))
    }

    private val journal = Journal("Started task", "Stopped task")
    private val withNotes = row(
        annotations = listOf(Note(now, "Started task"), Note(now + 5, "call Sam first"), Note(now + 9, "Stopped task")),
    )

    @Test
    fun journalMarkersAreLeftOutOfTheLinesUnderADescription() {
        val withJournal = Ctx(now = now, tz = 0, journal = journal)
        assertEquals(listOf("2026-10-07 12:30 call Sam first"), cell(col("description"), withNotes, withJournal).lines)
        // Without journalling they are ordinary notes and show.
        assertEquals(3, cell(col("description"), withNotes, ctx).lines?.size)
    }

    @Test
    fun journalMarkersDoNotInflateTheCountOrTheOneLineForm() {
        val withJournal = Ctx(now = now, tz = 0, journal = journal)
        assertEquals("Buy milk [1]", cell(col("description.count"), withNotes, withJournal).text)
        assertEquals("Buy milk 2026-10-07 12:30 call Sam first", cell(col("description.oneline"), withNotes, withJournal).text)
    }

    @Test
    fun aTaskWithOnlyMarkersShowsNoNotesAtAll() {
        val only = row(annotations = listOf(Note(now, "Started task"), Note(now + 5, "Stopped task")))
        assertNull(cell(col("description"), only, Ctx(now = now, tz = 0, journal = journal)).lines)
    }

    @Test
    fun tableDatesAndTheNotesUnderADescriptionFollowTheirPatterns() {
        val withDates = Ctx(now = now, tz = 0, reportDates = DateFmt("D.M.Y", 1), annotationDates = DateFmt("M/D", 1))
        val r = row(due = now, annotations = listOf(Note(now, "called")))
        assertEquals("07.10.2026", cell(col("due"), r, withDates).text)
        assertEquals(listOf("10/07 called"), cell(Column("description", "description", null, "description", "description"), r, withDates).lines)
        // Other column formats ignore the pattern.
        assertEquals(now.toString(), cell(col("due.epoch"), r, withDates).text)
        assertEquals("2026-10-07T12:30:00Z", cell(col("due.iso"), r, withDates).text)
    }

    @Test
    fun recurrenceIndicator() {
        val r = row(recur = "weekly")
        assertEquals("R", cell(col("recur.indicator"), r, ctx).text)
        assertEquals("↻", cell(col("recur.indicator"), r, Ctx(now = now, tz = 0, recurIndicator = "↻")).text)
        assertEquals("", cell(col("recur.indicator"), row(), Ctx(now = now, tz = 0, recurIndicator = "↻")).text)
    }
}
