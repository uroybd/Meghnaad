package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

class TaskCommandsTest {
    private val fail = TaskCommand("fail", "done outcome:fail")
    private val late = TaskCommand("late", "+OVERDUE list")
    private val cmds = listOf(fail, late, TaskCommand("bad", "fail +flagged"))

    private fun expand(line: String, c: List<TaskCommand> = cmds) = expandTaskCommands(line, c)

    @Test
    fun standsForTheWholeCommandLineWithOrWithoutTask() {
        assertEquals("done outcome:fail", expand("fail"))
        assertEquals("done outcome:fail", expand("task fail"))
    }

    @Test
    fun keepsTheFilterBeforeItAndWhatFollowsAfter() {
        assertEquals("3 done outcome:fail", expand("3 fail"))
        assertEquals("3,5-7 done outcome:fail", expand("task 3,5-7 fail"))
        assertEquals("project:Home +work done outcome:fail", expand("project:Home +work fail"))
        assertEquals("( +a or +b ) done outcome:fail", expand("( +a or +b ) fail"))
        assertEquals("1 done outcome:fail +x", expand("1 fail +x"))
        assertEquals("+work +OVERDUE list", expand("+work late"))
    }

    @Test
    fun canStandForACommandThatHasAFilterOfItsOwn() {
        assertEquals("+OVERDUE list", expand("late"))
        assertEquals("project:Home +OVERDUE list", expand("project:Home late"))
    }

    @Test
    fun letsOneTaskCommandUseAnother() {
        assertEquals("2 done outcome:fail +flagged", expand("2 bad"))
    }

    @Test
    fun endsALoop() {
        assertEquals("a", expand("a", listOf(TaskCommand("a", "b"), TaskCommand("b", "a"))))
    }

    @Test
    fun winsOverATaskwarriorCommandOrReportOfTheSameName() {
        assertEquals("1 done outcome:ok", expand("1 done", listOf(TaskCommand("done", "done outcome:ok"))))
        assertEquals("list +PENDING", expand("next", listOf(TaskCommand("next", "list +PENDING"))))
        // Written out once, so a command can be extended without looping on itself.
        assertEquals("2 done outcome:ok", expand("task 2 done", listOf(TaskCommand("done", "done outcome:ok"))))
    }

    @Test
    fun leavesAloneAWordThatIsNotWhereTheCommandGoes() {
        assertEquals("add fail the test", expand("add fail the test"))
        assertEquals("1 modify fail", expand("1 modify fail"))
        assertEquals("1 annotate late for work", expand("1 annotate late for work"))
        assertEquals("1 done -- fail", expand("1 done -- fail"))
    }

    @Test
    fun returnsTheLineExactlyAsTypedWhenNothingIsExpanded() {
        assertEquals("1 done   outcome:\"a b\"", expand("1 done   outcome:\"a b\""))
        assertEquals("fail", expand("fail", emptyList()))
        assertEquals("", expand(""))
    }

    @Test
    fun keepsQuotedWordsTogether() {
        assertEquals("'project:My Work' done outcome:fail", expand("project:'My Work' fail"))
        assertEquals("1 annotate 'a b'", expand("1 note", listOf(TaskCommand("note", "annotate 'a b'"))))
    }

    @Test
    fun knowsTheShapesOfAFilter() {
        for (w in listOf("+tag", "-tag", "project:Home", "rc.confirmation=no", "12", "1,3-5", "/re/", "(", ")", "and", "OR")) {
            assertEquals(true, isFilterWord(w), w)
        }
        assertEquals(true, isFilterWord("2c2b9cb2-0f55-4a5c-9b7a-2f5b3c0e1d11"))
        for (w in listOf("done", "fail", "next", "add", "deadbeef")) assertEquals(false, isFilterWord(w), w)
    }

    @Test
    fun commandStepsIsWhatATapAddsAfterTheTasksAStatementAtATime() {
        assertEquals(listOf(CommandStep(listOf("modify", "a b", "+c"), null)), commandSteps(TaskCommand("x", "modify 'a b' +c")))
        assertEquals(
            listOf(
                CommandStep(listOf("done"), null),
                CommandStep(listOf("modify", "+failed"), Joiner.And),
                CommandStep(listOf("annotate", "x"), Joiner.Then),
            ),
            commandSteps(TaskCommand("x", "done && modify +failed ; annotate x")),
        )
    }

    private val both = cmds + listOf(
        TaskCommand("wrap", "done outcome:fail && modify +failed"),
        TaskCommand("tidy", "wrap || annotate gave up"),
    )

    private fun plan(line: String) = planChain(line, both).map { it.join to it.line }

    @Test
    fun aLineIsItselfWhenItIsOneStatement() {
        assertEquals(listOf(null to "1 done"), plan("1 done"))
        assertEquals(emptyList(), plan(""))
    }

    @Test
    fun writesOutTheTaskCommandsOfEachStatement() {
        assertEquals(listOf(null to "3 done outcome:fail", Joiner.Then to "+OVERDUE list"), plan("3 fail ; late"))
    }

    @Test
    fun putsTheFilterInFrontOfEveryStatementACommandStandsForAndWhatFollowsAfterTheLast() {
        assertEquals(listOf(null to "3 done outcome:fail", Joiner.And to "3 modify +failed +x"), plan("3 wrap +x"))
    }

    @Test
    fun putsACommandThatStandsForAChainIntoTheChainItIsIn() {
        assertEquals(
            listOf(
                null to "count",
                Joiner.And to "7 done outcome:fail",
                Joiner.And to "7 modify +failed",
                Joiner.Or to "7 annotate gave up",
                Joiner.Then to "list",
            ),
            plan("count && 7 tidy ; list"),
        )
    }

    @Test
    fun doesNotTakeAnOperatorInQuotesForOne() {
        assertEquals(listOf(null to "add 'a && b'", Joiner.Then to "add c';'d"), plan("add 'a && b' ; add c\\;d"))
    }

    @Test
    fun writesAChainBackAsALine() {
        assertEquals("3 done outcome:fail && 3 modify +failed", expandTaskCommands("3 wrap", both))
        assertEquals("1 done ; 2 done", expandTaskCommands("1 done ; 2 done", both))
    }
}
