package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull

/** The engine's Markdown as the app reads it: the parser is `tc_core::markdown` (tested there), so these check the hand-over. */
class MarkdownTest {
    @Test
    fun aTextThatUsesMarkdownComesBackAsATreeAndPlainWordsDoNot() {
        val (rich, plain) = markdownTrees(listOf("- [ ] milk\n- [x] **eggs**", "just words\non two lines"))
        assertNull(plain)
        val list = rich!!.single()
        assertEquals("ul", list.t)
        assertEquals(listOf(false, true), list.c.map { it.done })
        assertEquals(listOf(0, 1), list.c.map { it.line })
        assertEquals("b", list.c[1].c.single().c.single { it.t == "b" }.t)
        assertEquals(emptyList(), markdownTrees(emptyList()))
    }

    @Test
    fun linksCarryTheirAddressAndOnlySafeOnesAreMade() {
        val (tree) = markdownTrees(listOf("[docs](https://x.example) and [bad](javascript:alert(1))"))
        val inlines = tree!!.single().c
        assertEquals("https://x.example", inlines.single { it.t == "a" }.href)
        assertEquals(1, inlines.count { it.t == "a" })
    }

    @Test
    fun aBoxIsFlippedOnItsLineAndNowhereElse() {
        assertEquals("a\n- [x] b\n- [x] c", toggledBox("a\n- [ ] b\n- [x] c", 1))
        assertEquals("a\n- [ ] b\n- [ ] c", toggledBox("a\n- [ ] b\n- [x] c", 2))
        assertNull(toggledBox("a\n- [ ] b", 0))
        assertNull(toggledBox("```\n- [ ] in code\n```", 1))
    }

    @Test
    fun aCellOfAStringUdaCarriesItsTree() {
        val udas = mapOf("plan" to UdaDef(name = "plan", type = "string"))
        val ctx = Ctx(now = 1_791_376_200L, tz = 0, udas = udas)
        val row = Row(uuid = "u", status = "pending", description = "t", extra = mapOf("plan" to "- [ ] a"))
        val col = Column(spec = "plan", name = "plan", format = null, label = "plan", kind = "string")
        val cell = formatCells(listOf(col), listOf(row), ctx)[0][0]
        assertEquals("- [ ] a", cell.text)
        assertNotNull(cell.blocks)
        val noted = row.copy(annotations = listOf(Note(5, "**x**"), Note(6, "plain")))
        val d = formatCells(listOf(Column("description", "description", null, "d", "description")), listOf(noted), ctx)[0][0]
        assertEquals(listOf(true, false), d.notes!!.map { it.blocks != null })
    }
}
