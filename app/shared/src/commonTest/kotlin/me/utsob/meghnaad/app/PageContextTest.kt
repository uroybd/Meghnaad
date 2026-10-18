package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertSame

// The same cases as web/src/lib/pageContext.test.ts.
class PageContextTest {
    @Test
    fun aCommandInAPagesContextIsSentAsTaskwarriorsOwnOverride() {
        assertEquals("rc.context=work", contextWord("work"))
        assertEquals(CliInput(args = listOf("rc.context=work", "+x", "export")), withContext(CliInput(args = listOf("+x", "export")), "work"))
        assertEquals(CliInput(line = "rc.context=none next"), withContext(CliInput(line = "next"), NO_CONTEXT))
    }

    @Test
    fun aPageThatFollowsTheActiveContextIsLeftAlone() {
        val input = CliInput(args = listOf("next"))
        assertSame(input, withContext(input, null))
    }

    @Test
    fun theChoicesComeBackAsSavedAndADamagedValueIsNothing() {
        val choices = mapOf("tasks" to "work", "board:b" to NO_CONTEXT)
        assertEquals(choices, loadChoices(saveChoices(choices)))
        assertEquals(emptyMap(), loadChoices("[1,2]"))
        assertEquals(emptyMap(), loadChoices("not json"))
        assertEquals(emptyMap(), loadChoices(null))
        assertEquals(mapOf("a" to "x"), loadChoices("""{"a":"x","c":""}"""))
    }

    @Test
    fun eachPageHasItsOwnScope() {
        assertEquals("summary", Scope.panel(Panel.Summary))
        assertEquals("board:b1", Scope.board("b1"))
    }
}
