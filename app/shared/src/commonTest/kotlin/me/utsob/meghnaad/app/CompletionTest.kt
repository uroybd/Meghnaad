package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class CompletionTest {
    private val vocab = Vocab(
        contexts = listOf("work", "home"),
        reports = listOf(ReportName("next", "Most urgent"), ReportName("list"), ReportName("mine", "My tasks")),
        tasks = listOf(
            TaskRef(3, "aaaa1111-0000-0000-0000-000000000000", "Fix the tap"),
            TaskRef(30, "bbbb2222-0000-0000-0000-000000000000", "Write report"),
            TaskRef(null, "cccc3333-0000-0000-0000-000000000000", "Old done task"),
        ),
        projects = listOf("Home", "Home.Kitchen", "Work"),
        tags = listOf("errand", "work", "waiting-on"),
        udas = listOf(
            UdaDef(name = "estimate", type = "string", label = "Size", values = listOf("big", "small", "")),
            UdaDef(name = "ship", type = "date"),
        ),
    )

    /** The option *values* (hints are checked separately). */
    private fun c(line: String, caret: Int = line.length) = complete(line, caret, vocab)

    private fun values(line: String) = c(line).options.map { it.value }

    @Test
    fun commandsAndReportsByPrefix() {
        assertEquals(listOf("limit:", "list"), values("li"))
        assertEquals(listOf("list"), values("lis"))
        assertEquals(listOf("modify"), values("mo"))
        assertEquals(listOf("mine"), values("mi"))
        assertEquals(listOf("done"), values("3 do"))
    }

    @Test
    fun attributeNamesGetAColonAndSharePrefixesWithCommands() {
        assertEquals(listOf("project:", "projects"), values("proj"))
        assertEquals(listOf("estimate:"), values("est"))
    }

    @Test
    fun projectValues() {
        assertEquals(listOf("project:Home", "project:Home.Kitchen"), values("project:Ho"))
        assertEquals("project:Home", c("project:Ho").common)
        assertEquals(listOf("project.is:Work"), values("project.is:W"))
    }

    @Test
    fun priorityStatusUdaValuesAndDateWords() {
        assertEquals(listOf("priority:H", "priority:L", "priority:M"), values("priority:"))
        assertEquals(listOf("priority:H"), values("priority:H"))
        assertEquals(listOf("estimate:big"), values("estimate:b"))
        assertEquals(listOf("estimate:big", "estimate:small"), values("estimate:")) // blank value omitted
        assertEquals(listOf("due:tomorrow"), values("due:tom"))
        assertEquals(listOf("due.before:eod", "due.before:eom", "due.before:eow", "due.before:eoy"), values("due.before:e"))
        assertEquals(listOf("ship:today", "ship:tomorrow"), values("ship:to"))
        assertEquals(listOf("status:waiting"), values("status:wa"))
    }

    @Test
    fun tagsAndVirtualTags() {
        assertEquals(listOf("+WAITING", "+WEEK", "+waiting-on", "+work"), values("+w"))
        assertEquals(listOf("-errand"), values("-err"))
        assertEquals(listOf("+OVERDUE"), values("+OVER"))
    }

    @Test
    fun completesTheWordAtTheCaretNotTheWholeLine() {
        val line = "project:Home +er due:today"
        val r = complete(line, "(project:Home +er".length - 1, vocab)
        assertEquals(listOf("+errand"), r.options.map { it.value })
        assertEquals("project:Home ".length, r.start)
    }

    @Test
    fun nothingForAnEmptyWord() {
        assertEquals(emptyList(), values("list "))
    }

    @Test
    fun aUniqueMatchIsCompletedWithATrailingSpace() {
        val r = apply("3 do", complete("3 do", 4, vocab))
        assertEquals("3 done ", r.line)
        assertEquals(7, r.caret)
        assertTrue(r.progressed)
    }

    @Test
    fun anAttributeCompletesUpToTheColon() {
        assertEquals("estimate:", apply("est", complete("est", 3, vocab)).line)
    }

    @Test
    fun aStemSharedByAnAttributeAndACommandTakesTheCommonPrefix() {
        assertEquals("project", apply("proj", complete("proj", 4, vocab)).line)
    }

    @Test
    fun ambiguousMatchesTakeTheCommonPrefixOnly() {
        assertEquals("project:Home", apply("project:Ho", complete("project:Ho", 10, vocab)).line)
    }

    @Test
    fun noOptionsLeavesTheLineAlone() {
        assertEquals("zzz", apply("zzz", complete("zzz", 3, vocab)).line)
    }

    @Test
    fun replacesOnlyTheWordUnderTheCaret() {
        val line = "project:Home +er due:today"
        val r = apply(line, complete(line, "project:Home +er".length, vocab))
        assertEquals("project:Home +errand  due:today", r.line)
    }

    @Test
    fun commandsAndReportsCarryHints() {
        assertEquals(listOf(Option("mine", "My tasks")), complete("mi", 2, vocab).options)
        assertEquals(Option("done", "complete tasks"), complete("don", 3, vocab).options[0])
    }

    @Test
    fun aNumberCompletesToTaskIdsWithTheirDescriptions() {
        val o = complete("3", 1, vocab).options
        assertEquals(listOf("3", "30"), o.map { it.value })
        assertEquals("Fix the tap", o[0].hint)
        assertEquals(listOf("30"), complete("30", 2, vocab).options.map { it.value })
        assertEquals(emptyList(), complete("9", 1, vocab).options)
    }

    @Test
    fun dependsCompletesTheLastItemOfACommaList() {
        assertEquals(listOf("depends:3", "depends:30"), values("depends:3"))
        assertEquals(listOf("depends:3,3", "depends:3,30"), values("depends:3,3"))
        assertEquals(listOf("depends:3,3", "depends:3,30"), values("depends:3,-3"))
    }

    @Test
    fun namePartialCompletesModifiersAppropriateToTheAttributeType() {
        assertEquals(listOf("due.before:"), values("due.be"))
        assertTrue("due.after:" in values("due."))
        assertFalse("due.has:" in values("due."))
        assertEquals(listOf("project.startswith:"), values("project.st"))
        assertEquals(listOf("estimate.is:", "estimate.isnt:"), values("estimate.i"))
        assertEquals(listOf("ship.before:", "ship.by:"), values("ship.b"))
        assertEquals(emptyList(), values("nonsense.b"))
        assertEquals("earlier than", complete("due.be", 6, vocab).options[0].hint)
    }

    @Test
    fun dateValuesIncludeRelativeShortcuts() {
        assertEquals(listOf("due:3d"), values("due:3"))
        assertEquals(listOf("wait:1d", "wait:1mo", "wait:1w"), values("wait:1"))
    }

    @Test
    fun limitHasValues() {
        assertEquals(listOf("limit:page"), values("limit:p"))
    }

    @Test
    fun insertAddsASpaceUnlessTheValueEndsMidToken() {
        val cc = complete("3 do", 4, vocab)
        assertEquals("3 done " to 7, insert("3 do", cc, "done").let { it.line to it.caret })
        assertEquals("3 due:" to 6, insert("3 do", cc, "due:").let { it.line to it.caret })
        assertEquals("3 done" to 6, insert("3 do", cc, "done", false).let { it.line to it.caret })
    }

    @Test
    fun rcOffersReportAndContext() {
        assertEquals(listOf("rc.context:", "rc.report."), values("rc."))
        assertEquals(listOf("rc.report."), values("rc.r"))
    }

    @Test
    fun rcReportListsReportsWithTheirDescriptions() {
        assertEquals(listOf("rc.report.list.", "rc.report.mine.", "rc.report.next."), values("rc.report."))
        assertEquals(listOf("rc.report.mine."), values("rc.report.mi"))
        assertEquals("My tasks", complete("rc.report.mi", 12, vocab).options[0].hint)
    }

    @Test
    fun rcReportNameListsTheSettings() {
        assertEquals(
            listOf("rc.report.next.columns:", "rc.report.next.filter:", "rc.report.next.labels:", "rc.report.next.sort:"),
            values("rc.report.next."),
        )
        assertEquals(listOf("rc.report.next.sort:"), values("rc.report.next.so"))
    }

    @Test
    fun sortValuesCompleteColumnNamesWithADirectionIncludingUdas() {
        assertEquals(listOf("rc.report.next.sort:due+", "rc.report.next.sort:due-"), values("rc.report.next.sort:du"))
        assertEquals(
            listOf("rc.report.next.sort:due+,estimate+", "rc.report.next.sort:due+,estimate-"),
            values("rc.report.next.sort:due+,es"),
        )
        // A direction was already chosen: completing it leaves the list open for a `,`.
        val done = complete("rc.report.next.sort:due+", 24, vocab)
        assertEquals(listOf("rc.report.next.sort:due+"), done.options.map { it.value })
        assertEquals("rc.report.next.sort:due+", apply("rc.report.next.sort:due+", done).line)
    }

    @Test
    fun rcContextListsContexts() {
        assertEquals(listOf("rc.context:work"), values("rc.context:w"))
    }

    @Test
    fun recurOffersTheCommonPeriodsWithAPlainEnglishHint() {
        assertEquals(listOf("recur:weekdays", "recur:weekly"), values("recur:we"))
        assertEquals("every 2 weeks", complete("recur:bi", 8, vocab).options[0].hint)
        assertTrue("recur:" in values("rec"))
    }
}

