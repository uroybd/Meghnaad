package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonElement
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

/** The Export button through the engine (web/src/lib/exportFile.test.ts): `tc_core::table_export` has the tests of what it means. */
class ExportFileTest {
    private val text = "[\n{\"id\":1,\"description\":\"Buy, milk\",\"tags\":[\"a\",\"b\"],\"uuid\":\"u1\"}\n]\n"
    private val raw: JsonElement = engineJson.parseToJsonElement(
        """{"udas":{},"reports":{},"contexts":{"home":{"name":"home","read":"project:Home","write":null,"rc":{}}},
           "active_context":"home","urgency":{},"settings":{}}""",
    )

    @Test
    fun knowsWhatATableExportsInThePagesContext() {
        assertEquals(listOf("(", "project:Home", ")", "+x", "export", "next"), exportArgs(CliInput(line = "+x next"), null, raw))
        assertEquals(listOf("export", "next"), exportArgs(CliInput(args = listOf("next")), "none", raw))
        assertNull(exportArgs(CliInput(line = "calendar"), null, raw))
        assertNull(exportArgs(CliInput(line = "next"), null, null))
    }

    @Test
    fun shapesTheFileAsJsonCsvOrTsv() {
        assertEquals(listOf("id", "description", "tags", "uuid"), exportFields(text))
        assertEquals(listOf("id", "due"), tableFields(listOf("id", "due.relative")))
        val csv = renderExport("tasks-next.json", text, ExportChoice(ExportFormat.Csv, fields = ExportFields.Custom, picked = listOf("description", "tags")), emptyList(), raw)
        assertEquals(FileResult("tasks-next.csv", "text/csv", "description,tags\r\n\"Buy, milk\",a b\r\n"), csv)
        assertEquals(text, renderExport("x.json", text, ExportChoice(), emptyList(), raw).text)
        val dated = "[\n{\"description\":\"two\\nlines\",\"due\":\"20261119T180000Z\",\"uuid\":\"u1\"}\n]\n"
        val tsv = renderExport("tasks.json", dated, ExportChoice(ExportFormat.Tsv), emptyList(), raw)
        assertEquals(FileResult("tasks.tsv", "text/tab-separated-values", "description\tdue\tuuid\ntwo\\nlines\t2026-11-19 18:00:00\tu1\n"), tsv)
        assertEquals(true, jsonArrayOf(raw))
    }

    @Test
    fun theLastChoiceIsKeptAndReadBack() {
        val c = ExportChoice(ExportFormat.Tsv, array = false, fields = ExportFields.Custom, picked = listOf("due"))
        assertEquals(c.copy(array = null), rememberedExport(keptExport(c)))
        assertEquals(ExportChoice(), rememberedExport("not json"))
        assertEquals(ExportChoice(), rememberedExport(null))
        assertEquals(2, exportFieldCount(c.copy(picked = listOf("due", "id")), listOf("id", "due", "uuid"), emptyList()))
    }
}
