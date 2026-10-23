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

    @Test
    fun everyPageHasALinkThatIsReadBackTheSame() {
        val places = listOf(
            AppPlace.Report("next"),
            AppPlace.Report("_reviewed"),
            AppPlace.Report("my report/2 é"),
            AppPlace.Page(View.Projects),
            AppPlace.Page(View.Tags),
            AppPlace.Page(View.Summary),
            AppPlace.Page(View.Calendar),
            AppPlace.Page(View.Burndown),
            AppPlace.Page(View.Console),
            AppPlace.Boards(),
            AppPlace.Boards("q4-review_2"),
            AppPlace.SettingsAt(),
            AppPlace.SettingsAt(SettingsTab.Hooks),
            AppPlace.SettingsAt(SettingsTab.Taskrc, "taskrc"),
            AppPlace.SettingsAt(SettingsTab.Taskrc, "holidays/en-US.rc"),
            AppPlace.SettingsAt(SettingsTab.Taskrc, "my notes/a b%.rc"),
            AppPlace.TaskAt("1a2b3c4d"),
        )
        for (p in places) assertEquals(p, parseLink(placeLink(p)), placeLink(p))
        assertEquals("meghnaad://report/next", placeLink(AppPlace.Report("next")))
        assertEquals("meghnaad://report/a%20b%2Fc", placeLink(AppPlace.Report("a b/c")))
        assertEquals("meghnaad://board/week", placeLink(AppPlace.Boards("week")))
        assertEquals("meghnaad://settings/urgency", placeLink(AppPlace.SettingsAt(SettingsTab.Urgency)))
        assertEquals("meghnaad://settings/taskrc/holidays/en-US.rc", placeLink(AppPlace.SettingsAt(SettingsTab.Taskrc, "holidays/en-US.rc")))
        assertEquals("meghnaad://settings/taskrc/my%20notes.rc", placeLink(AppPlace.SettingsAt(SettingsTab.Taskrc, "my notes.rc")))
        assertEquals(AppPlace.Report("list"), parseLink("meghnaad://report/list/"))
    }

    @Test
    fun aLinkThatNamesNothingIsNothing() {
        for (bad in listOf(
            "", "https://example.com", "meghnaad://", "meghnaad://nope", "meghnaad://report", "meghnaad://report/%E0%A4%A",
            "meghnaad://board/Weekly", "meghnaad://board/a b", "meghnaad://board/a/b", "meghnaad://boards/x", "meghnaad://settings/nope",
            "meghnaad://settings/taskrc/../x", "meghnaad://settings/taskrc/a//b", "meghnaad://settings/taskrc/a/./b", "meghnaad://settings/taskrc/%2E%2E/x",
            "meghnaad://settings/taskrc/%E0%A4%A", "meghnaad://settings/hooks/x",
            "meghnaad://projects/x", "meghnaad://task/zzzzzzzz", "meghnaad://task/1a2b3c",
        )) {
            assertEquals(null, parseLink(bad), bad)
        }
    }

    @Test
    fun anEscapedPartIsReadAsUtf8() {
        assertEquals("é b", percentDecode("%C3%A9%20b"))
        assertEquals("a b", percentDecode("a b"))
        assertEquals(null, percentDecode("%zz"))
        assertEquals("%C3%A9%20b", percentEncode("é b"))
    }
}
