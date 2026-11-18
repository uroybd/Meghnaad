package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

// Spec 0016: the short text and the JSON a task is shared as. (`task info`'s text is the engine's, tested in tc-core; the
// app's part of it, against the real engine, is in ShareStoreTest.)
class ShareTest {
    private val utc: (Long) -> Int = { 0 }

    private fun row(
        description: String = "Buy milk",
        project: String? = null,
        priority: String? = null,
        due: Long? = null,
        tags: List<String> = emptyList(),
    ) = Row(uuid = "u", status = "pending", description = description, project = project, priority = priority, due = due, tags = tags)

    @Test
    fun theShortTextIsTheDescriptionThenItsAttributesAsTheQuickAddReadsThem() {
        val r = row(project = "home", priority = "H", due = 1_795_179_600L, tags = listOf("errand", "shop"))
        assertEquals("Buy milk\nproject:home priority:H due:2026-11-20T13:00 +errand +shop", shortShare(r, utc))
    }

    @Test
    fun aDueDateAtMidnightIsTheDayAloneInThePhonesZone() {
        // 2026-11-19T18:00Z is midnight six hours east of UTC.
        assertEquals("Buy milk\ndue:2026-11-20", shortShare(row(due = 1_795_111_200L), { 21_600 }))
    }

    @Test
    fun aTaskWithNothingButADescriptionIsTheDescriptionAlone() {
        assertEquals("Buy milk", shortShare(row(), utc))
        assertEquals("Buy milk\n+errand", shortShare(row(tags = listOf("errand")), utc))
    }

    @Test
    fun aLongDescriptionStaysOnItsOwnLineWhole() {
        val long = "Read the whole of the quarterly report and write up what it means for the team, with numbers"
        assertEquals("$long\npriority:L", shortShare(row(description = long, priority = "L"), utc))
    }

    @Test
    fun aProjectWithSpacesIsQuotedSoItReadsBackWhole() {
        assertEquals("Buy milk\nproject:'big house'", shortShare(row(project = "big house"), utc))
    }

    @Test
    fun theShortTextReadsBackIntoTheSameFields() {
        val text = shortShare(row(project = "home", priority = "H", due = 1_795_179_600L, tags = listOf("errand")), utc)
        val p = previewAdd(text.replace("\n", " "), null, 1_795_000_000L, 0)
        assertEquals("Buy milk", p.description)
        assertEquals("home", p.project)
        assertEquals("H", p.priority)
        assertEquals(1_795_179_600L, p.dates["due"])
        assertEquals(listOf("errand"), p.tags)
        assertTrue(p.errors.isEmpty(), "${p.errors}")
    }

    @Test
    fun theJsonIsTheOneObjectExportWritesSetOutOnLines() {
        val export = """[
{"id":2,"description":"Buy milk","status":"pending","uuid":"8f3f4cbb-c9b2-4c8e-beca-9583d257635e","urgency":28.1678}
]"""
        assertEquals(
            """{
    "id": 2,
    "description": "Buy milk",
    "status": "pending",
    "uuid": "8f3f4cbb-c9b2-4c8e-beca-9583d257635e",
    "urgency": 28.1678
}""",
            taskJsonOf(export),
        )
        // `json.array=off` writes a line of JSON for each task.
        assertEquals(taskJsonOf(export), taskJsonOf("""{"id":2,"description":"Buy milk","status":"pending","uuid":"8f3f4cbb-c9b2-4c8e-beca-9583d257635e","urgency":28.1678}"""))
        assertNull(taskJsonOf("[]"))
    }
}
