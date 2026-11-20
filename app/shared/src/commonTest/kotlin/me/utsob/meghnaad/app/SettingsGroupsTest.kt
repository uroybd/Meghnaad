package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

class SettingsGroupsTest {
    @Test
    fun everyTabIsInExactlyOneGroup() {
        assertEquals(SettingsTab.entries.toSet(), SETTINGS_GROUPS.flatMap { it.tabs }.toSet())
        assertEquals(SettingsTab.entries.size, SETTINGS_GROUPS.sumOf { it.tabs.size })
    }

    @Test
    fun theGroupsAreMeghnaadsAndTaskwarriorsAsOnTheWeb() {
        assertEquals(listOf("Meghnaad", "Taskwarrior"), SETTINGS_GROUPS.map { it.label })
        assertEquals(listOf(SettingsTab.Taskrc, SettingsTab.Urgency, SettingsTab.Hooks, SettingsTab.Tasksh), SETTINGS_GROUPS[1].tabs)
        assertEquals("Taskwarrior", settingsGroupOf(SettingsTab.Hooks).label)
        assertEquals("Meghnaad", settingsGroupOf(SettingsTab.Kanban).label)
        assertEquals("Meghnaad", settingsGroupOf(SettingsTab.Sync).label)
    }
}
