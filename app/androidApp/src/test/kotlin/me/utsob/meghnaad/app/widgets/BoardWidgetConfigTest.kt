package me.utsob.meghnaad

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class BoardWidgetConfigTest {
    @Test
    fun aBoardWidgetNamesABoardAndACard() {
        val c = BoardWidgetConfig("week", "done")
        assertTrue(c.chosen)
        assertEquals(c, BoardWidgetConfig.fromJson(c.toJson()))
        assertEquals("week", c.toJson().getString("board"))
    }

    @Test
    fun aWidgetThatWasNeverSetUpHasNothingChosen() {
        assertFalse(BoardWidgetConfig().chosen)
        assertFalse(BoardWidgetConfig("week", "").chosen)
        assertFalse(BoardWidgetConfig("", "done").chosen)
        assertEquals(BoardWidgetConfig(), BoardWidgetConfig.fromJson(JSONObject()))
    }
}