class TypingSuggestionsTest {
    private val vocab = Vocab(projects = listOf("Home", "Home.Kitchen", "Work"), tags = listOf("errand", "work"))

    @Test
    fun aWordBeingTypedIsOfferedItsCompletionsWithoutChangingTheLine() {
        val completer = Completer()
        completer.suggest("pro", 3, vocab)
        val menu = completer.menu!!
        assertEquals("pro", menu.base)
        assertTrue(menu.options.any { it.value.startsWith("project:") }, menu.options.toString())
        assertEquals(-1, menu.sel)
    }

    @Test
    fun aValueAboutToBeTypedIsOfferedToo() {
        val completer = Completer()
        completer.suggest("project:", 8, vocab)
        assertEquals(listOf("Home", "Home.Kitchen", "Work"), completer.menu!!.options.map { it.value.removePrefix("project:") })
    }

    @Test
    fun nothingIsOfferedForAnEmptyWordOrWhatIsAlreadyThere() {
        val completer = Completer()
        completer.suggest("", 0, vocab)
        assertNull(completer.menu)
        completer.suggest("add ", 4, vocab)
        assertNull(completer.menu)
        completer.suggest("project:Work", 12, vocab)
        assertNull(completer.menu)
    }

    @Test
    fun aTappedSuggestionTakesTheWordsPlace() {
        val completer = Completer()
        completer.suggest("project:Ho", 10, vocab)
        val edit = completer.pick(0)!!
        assertTrue(edit.line.startsWith("project:Home"), edit.line)
        assertNull(completer.menu)
    }
}
