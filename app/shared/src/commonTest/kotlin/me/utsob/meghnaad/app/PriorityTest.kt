package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

class PriorityTest {
    private fun priority(vararg values: String) = mapOf("priority" to UdaDef(name = "priority", type = "string", label = "Priority", values = values.toList()))

    @Test
    fun areHMAndLUntilTheTaskrcSaysOtherwise() {
        assertEquals(listOf("H", "M", "L"), priorityValues(emptyMap()))
        assertEquals(listOf("H", "M", "L"), priorityValues(priority()))
    }

    @Test
    fun followUdaPriorityValuesWithoutTheBlankOne() {
        assertEquals(listOf("Critical", "Important"), priorityValues(priority("Critical", "Important", "")))
        assertEquals(listOf("H", "M", "L"), priorityValues(priority("H", "M", "", "L")))
    }

    @Test
    fun readAsASentence() {
        assertEquals("H, M or L", orList(listOf("H", "M", "L")))
        assertEquals("Critical or Important", orList(listOf("Critical", "Important")))
        assertEquals("Only", orList(listOf("Only")))
    }
}
