package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.ui.draw.clip
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.material3.TabRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** What every tab of Settings keeps while another is shown (the web keeps them mounted, hidden). */
class SettingsState(store: Store) {
    val general = GeneralState(store)
    val deviceGeneral = GeneralState(store, device = true)
    val deviceKeys = KeysState(store, device = true)
    val appearance = AppearanceState(store)
    val keys = KeysState(store)
    val taskrc = TaskrcState(store)
    val commands = CommandsState(store)
    val urgency = UrgencyState(store)
    val tasksh = TaskshState(store)
    val hooks = HooksState(store)
    val kanban = KanbanState(store)
}

private class SettingsTabInfo(val tab: SettingsTab, val label: String)

private val TABS = listOf(
    SettingsTabInfo(SettingsTab.General, "General"),
    SettingsTabInfo(SettingsTab.Device, "This device"),
    SettingsTabInfo(SettingsTab.Appearance, "Appearance"),
    SettingsTabInfo(SettingsTab.Keys, "Keybindings"),
    SettingsTabInfo(SettingsTab.Taskrc, "taskrc"),
    SettingsTabInfo(SettingsTab.Tasksh, "tasksh"),
    SettingsTabInfo(SettingsTab.Kanban, "Kanban"),
    SettingsTabInfo(SettingsTab.Commands, "Commands"),
    SettingsTabInfo(SettingsTab.Hooks, "Hooks"),
    SettingsTabInfo(SettingsTab.Urgency, "Urgency"),
    SettingsTabInfo(SettingsTab.Sync, "Sync"),
).associateBy { it.tab }

/** Settings in two groups, as on the web (SettingsPage.svelte): the app's own, and what Taskwarrior itself reads. */
class SettingsGroup(val label: String, val tabs: List<SettingsTab>)

val SETTINGS_GROUPS = listOf(
    SettingsGroup("Meghnaad", listOf(SettingsTab.General, SettingsTab.Device, SettingsTab.Appearance, SettingsTab.Keys, SettingsTab.Kanban, SettingsTab.Commands, SettingsTab.Sync)),
    SettingsGroup("Taskwarrior", listOf(SettingsTab.Taskrc, SettingsTab.Urgency, SettingsTab.Hooks, SettingsTab.Tasksh)),
)

/** The group that holds a tab: an address names only the tab. */
fun settingsGroupOf(tab: SettingsTab): SettingsGroup = SETTINGS_GROUPS.firstOrNull { tab in it.tabs } ?: SETTINGS_GROUPS[0]

