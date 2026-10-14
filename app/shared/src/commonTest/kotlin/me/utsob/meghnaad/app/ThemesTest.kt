package me.utsob.meghnaad.app

import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ThemesTest {
    private val dark = """# header
color.active=rgb555 on rgb410   # active
color=1   # Enable color
rule.precedence.color=deleted,completed,active

color.tag.next=rgb440
"""

    @Test
    fun themeLinesKeepSettingsAndDropComments() {
        assertEquals(
            listOf("color.active=rgb555 on rgb410", "color=1", "rule.precedence.color=deleted,completed,active", "color.tag.next=rgb440"),
            themeLines(dark),
        )
    }

    private val rc = "bulk=5\ncolor.overdue=red\ncolor=off\nrule.color.merge=off\nuda.size.type=string\n"

    @Test
    fun applyingReplacesTheColourLinesAndKeepsTheRest() {
        val out = applyTheme(rc, "dark-256", dark)
        assertTrue("bulk=5" in out)
        assertTrue("uda.size.type=string" in out)
        assertTrue("color=off" in out)
        assertFalse("color.overdue=red" in out)
        assertFalse("rule.color.merge=off" in out)
        assertTrue("# theme: dark-256\ncolor.active=rgb555 on rgb410" in out)
    }

    @Test
    fun theAppsOwnThemeJustClearsTheColourLines() {
        assertEquals("bulk=5\ncolor=off\nuda.size.type=string\n", applyTheme(rc, "meghnaad", ""))
    }

    @Test
    fun aSecondThemeReplacesTheFirstRatherThanPilingUp() {
        val once = applyTheme("bulk=5\n", "dark-256", dark)
        val twice = applyTheme(once, "light-16", "color.active=black on yellow\n")
        assertEquals(1, Regex("color\\.active").findAll(twice).count())
        assertTrue("color.active=black on yellow" in twice)
    }

    @Test
    fun writesAndRemovesTheColourOffLine() {
        assertEquals("bulk=5\ncolor=off\n", withColour("bulk=5\n", false))
        assertEquals("bulk=5\n", withColour("bulk=5\ncolor=off\n", true))
        assertEquals("color=off\n", withColour("", false))
        assertEquals("color=off\n", withColour("color=off\n", false))
        // color.* lines are not the switch.
        assertEquals("color.active=red\n", withColour("color.active=red\n", true))
    }

    @Test
    fun readsWhetherColourIsOff() {
        assertTrue(colourOff("color=off\n"))
        assertTrue(colourOff("color = 0 # no\n"))
        assertFalse(colourOff("color=on\n"))
        assertFalse(colourOff("color.active=red\n"))
    }

    @Test
    fun recognisesTheBundledThemesByNameWithOrWithoutAPath() {
        assertEquals("dark-256", includedTheme("include dark-256.theme"))
        assertEquals("solarized-dark-256", includedTheme("  include /usr/share/doc/task/rc/solarized-dark-256.theme "))
        assertNull(includedTheme("include ~/mine.theme"))
        assertNull(includedTheme("include something.rc"))
        assertNull(includedTheme("color.active=red"))
    }

    @Test
    fun expandsOnlyThoseInPlace() = runBlocking {
        val out = expandIncludes("bulk=5\ninclude dark-256.theme\ninclude ~/mine.theme\nlimit=9") { id -> "color.active=$id" }
        assertEquals("bulk=5\n# theme: dark-256\ncolor.active=dark-256\ninclude ~/mine.theme\nlimit=9", out)
    }

    @Test
    fun theListHasTheAppThemeFirstAndEachThemeOnce() {
        assertEquals("meghnaad", THEMES[0].id)
        assertEquals(THEMES.size, THEMES.map { it.id }.toSet().size)
        assertEquals(17, THEMES.size)
    }

    @Test
    fun everyThemeTravelsWithTheApp() {
        for (t in THEMES) assertTrue(readResource("/themes/${t.id}.theme") != null, t.id)
        assertTrue(themeLines(loadTheme("dark-256")).any { it.startsWith("color") })
    }

    @Test
    fun searchesIconNamesAndTheWordsTheyAreFoundByNamesFirst() {
        val catalog = mapOf(
            "check" to listOf("done", "todo"),
            "circle-x" to listOf("cancel", "close"),
            "x" to listOf("close", "cancel"),
            "x-circle-ish" to listOf("other"),
        )
        assertEquals(listOf("check", "circle-x", "x", "x-circle-ish"), searchIcons(catalog, ""))
        assertEquals(listOf("circle-x", "x-circle-ish"), searchIcons(catalog, "circle x"))
        assertEquals(listOf("circle-x", "x"), searchIcons(catalog, "close"))
        assertEquals(listOf("circle-x", "x", "x-circle-ish"), searchIcons(catalog, "x"))
        assertEquals(listOf("check"), searchIcons(catalog, "done"))
        assertEquals(emptyList(), searchIcons(catalog, "zzz"))
    }
}
