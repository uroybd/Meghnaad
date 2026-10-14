package me.utsob.meghnaad.app

import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class LucideTest {
    @Test
    fun everyIconTravelsWithTheApp() = runBlocking {
        LucideLib.load()
        assertTrue(LucideLib.loaded)
        assertTrue(LucideLib.names().size > 1800, "${LucideLib.names().size} icons")
        // A path, a path with a circle, and a rect: the shapes an icon is made of.
        val check = assertNotNull(LucideLib.shapes("check"))
        assertEquals("path", check.single().tag)
        assertEquals(setOf("path", "circle"), LucideLib.shapes("clock")!!.map { it.tag }.toSet())
        assertNotNull(LucideLib.shapes("calendar")!!.first { it.tag == "rect" })
        assertNull(LucideLib.shapes("no-such-icon"))
    }

    @Test
    fun theWordsAnIconIsFoundBy() = runBlocking {
        LucideLib.load()
        // "cancel finds the circled cross": the picker searches these.
        assertTrue("cancel" in LucideLib.tags.getValue("circle-x"))
    }
}
