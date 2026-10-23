package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

class RcPathsTest {
    @Test
    fun theIncludeLineIsWrittenFromTheFolderOfTheFileThatHoldsIt() {
        assertEquals("include holidays/en-US.rc", includeLine("taskrc", "holidays/en-US.rc"))
        assertEquals("include extra.rc", includeLine("holidays/en-US.rc", "holidays/extra.rc"))
        assertEquals("include ../taskrc", includeLine("holidays/en-US.rc", "taskrc"))
        assertEquals("include ../x/y.rc", includeLine("a/b/c.rc", "a/x/y.rc"))
        assertEquals("include my notes.rc", includeLine("taskrc", "my notes.rc"))
    }

    @Test
    fun anIncludeGoesOnALineOfItsOwnOnce() {
        assertEquals("bulk=3\ninclude a.rc\n", withInclude("bulk=3", "taskrc", "a.rc"))
        assertEquals("bulk=3\ninclude a.rc\n", withInclude("bulk=3\n", "taskrc", "a.rc"))
        assertEquals("include a.rc\n", withInclude("", "taskrc", "a.rc"))
        assertEquals("include a.rc\r\nbulk=3", withInclude("include a.rc\r\nbulk=3", "taskrc", "a.rc"))
    }

    @Test
    fun anUploadLosesTheFolderThatWasPickedAndDotTaskrcIsTaskrc() {
        val r = uploadFiles(listOf(
            Picked("en-US.rc", "dots/holidays/en-US.rc", "holiday.a.name=A\n"),
            Picked(".taskrc", "dots/.taskrc", "bulk=3\n"),
        ))
        assertEquals(listOf("taskrc", "holidays/en-US.rc"), r.files.map { it.first })
        assertEquals(emptyList(), r.skipped)
        assertEquals("a.rc", uploadFiles(listOf(Picked("a.rc", null, "x"))).files.single().first)
    }

    @Test
    fun anUploadLeavesOutWhatIsNotATaskrcFileAndSaysSo() {
        val r = uploadFiles(listOf(
            Picked("HEAD", "dots/.git/HEAD", "ref"),
            Picked(".DS_Store", "dots/.DS_Store", "x"),
            Picked("logo.png", "dots/logo.png", "a\u0000b"),
            Picked("big.rc", "dots/big.rc", null),
            Picked("ok.rc", "dots/ok.rc", "bulk=1\n"),
            Picked("ok.rc", "dots/ok.rc", "bulk=2\n"),
        ))
        assertEquals(listOf("ok.rc"), r.files.map { it.first })
        assertEquals(
            listOf("dots/.git/HEAD (hidden)", "dots/.DS_Store (hidden)", "dots/logo.png (not text)", "dots/big.rc (too large)", "dots/ok.rc (the same name as another)"),
            r.skipped,
        )
    }

    @Test
    fun anUploadStopsAtTheMostAFolderHolds() {
        val r = uploadFiles((0 until 70).map { Picked("f$it.rc", null, "bulk=1\n") })
        assertEquals(64, r.files.size)
        assertEquals(6, r.skipped.size)
    }

    @Test
    fun theListPutsTaskrcFirstAndNamesEachFolderAboveItsFiles() {
        assertEquals(
            listOf(
                RcRow("taskrc", 0, "taskrc", null),
                RcRow("a.rc", 0, "a.rc", null),
                RcRow("", 0, "holidays", "holidays"),
                RcRow("holidays/de.rc", 1, "de.rc", "holidays"),
                RcRow("holidays/en-US.rc", 1, "en-US.rc", "holidays"),
            ),
            listing(listOf("holidays/en-US.rc", "taskrc", "a.rc", "holidays/de.rc")),
        )
    }
}
