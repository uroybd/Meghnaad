package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ScrollableTabRow
import androidx.compose.material3.Tab
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** What every tab of Settings keeps while another is shown (the web keeps them mounted, hidden). */
class SettingsState(store: Store) {
    val taskrc = TaskrcState(store)
    val commands = CommandsState(store)
    val urgency = UrgencyState(store)
    val tasksh = TaskshState(store)
    val hooks = HooksState(store)
}

private class SettingsTabInfo(val tab: SettingsTab, val label: String)

private val TABS = listOf(
    SettingsTabInfo(SettingsTab.Taskrc, "taskrc"),
    SettingsTabInfo(SettingsTab.Tasksh, "tasksh"),
    SettingsTabInfo(SettingsTab.Commands, "Commands"),
    SettingsTabInfo(SettingsTab.Hooks, "Hooks"),
    SettingsTabInfo(SettingsTab.Urgency, "Urgency"),
    SettingsTabInfo(SettingsTab.Sync, "Sync"),
)

/** One page with a tab each for the taskrc, tasksh, commands, hooks, urgency and sync; each has its own Apply (SettingsPage.svelte). */
@Composable
fun SettingsPage(store: Store, state: SettingsState) {
    val dirty = mapOf(SettingsTab.Taskrc to state.taskrc.edited, SettingsTab.Commands to state.commands.edited, SettingsTab.Urgency to state.urgency.edited, SettingsTab.Tasksh to state.tasksh.edited, SettingsTab.Hooks to state.hooks.edited)
    Column(Modifier.fillMaxSize()) {
        Text("Settings", Modifier.padding(start = 16.dp, top = 10.dp), fontSize = 20.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
        ScrollableTabRow(selectedTabIndex = TABS.indexOfFirst { it.tab == store.settingsTab }.coerceAtLeast(0), edgePadding = 8.dp, containerColor = MaterialTheme.colorScheme.background) {
            for (t in TABS) {
                Tab(selected = store.settingsTab == t.tab, onClick = { store.settingsTab = t.tab }) {
                    Row(Modifier.padding(horizontal = 12.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
                        Text(t.label, fontSize = 14.sp)
                        // Not applied yet.
                        if (dirty[t.tab] == true) Box(Modifier.padding(start = 6.dp).size(7.dp).background(MaterialTheme.colorScheme.primary, CircleShape))
                    }
                }
            }
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            when (store.settingsTab) {
                SettingsTab.Taskrc -> TaskrcPanel(store, state.taskrc)
                SettingsTab.Commands -> CommandsPanel(store, state.commands)
                SettingsTab.Tasksh -> TaskshPanel(store, state.tasksh)
                SettingsTab.Hooks -> HooksPanel(store, state.hooks)
                SettingsTab.Urgency -> UrgencyPanel(store, state.urgency)
                SettingsTab.Sync -> SyncPanel(store)
            }
            if (APP_VERSION.isNotEmpty()) Dim("Meghnaad $APP_VERSION", Modifier.padding(vertical = 16.dp), 12)
        }
    }
}

/** The app's version, set by the platform that builds it. */
var APP_VERSION: String = ""
