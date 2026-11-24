package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

class ContextLabelTest {
    @Test
    fun noContextIsNa() {
        assertEquals("NA", contextLabel(null, listOf("work")))
    }

    @Test
    fun aNameOfWordsIsItsInitialsAndOneWordItsFirstTwoLetters() {
        val all = listOf("this_month", "this_week", "work", "deep-focus", "on.call", "x")
        assertEquals("TM", contextLabel("this_month", all))
        assertEquals("TW", contextLabel("this_week", all))
        assertEquals("WO", contextLabel("work", all))
        assertEquals("DF", contextLabel("deep-focus", all))
        assertEquals("OC", contextLabel("on.call", all))
        assertEquals("X", contextLabel("x", all))
    }

    @Test
    fun namesWithTheSameInitialsGetTheShortestPrefixThatTellsThemApart() {
        val all = listOf("this_week", "two_weeks", "work", "world")
        assertEquals("THI", contextLabel("this_week", all))
        assertEquals("TWO", contextLabel("two_weeks", all))
        assertEquals("WORK", contextLabel("work", all))
        assertEquals("WORL", contextLabel("world", all))
    }

    @Test
    fun namesNoFourLettersTellApartKeepTheirInitials() {
        val all = listOf("this_week", "this_weekend")
        assertEquals("TW", contextLabel("this_week", all))
        assertEquals("TW", contextLabel("this_weekend", all))
    }

    @Test
    fun theActiveContextOutsideTheListIsShortenedAlone() {
        assertEquals("HO", contextLabel("home", emptyList()))
    }
}
