package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

// The same cases as web/src/lib/addPreview.test.ts. What a line means is the engine's (tc_core::preview, tested there); these
// check that the app carries it across, and the form's fields it starts from.
class AddPreviewTest {
    private val utc: (Long) -> Int = { 0 }
    private fun preview(line: String, config: ConfigBody? = null) = previewAdd(line, config, 1_792_324_800L, 0)

    @Test
    fun aLineIsBrokenIntoWhatAddWouldSet() {
        val p = preview("Buy milk project:Home priority:H +errand due:2026-12-24")
        assertEquals("Buy milk", p.description)
        assertEquals("Home", p.project)
        assertEquals("H", p.priority)
        assertEquals(listOf("errand"), p.tags)
        assertEquals(listOf("due"), p.dates.keys.toList())
        assertTrue(p.errors.isEmpty())
    }

    @Test
    fun quotesKeepAWordInTheDescriptionAndTheArgumentsAreTheSameWords() {
        val p = preview("'read project:x' later")
        assertEquals("read project:x later", p.description)
        assertNull(p.project)
        assertEquals(listOf("add", "read project:x", "later", "+a"), addArgs("'read project:x' later +a"))
    }

    @Test
    fun aBadPartIsReportedAndTheRestShows() {
        val p = preview("Pay rent due:nonsense +bills")
        assertEquals("Pay rent", p.description)
        assertEquals(listOf("bills"), p.tags)
        assertEquals(1, p.errors.size)
    }

    @Test
    fun theConfigsUdasAreKnown() {
        val udas = mapOf("estimate" to UdaDef(name = "estimate", type = "numeric"))
        val p = preview("Fix estimate:3", ConfigBody(udas = udas))
        assertEquals(mapOf("estimate" to "3"), p.udas)
        assertEquals("Fix", p.description)
    }

    @Test
    fun theFormStartsFromTheBreakdown() {
        val seed = seedOf(preview("Ship it project:Work +a +b priority:M due:2026-12-24T09:30 depends:3"), offsetAt = utc)
        assertEquals("Ship it", seed.description)
        assertEquals("Work", seed.project)
        assertEquals("M", seed.priority)
        assertEquals("a b", seed.tags)
        assertEquals("2026-12-24T09:30", seed.due)
        assertEquals("3", seed.depends)
    }

    @Test
    fun aDateUdaIsWrittenAsTheFormHoldsItAndWhatHasNoFieldIsCarried() {
        val defs = mapOf("seen" to UdaDef(name = "seen", type = "date"))
        val p = preview("Look seen:2026-12-01 start:2026-12-02", ConfigBody(udas = defs))
        val seed = seedOf(p, defs = defs, offsetAt = utc)
        assertEquals("2026-12-01", seed.udas["seen"])
        assertEquals(listOf("start:2026-12-02"), seed.carried)
    }

    @Test
    fun withNoBreakdownOnlyTheTextIsKept() {
        val seed = seedOf(null, "hello", offsetAt = utc)
        assertEquals("hello", seed.description)
        assertEquals("", seed.project)
    }
}
