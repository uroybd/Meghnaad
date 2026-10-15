package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

// The same table as web/src/lib/route.test.ts.
class RouteTest {
    private val uuid = "1a2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d"

    @Test
    fun aTasksLinkIsTheFirst8CharactersOfItsUuid() {
        assertEquals("meghnaad://task/1a2b3c4d", taskLink(uuid))
    }

    @Test
    fun aLinkIsReadBackFromTheShortFormOrALongerPrefix() {
        assertEquals("1a2b3c4d", prefixFromLink("meghnaad://task/1a2b3c4d"))
        assertEquals("1a2b3c4d", prefixFromLink("meghnaad://task/1A2B3C4D/"))
        assertEquals(uuid, prefixFromLink("meghnaad://task/$uuid"))
        assertEquals("12345678", prefixFromLink("meghnaad://task/12345678"))
    }

    @Test
    fun aLinkIsNotReadFromAnythingElse() {
        for (p in listOf(
            "meghnaad://",
            "meghnaad://task",
            "meghnaad://task/",
            "meghnaad://task/zzzzzzzz",
            "meghnaad://task/1a2b3c",
            "meghnaad://tasks/1a2b3c4d",
            "meghnaad://task/1a2b3c4d/edit",
            "https://example.com/task/1a2b3c4d",
            "/task/1a2b3c4d",
        )) assertNull(prefixFromLink(p), p)
    }

    @Test
    fun aTaskIsLookedUpWithAnAttributeFilterSoDigitsOnlyPrefixesAreNotTaskNumbers() {
        assertEquals("uuid.startswith:12345678", prefixFilter("12345678"))
    }
}
