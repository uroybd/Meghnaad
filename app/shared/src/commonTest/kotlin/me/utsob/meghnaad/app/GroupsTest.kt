package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

// The same cases as web/src/lib/groups.test.ts.
class GroupsTest {
    private fun col(name: String, label: String, kind: String = "string") = Column(spec = name, name = name, format = null, label = label, kind = kind)

    private fun row(project: String?, outcome: String? = null) =
        Row(uuid = project ?: "none", status = "pending", description = "d", project = project, extra = if (outcome != null) mapOf("outcome" to outcome) else emptyMap())

    private fun report(sort: String?, columns: List<Column>, rows: List<Row>, breaks: List<Boolean>) =
        ReportResult(report = "g", columns = columns, rows = rows, breaks = breaks, sort = sort)

    private val ctx = Ctx(now = 0, udas = mapOf("outcome" to UdaDef(name = "outcome", type = "string", label = "Result")))
    private val rows = listOf(row("X", "pass"), row("X", "pass"), row("Y", "fail"), row("Y"))
    private val breaks = listOf(false, false, true, true)

    @Test
    fun aHeaderGoesAboveTheFirstRowOfEachGroupAndNoneInsideIt() {
        val r = report("project+/", listOf(col("project", "Proj", "project")), rows, listOf(false, false, true, false))
        assertEquals(listOf("Proj: X", null, "Proj: Y", null), groupHeads(r, ctx))
    }

    @Test
    fun aUdaIsNamedByItsLabelEvenWhenTheReportDoesNotShowItAndNoValueSaysNone() {
        val r = report("outcome+/", listOf(col("project", "Proj", "project")), rows, breaks)
        assertEquals(listOf("Result: pass", null, "Result: fail", "Result: (none)"), groupHeads(r, ctx))
    }

    @Test
    fun everyColumnThatBreaksTheTableIsJoined() {
        val r = report("project+/,outcome+/", listOf(col("project", "Proj", "project")), rows, breaks)
        assertEquals("Proj: Y · Result: fail", groupHeads(r, ctx)[2])
    }

    @Test
    fun nothingIsDrawnWhenNoSortKeyHasASlash() {
        assertEquals(emptyList(), groupHeads(report("project+,outcome-", emptyList(), rows, breaks), ctx))
        assertEquals(emptyList(), groupHeads(report(null, emptyList(), rows, breaks), ctx))
    }
}
