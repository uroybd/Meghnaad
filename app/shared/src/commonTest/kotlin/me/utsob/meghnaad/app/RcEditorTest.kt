package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

// The same cases as web/src/lib/rcEditor.test.ts. What a line is is the engine's (tc_core::rc_highlight, tested there); these check
// that the app carries it across, and the numbers that go beside the text.
class RcEditorTest {
    private fun named(text: String) = rcTokens(text).map { text.substring(it.start, it.end) to it.kind }

    @Test
    fun theKindsOfAPieceComeFromTheEngine() {
        assertEquals(
            listOf("bulk" to Rc.Key, "=" to Rc.Punct, "3" to Rc.Value, "# three" to Rc.Comment),
            named("bulk=3 # three"),
        )
        assertEquals(listOf("include" to Rc.Keyword, "holidays/en.rc" to Rc.Path), named("include holidays/en.rc"))
        assertEquals(listOf("not a setting" to Rc.Bad), named("not a setting"))
        assertEquals(Rc.Secret, rcTokens("sync.encryption_secret=x").first().kind)
        assertEquals(listOf("# " to Rc.Comment, "[unsupported]" to Rc.Marker, " a=1" to Rc.Comment), named("# [unsupported] a=1"))
    }

    @Test
    fun aLineThatWrapsTakesItsNumberOnce() {
        val text = "a=1\nlong long\nb=2\n"
        // Drawn: a=1 / long / long / b=2 / (the empty last line).
        assertEquals(listOf(1, 2, null, 3, 4), lineNumbers(text, listOf(0, 4, 9, 14, 18)))
        assertEquals(listOf(1), lineNumbers("", listOf(0)))
    }
}
