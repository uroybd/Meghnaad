package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

// Ported from web/src/lib/tasksh.test.ts.
class TaskshTest {
    private val want = ReviewWanted(REVIEW_FILTER, REVIEW_SORT)

    private fun col(spec: String) = Column(spec, spec, null, spec, "string")

    private fun config(uda: Pair<String, String?>? = null, report: List<String>? = null, filter: String = REVIEW_FILTER, sort: String = REVIEW_SORT) =
        ConfigResponse(
            config = ConfigBody(udas = if (uda != null) mapOf("reviewed" to UdaDef("reviewed", uda.first, uda.second)) else emptyMap()),
            reports = if (report != null) listOf(ReportMeta("_reviewed", "x", report.map(::col), filter, sort)) else listOf(ReportMeta("next")),
        )

    private val date = "date" to "Reviewed"

    @Test
    fun isNotEnabledUntilTheUdaAndTheReportAreBothThere() {
        assertFalse(taskshStatus(null).enabled)
        assertEquals(Part.Missing, taskshStatus(null).uda)
        assertEquals(Part.Missing, taskshStatus(config(uda = date)).report)
        assertFalse(taskshStatus(config(report = listOf("uuid"))).enabled)
        assertTrue(taskshStatus(config(uda = date, report = listOf("uuid"))).enabled)
    }

    @Test
    fun saysWhatIsThereWhenItIsNotTheWayTaskshNeedsIt() {
        val s = taskshStatus(config(uda = "string" to null, report = listOf("id", "uuid")))
        assertEquals(Part.Different, s.uda)
        assertEquals("string", s.udaType)
        assertEquals(Part.Different, s.report)
        assertFalse(s.enabled)
        assertEquals(listOf("id", "uuid"), s.columns)
    }

    @Test
    fun writesEverythingTaskshDoesWithTheColumnsBeforeTheRestOfTheReport() {
        assertEquals(
            listOf(
                "uda.reviewed.type" to "date",
                "uda.reviewed.label" to "Reviewed",
                "report._reviewed.columns" to "uuid",
                "report._reviewed.description" to "Tasksh review report.  Adjust the filter to your needs.",
                "report._reviewed.sort" to "reviewed+,modified+",
                "report._reviewed.filter" to "( reviewed.none: or reviewed.before:now-6days ) and ( +PENDING or +WAITING )",
            ),
            setupLines(config(), want),
        )
    }

    @Test
    fun writesOnlyWhatIsMissing() {
        assertEquals(
            listOf("report._reviewed.columns", "report._reviewed.description", "report._reviewed.sort", "report._reviewed.filter"),
            setupLines(config(uda = date), want).map { it.first },
        )
        assertEquals(listOf("uda.reviewed.type" to "date", "uda.reviewed.label" to "Reviewed"), setupLines(config(report = listOf("uuid")), want))
        assertEquals(emptyList(), setupLines(config(uda = date, report = listOf("uuid")), want))
    }

    @Test
    fun changesAUdaOfAnotherTypeButKeepsALabelItHasAndRewritesAReportWithOtherColumns() {
        val lines = setupLines(config(uda = "string" to "When", report = listOf("id")), want)
        assertEquals("uda.reviewed.type" to "date", lines[0])
        assertEquals(
            listOf("uda.reviewed.type", "report._reviewed.columns", "report._reviewed.sort", "report._reviewed.filter"),
            lines.map { it.first },
        )
    }

    @Test
    fun appliesAnAdjustedFilterOrSortAndNothingElse() {
        val c = config(uda = date, report = listOf("uuid"))
        val adjusted = ReviewWanted("reviewed.before:now-30days and +PENDING", REVIEW_SORT)
        assertEquals(listOf("report._reviewed.filter" to adjusted.filter), setupLines(c, adjusted))
        assertEquals(listOf("report._reviewed.sort" to "urgency-"), setupLines(c, want.copy(sort = "urgency-")))
    }

    @Test
    fun onlyEverNamesTheReviewedUdaAndTheReviewedReport() {
        for ((name) in setupLines(config(), want)) assertTrue(name.startsWith("uda.reviewed.") || name.startsWith("report._reviewed."), name)
    }

    @Test
    fun asksNoQuestionsAndKeepsAValueWithSpacesAsOneWord() {
        assertEquals(
            listOf("rc.confirmation:no", "rc.verbose:nothing", "config", "report._reviewed.filter", "a and b"),
            configArgs("report._reviewed.filter" to "a and b"),
        )
    }

    @Test
    fun readsTheReviewCommandAsTaskshDoes() {
        for (line in listOf("review", "task review", "rev", "revi", "REVIEW", "  review  ")) assertEquals(0, parseReview(line), line)
        assertEquals(10, parseReview("review 10"))
        assertEquals(3, parseReview("task rev 3"))
    }

    @Test
    fun leavesEverythingElseToTaskwarrior() {
        for (line in listOf("", "re", "reviews", "review foo", "review 1 2", "review 3x", "revue", "next", "add review the code", "project:Work review")) {
            assertNull(parseReview(line), line)
        }
    }

    @Test
    fun readsTheMenuKeys() {
        assertEquals(ReviewKey.Reviewed, menuKey(""))
        assertEquals(ReviewKey.Reviewed, menuKey(" R "))
        assertEquals(ReviewKey.Quit, menuKey("q"))
        assertNull(menuKey("x"))
    }

    @Test
    fun asksTheQuestionPerPartInTaskshWords() {
        val missing = taskshStatus(null)
        assertTrue("Tasksh needs to define a 'reviewed' UDA" in setupQuestion(SetupPart.Uda, missing))
        val different = taskshStatus(config(uda = "string" to null, report = listOf("id", "uuid")))
        assertTrue("type 'string' now" in setupQuestion(SetupPart.Uda, different))
        assertTrue("'id, uuid' now" in setupQuestion(SetupPart.Report, different))
        assertEquals(listOf("uda.reviewed.type" to "date"), partLines(listOf("uda.reviewed.type" to "date", "report._reviewed.columns" to "uuid"), SetupPart.Uda))
    }
}
