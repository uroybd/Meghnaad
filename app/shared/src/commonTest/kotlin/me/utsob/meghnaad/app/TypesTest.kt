package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertIs
import kotlin.test.assertNull
import kotlin.test.assertTrue

class TypesTest {
    private val report = """
        {"command":{"filter":[],"name":"next","report":true},"feedback":[{"kind":"info","text":"hello"}],
         "result":{"kind":"report","report":"next","description":"Most urgent tasks","matched":1,"sort":"urgency-",
           "breaks":[false],
           "columns":[{"format":null,"kind":"id","label":"ID","name":"id","spec":"id"},
                      {"format":"age","kind":"date","label":"Age","name":"entry","spec":"entry.age"}],
           "rows":[{"uuid":"09d10420-fb47-4a69-b659-ebf4b360e4dd","status":"pending","description":"task 2",
                    "project":"spike","tags":["t2"],"due":1792087200,"entry":1792002406,"id":1,"urgency":10.15,
                    "style":{"fg":7},"extra":{},"annotations":[],"depends":[],"blocked":false,"blocking":false,
                    "orphans":[],"sessions":[],"active_seconds":null,"future_field":true}]},
         "wrote":false}
    """

    @Test
    fun aReportIsRead() {
        val got = parseResponse(report)
        val result = assertIs<CliResult.Report>(got.result).value
        assertEquals("next", result.report)
        assertEquals(2, result.columns.size)
        assertEquals("date", result.columns[1].kind)
        val row = result.rows.single()
        assertEquals("task 2", row.description)
        assertEquals(listOf("t2"), row.tags)
        assertEquals(1792087200L, row.due)
        assertEquals(1, row.id)
        assertEquals(7, row.style?.fg)
        assertNull(row.project?.takeIf { it != "spike" })
        assertEquals("next", got.command?.name)
        assertEquals("hello", got.feedback.single().text)
        assertTrue(!got.wrote)
    }

    @Test
    fun aRefusedRequestIsAnError() {
        val e = assertFailsWith<EngineError> { parseResponse("""{"error":"invalid request: no"}""") }
        assertEquals("invalid request: no", e.message)
    }

    @Test
    fun aConfirmationCarriesItsQuestions() {
        val got = parseResponse(
            """{"command":null,"wrote":false,"result":{"kind":"confirm","message":"Delete 1 task?","ask":"permission",
               "items":[{"key":"k1","uuid":"u","id":3,"description":"x","question":"Delete?"}]}}""",
        )
        val q = assertIs<CliResult.Confirm>(got.result).value
        assertEquals("permission", q.ask)
        assertEquals("k1", q.items.single().key)
        assertNull(got.command)
    }

    @Test
    fun aKindWithoutAScreenIsKept() {
        val got = parseResponse("""{"command":null,"wrote":false,"result":{"kind":"styled","lines":[]}}""")
        assertEquals("styled", assertIs<CliResult.Unsupported>(got.result).kind)
    }

    @Test
    fun datesAreInTheUsersZone() {
        assertEquals("2026-10-16", isoDate(1792087200, 21600))
        assertEquals("1970-01-01", isoDate(0, 0))
        assertEquals("2000-02-29", isoDate(951782400, 0))
        assertEquals("2024-02-28", isoDate(1709164800 - 1, 0))
        assertEquals("2024-02-29", isoDate(1709164800, 0))
        assertEquals("1969-12-31", isoDate(0, -3600))
    }
}
