package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class CommandLineTest {
    @Test
    fun splitsOnWhitespaceAndHonoursQuotes() {
        assertEquals(listOf("project:Home Stuff", "a b", "c"), splitWords("""project:"Home Stuff" 'a b' c"""))
    }

    @Test
    fun handlesEscapesAndEmpties() {
        assertEquals(listOf("a b", "c"), splitWords("a\\ b c"))
        assertEquals(emptyList(), splitWords("   "))
        assertEquals(listOf(""), splitWords("''"))
    }

    @Test
    fun previewShowsWhatWouldBeTyped() {
        assertEquals("task project:Home +work list", previewLine(listOf("project:Home", "+work", "list")))
        assertEquals("task add 'description:Buy milk'", previewLine(listOf("add", "description:Buy milk")))
        assertEquals("'it'\\''s'", shellQuote("it's"))
        assertEquals("''", shellQuote(""))
    }

    @Test
    fun previewRoundTripsThroughSplitWords() {
        val args = listOf("add", "description:Buy \"fresh\" milk", "project:O'Neil", "due:2026-12-25T08:30")
        assertEquals(args, splitWords(previewLine(args).drop(5)))
    }

    @Test
    fun theReportGoesAfterTheFilter() {
        assertEquals(listOf("project:Home", "+work", "next"), reportArgs("project:Home +work", "next"))
        assertEquals(listOf("list"), reportArgs("", "list"))
    }

    @Test
    fun aTagIsToggledInAFilter() {
        assertEquals(listOf("work", "home"), activeTags("project:A +work -x +home"))
        assertEquals("+work", toggleTag("", "work"))
        assertEquals("", toggleTag("+work", "work"))
        assertEquals("project:Home +work", toggleTag("project:Home -work", "work"))
    }
}
