package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class SuggestTest {
    private val projects = listOf("Home", "Home.Kitchen", "Work", "Work.Reports", "Garden")

    private fun values(names: List<String>, typed: String, taken: Set<String> = emptySet()) = suggest(names, typed, taken).map { it.value }

    @Test
    fun wordAtIsTheWholeFieldForASingleValue() {
        assertEquals(Word(0, 7, "Home.Ki"), wordAt("Home.Ki", 3, false))
    }

    @Test
    fun wordAtIsTheWordAroundTheCaretInAListWithoutALeadingPlus() {
        assertEquals(Word(7, 9, "wo"), wordAt("errand wo", 9, true))
        assertEquals(Word(8, 10, "wo"), wordAt("errand +wo", 10, true))
        assertEquals(Word(0, 6, "errand"), wordAt("errand work", 3, true))
        assertEquals(Word(2, 3, "b"), wordAt("a,b", 3, true))
    }

    @Test
    fun wordAtIsEmptyAfterASpace() {
        assertEquals(Word(7, 7, ""), wordAt("errand ", 7, true))
        assertEquals(Word(0, 0, ""), wordAt("", 0, true))
    }

    @Test
    fun offersNamesThatBeginWithWhatIsTypedThenNamesThatContainItIgnoringCase() {
        assertEquals(listOf("Home", "Home.Kitchen"), values(projects, "ho"))
        assertEquals(listOf("Home.Kitchen"), values(projects, "kit"))
        // Nothing begins with it; these contain it.
        assertEquals(listOf("Work", "Work.Reports", "Garden"), values(projects, "r"))
    }

    @Test
    fun offersAFewNamesForNothingTypedAndNoneAListAlreadyHas() {
        assertEquals(5, values(projects, "").size)
        assertEquals(listOf("a", "c"), values(listOf("a", "b", "c"), "", setOf("b")))
        assertEquals(8, values(List(30) { "t$it" }, "").size)
    }

    @Test
    fun hasNothingToOfferForANewNameOrWhenWhatIsTypedIsAllThereIs() {
        assertEquals(emptyList(), values(projects, "Nowhere"))
        assertEquals(emptyList(), values(projects, "Garden"))
        assertEquals(listOf("Home", "Home.Kitchen"), values(projects, "Home"))
    }

    @Test
    fun chooseReplacesASingleValue() {
        val r = choose("Ho", wordAt("Ho", 2, false), "Home.Kitchen", false)
        assertEquals("Home.Kitchen" to 12, r.value to r.caret)
    }

    @Test
    fun chooseReplacesTheWordInAListAndLeavesASpaceForTheNext() {
        val v = "errand wo"
        val r = choose(v, wordAt(v, 9, true), "work", true)
        assertEquals("errand work " to 12, r.value to r.caret)
        val mid = "er wo"
        val m = choose(mid, wordAt(mid, 2, true), "errand", true)
        assertEquals("errand wo" to 7, m.value to m.caret)
    }

    @Test
    fun chooseKeepsAPlusTheUserTyped() {
        val v = "+wo"
        val r = choose(v, wordAt(v, 3, true), "work", true)
        assertEquals("+work " to 6, r.value to r.caret)
    }

    @Test
    fun isNewIsTrueForANameNobodyHasUsed() {
        assertTrue(isNew(projects, "Taxes"))
        assertFalse(isNew(projects, "Home"))
        assertFalse(isNew(projects, "  "))
    }
}
