package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

// Ported from web/src/lib/remindUda.test.ts.
class RemindUdaTest {
    private fun config(vararg udas: UdaDef) = ConfigResponse(config = ConfigBody(udas = udas.associateBy { it.name }))

    @Test
    fun writtenTheFirstTimeWithALabel() {
        assertEquals(Part.Missing, remindUdaStatus(config()))
        assertEquals(listOf("uda.remind_in.type" to "string", "uda.remind_in.label" to "Remind in"), remindUdaLines(config()))
        assertEquals(2, remindUdaLines(null).size)
    }

    @Test
    fun leftAloneWhenAStringOnlyItsTypeChangedOtherwise() {
        assertEquals(Part.Ok, remindUdaStatus(config(UdaDef("remind_in", "string"))))
        assertEquals(emptyList(), remindUdaLines(config(UdaDef("remind_in", "string"))))
        val other = config(UdaDef("remind_in", "numeric", label = "Mine"))
        assertEquals(Part.Different, remindUdaStatus(other))
        assertEquals(listOf("uda.remind_in.type" to "string"), remindUdaLines(other))
    }
}
