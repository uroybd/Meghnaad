package me.utsob.meghnaad

import org.junit.Assert.assertEquals
import org.junit.Test

class ListConfigTest {
    @Test
    fun aWidgetIsCalledByItsTitleThenByWhatItsReportIsDescribedAsThenByTheReport() {
        assertEquals("Work", ListConfig(report = "next", title = "Work", description = "Most urgent tasks").heading())
        assertEquals("Most urgent tasks", ListConfig(report = "next", description = "Most urgent tasks").heading())
        assertEquals("next", ListConfig(report = "next").heading())
        // A title of spaces is no title; a report with no description keeps its name.
        assertEquals("Most urgent tasks", ListConfig(report = "next", title = "  ", description = "Most urgent tasks").heading())
        assertEquals("mine", ListConfig(report = "mine", description = " ").heading())
    }

    @Test
    fun theDescriptionIsKeptWithTheRestAndWidgetsSetUpBeforeItWasKeptStillLoad() {
        val config = ListConfig(report = "next", filter = "+work", limit = 5, title = "", description = "Most urgent tasks")
        assertEquals(config, ListConfig.fromJson(config.toJson()))
        // What an earlier version saved has no description.
        val old = ListConfig(report = "next", title = "T").toJson().apply { remove("description") }
        assertEquals("", ListConfig.fromJson(old).description)
        assertEquals("T", ListConfig.fromJson(old).heading())
    }
}