@Composable
private fun DirtyLabel(label: String, dirty: Boolean) {
    Row(Modifier.padding(horizontal = 12.dp, vertical = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        Text(label, fontSize = 14.sp)
        // Not applied yet.
        if (dirty) Box(Modifier.padding(start = 6.dp).size(7.dp).background(MaterialTheme.colorScheme.primary, CircleShape))
    }
}

/** Two groups, Meghnaad's and Taskwarrior's, each with its tabs in a row; each tab has its own Apply (SettingsPage.svelte). */
@Composable
fun SettingsPage(store: Store, state: SettingsState) {
    // Every tab's draft follows what is saved, not only the one shown: one not opened yet was made before the settings
    // were read, and would otherwise look edited (its dot) against them.
    state.general.follow()
    state.deviceGeneral.follow()
    state.keys.follow()
    state.deviceKeys.follow()
    state.appearance.follow()
    state.commands.follow()
    state.kanban.follow()
    state.tasksh.follow()
    state.urgency.follow()
    val dirty = mapOf(SettingsTab.General to state.general.edited, SettingsTab.Device to (state.deviceGeneral.edited || state.deviceKeys.edited), SettingsTab.Appearance to state.appearance.edited, SettingsTab.Keys to state.keys.edited, SettingsTab.Taskrc to state.taskrc.edited, SettingsTab.Commands to state.commands.edited, SettingsTab.Urgency to state.urgency.edited, SettingsTab.Tasksh to state.tasksh.edited, SettingsTab.Hooks to state.hooks.edited, SettingsTab.Kanban to state.kanban.edited)
    val group = settingsGroupOf(store.settingsTab)
    // The tab last shown in each group, so going back to a group goes back to it.
    val last = remember { mutableStateMapOf<String, SettingsTab>() }
    LaunchedEffect(store.settingsTab) { last[group.label] = store.settingsTab }
    // The first time This device is opened, other devices' settings are offered (spec 0027).
    var offered by remember { mutableStateOf(false) }
    LaunchedEffect(store.settingsTab) {
        if (store.settingsTab == SettingsTab.Device && !offered) {
            offered = true
            offerDevices(store)
        }
    }
    BoxWithConstraints(Modifier.fillMaxSize()) {
    // A phone: one menu of the groups, each folding open to its tabs, in the title's place; the two rows of tabs took
    // the height the taskrc's editor needs.
    val narrow = maxWidth < 600.dp
    Column(Modifier.fillMaxSize()) {
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            if (narrow) {
                Box(Modifier.weight(1f)) { SettingsMenu(store, group, dirty) }
            } else {
                Text("Settings", Modifier.weight(1f), fontSize = 20.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
            }
        }
        DevicePrompt(store)
        if (!narrow) {
        TabRow(selectedTabIndex = SETTINGS_GROUPS.indexOf(group), Modifier.widthIn(max = 420.dp), containerColor = MaterialTheme.colorScheme.background) {
            for (g in SETTINGS_GROUPS) {
                Tab(selected = g === group, onClick = { store.settingsTab = last[g.label] ?: g.tabs.first() }) {
                    DirtyLabel(g.label, g.tabs.any { dirty[it] == true })
                }
            }
        }
        ScrollableTabRow(selectedTabIndex = group.tabs.indexOf(store.settingsTab).coerceAtLeast(0), edgePadding = 8.dp, containerColor = MaterialTheme.colorScheme.background) {
            for (t in group.tabs) {
                Tab(selected = store.settingsTab == t, onClick = { store.settingsTab = t }) {
                    DirtyLabel(TABS[t]?.label ?: t.name, dirty[t] == true)
                }
            }
        }
        }
        // The taskrc tab is a workspace that takes the height (its editor scrolls inside it, and Apply stays in view); the others scroll.
        if (store.settingsTab == SettingsTab.Taskrc) {
            Box(Modifier.weight(1f).widthIn(max = 1100.dp).padding(horizontal = 16.dp).padding(bottom = 72.dp)) { TaskrcPanel(store, state.taskrc) }
            return@Column
        }
        Column(Modifier.weight(1f).widthIn(max = 820.dp).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            when (store.settingsTab) {
                SettingsTab.General -> GeneralPanel(store, state.general)
                SettingsTab.Device -> ThisDevicePanel(store, state)
                SettingsTab.Appearance -> AppearancePanel(store, state.appearance)
                SettingsTab.Keys -> KeysPanel(store, state.keys)
                SettingsTab.Taskrc -> TaskrcPanel(store, state.taskrc)
                SettingsTab.Commands -> CommandsPanel(store, state.commands)
                SettingsTab.Tasksh -> TaskshPanel(store, state.tasksh)
                SettingsTab.Kanban -> KanbanPanel(store, state.kanban)
                SettingsTab.Hooks -> HooksPanel(store, state.hooks)
                SettingsTab.Urgency -> UrgencyPanel(store, state.urgency)
                SettingsTab.Sync -> SyncPanel(store)
            }
            UpdateRow(store.updates)
            if (store.settingsTab == SettingsTab.General) SettingsFilePanel(store, state.general)
            if (APP_VERSION.isNotEmpty()) Dim("Meghnaad $APP_VERSION", Modifier.padding(vertical = 16.dp), 12)
            // Room under the last control, so the floating + never covers it (the web's 72 px under every page).
            Box(Modifier.height(72.dp))
        }
    }
    }
}

/**
 * The settings' place on a phone, as one button: the group over the tab. It opens a menu of the groups, each folding
 * open to its tabs (the one shown open at first), the tab shown ticked and one not applied yet marked with a dot.
 */
@Composable
private fun SettingsMenu(store: Store, group: SettingsGroup, dirty: Map<SettingsTab, Boolean>) {
    val colors = MaterialTheme.colorScheme
    var open by remember { mutableStateOf(false) }
    val unfolded = remember(open) { mutableStateMapOf(group.label to true) }
    Row(
        Modifier.clip(RoundedCornerShape(8.dp)).clickable { open = true }.padding(horizontal = 4.dp, vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Column {
            Text(group.label, fontSize = 11.sp, color = colors.onSurfaceVariant)
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(TABS[store.settingsTab]?.label ?: store.settingsTab.name, fontSize = 17.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
                if (dirty.any { it.value }) Box(Modifier.padding(start = 6.dp).size(7.dp).background(colors.primary, CircleShape))
            }
        }
        LucideIcon("chevron-down", size = 16.dp)
    }
    DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
        for (g in SETTINGS_GROUPS) {
            val shown = unfolded[g.label] == true
            DropdownMenuItem(
                text = { DirtyLabel(g.label, g.tabs.any { dirty[it] == true }) },
                trailingIcon = { LucideIcon(if (shown) "chevron-down" else "chevron-right", size = 16.dp) },
                onClick = { unfolded[g.label] = !shown },
            )
            if (shown) {
                for (t in g.tabs) {
                    DropdownMenuItem(
                        text = { Text(TABS[t]?.label ?: t.name, fontSize = 14.sp, fontWeight = if (t == store.settingsTab) androidx.compose.ui.text.font.FontWeight.SemiBold else null) },
                        leadingIcon = { Box(Modifier.width(16.dp)) { if (t == store.settingsTab) LucideIcon("check", size = 16.dp) } },
                        trailingIcon = { if (dirty[t] == true) Box(Modifier.size(7.dp).background(colors.primary, CircleShape)) },
                        contentPadding = PaddingValues(start = 28.dp, end = 12.dp),
                        onClick = {
                            store.settingsTab = t
                            open = false
                        },
                    )
                }
            }
        }
    }
}

/** The app's version, set by the platform that builds it. */
var APP_VERSION: String = ""

/**
 * Settings → This device (spec 0028; ThisDevicePanel.svelte): only what a device can have of its own, over everyone's
 * settings.json: its ID and home page, its reminders, its keys, and its file as text.
 */
@Composable
private fun ThisDevicePanel(store: Store, state: SettingsState) {
    Dim("What this device has of its own: where it opens, its reminders and its keys. Everything else in Settings is the same on every device.", Modifier.padding(top = 10.dp))
    GeneralPanel(store, state.deviceGeneral)
    Text("Reminders", Modifier.padding(top = 14.dp), fontSize = 16.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
    val r = store.notifier.settings
    Row(verticalAlignment = Alignment.CenterVertically) {
        Dim("Always this device's own. " + if (r.enabled) "On, ${r.leadMinutes} minutes ahead." else "Off on this device.", Modifier.weight(1f))
        androidx.compose.material3.OutlinedButton({ store.notifyOpen = true }) { Text("Set reminders") }
    }
    Text("Keys", Modifier.padding(top = 14.dp), fontSize = 16.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
    KeysPanel(store, state.deviceKeys)
    SettingsFilePanel(store, state.deviceGeneral)
}
