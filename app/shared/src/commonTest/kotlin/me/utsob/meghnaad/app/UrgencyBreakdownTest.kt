package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

// The same case as web/src/lib/urgencyBreakdown.test.ts. The numbers are the engine's (tc_core::info_text, checked there
// against the real `task info`); this checks the app carries them across.
class UrgencyBreakdownTest {
    @Test
    fun theTermsAreTheOnesTaskInfoPrints() {
        val task = Row(
            uuid = "11111111-2222-3333-4444-555555555555", status = "pending", description = "Pay rent",
            entry = 1_700_000_000L, tags = listOf("home"), priority = "H", urgency = 6.8,
        )
        val b = urgencyBreakdown(task, null, 1_700_000_000L, 0)
        val labels = b.terms.map { it[0] }
        assertTrue(labels.containsAll(listOf("tags", "UDA priority.H")), labels.toString())
        assertEquals(listOf("UDA priority.H", "1", "6", "6"), b.terms.first { it[0] == "UDA priority.H" })
        assertEquals("6.8", b.total)
    }
}
