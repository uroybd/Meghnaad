package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

// The same table as web/src/lib/highlight.test.ts: the page and the app colour a line the same way.
class HighlightTest {
    private val commands = setOf("add", "modify", "done", "list", "next", "info")

    private fun parts(line: String) = highlight(line, commands).map { line.substring(it.start, it.end) to it.kind.name }

    @Test
    fun aCommandLineWithAnAttributeAndATag() {
        assertEquals(
            listOf("task" to "Keyword", "add" to "Command", "Buy" to "Text", "milk" to "Text", "due" to "Attr", ":" to "Punct", "tomorrow" to "Value", "+home" to "Tag"),
            parts("task add Buy milk due:tomorrow +home"),
        )
    }

    @Test
    fun idsBeforeTheCommand() {
        assertEquals(listOf("1,3-5" to "Id", "done" to "Command"), parts("1,3-5 done"))
        assertEquals(listOf("3f2a9b1c-1111-2222-3333-444455556666" to "Id", "info" to "Command"), parts("3f2a9b1c-1111-2222-3333-444455556666 info"))
    }

    @Test
    fun bracketsAndOperators() {
        assertEquals(
            listOf("project" to "Attr", ":" to "Punct", "Home" to "Value", "(" to "Op", "+work" to "Tag", "or" to "Op", "+play" to "Tag", ")" to "Op", "list" to "Command"),
            parts("project:Home (+work or +play) list"),
        )
        assertEquals(listOf("(" to "Op", "(" to "Op", "+a" to "Tag", ")" to "Op", ")" to "Op"), parts("((+a))"))
    }

    @Test
    fun quotedWordsAreOneThing() {
        assertEquals(
            listOf("add" to "Command", "\"buy milk\"" to "Str", "due" to "Attr", ":" to "Punct", "\"next week\"" to "Str"),
            parts("add \"buy milk\" due:\"next week\""),
        )
    }

    @Test
    fun settingsForOneCommand() {
        assertEquals(
            listOf("rc.confirmation" to "Config", ":" to "Punct", "no" to "Config", "1" to "Id", "modify" to "Command", "-old" to "Tag"),
            parts("rc.confirmation:no 1 modify -old"),
        )
    }

    @Test
    fun anAttributeWithAModifierAndAnEmptyValue() {
        assertEquals(listOf("due.before" to "Attr", ":" to "Punct", "eow" to "Value"), parts("due.before:eow"))
        assertEquals(listOf("project" to "Attr", ":" to "Punct"), parts("project:"))
    }

    @Test
    fun onlyTheFirstCommandCountsAndUnknownWordsAreText() {
        assertEquals(listOf("add" to "Command", "list" to "Text", "done" to "Text"), parts("add list done"))
        assertEquals(listOf("unknown" to "Text", "thing" to "Text"), parts("unknown thing"))
        assertEquals(emptyList(), parts("   "))
    }

    @Test
    fun aMinusBeforeNumbersIsNotATag() {
        assertEquals(listOf("add" to "Command", "-5" to "Text"), parts("add -5"))
    }

    @Test
    fun afterAddTheRestIsADescription() {
        assertEquals(
            listOf("add" to "Command", "milk" to "Text", "and" to "Text", "eggs" to "Text", "+home" to "Tag"),
            parts("add milk and eggs +home"),
        )
    }
}
