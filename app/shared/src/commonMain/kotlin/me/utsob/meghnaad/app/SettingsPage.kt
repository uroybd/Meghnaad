package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.focus.focusProperties
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import kotlinx.coroutines.launch
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
    val appearance = AppearanceState(store)
    val keys = KeysState(store)
    val taskrc = TaskrcState(store)
    val commands = CommandsState(store)
    val urgency = UrgencyState(store)
    val tasksh = TaskshState(store)
    val hooks = HooksState(store)
    val kanban = KanbanState(store)
}

/** What a device may have of its own (spec 0027); the rest shows under This device read only. */
private val DEVICE_OWN = setOf(SettingsTab.General, SettingsTab.Keys)

/** Nothing under it takes a touch or a key: a tab shown for its layout, read only. */
private fun Modifier.readOnly(): Modifier = this.pointerInput(Unit) {
    awaitPointerEventScope {
        while (true) awaitPointerEvent(PointerEventPass.Initial).changes.forEach { it.consume() }
    }
}.focusProperties { canFocus = false }

private class SettingsTabInfo(val tab: SettingsTab, val label: String)

private val TABS = listOf(
    SettingsTabInfo(SettingsTab.General, "General"),
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
    SettingsGroup("Meghnaad", listOf(SettingsTab.General, SettingsTab.Appearance, SettingsTab.Keys, SettingsTab.Kanban, SettingsTab.Commands, SettingsTab.Sync)),
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
    val dirty = mapOf(SettingsTab.General to state.general.edited, SettingsTab.Appearance to state.appearance.edited, SettingsTab.Keys to state.keys.edited, SettingsTab.Taskrc to state.taskrc.edited, SettingsTab.Commands to state.commands.edited, SettingsTab.Urgency to state.urgency.edited, SettingsTab.Tasksh to state.tasksh.edited, SettingsTab.Hooks to state.hooks.edited, SettingsTab.Kanban to state.kanban.edited)
    val group = settingsGroupOf(store.settingsTab)
    // The tab last shown in each group, so going back to a group goes back to it.
    val last = remember { mutableStateMapOf<String, SettingsTab>() }
    LaunchedEffect(store.settingsTab) { last[group.label] = store.settingsTab }
    Column(Modifier.fillMaxSize()) {
        val scope = rememberCoroutineScope()
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 10.dp), verticalAlignment = Alignment.CenterVertically) {
            Text("Settings", Modifier.weight(1f), fontSize = 20.sp, fontWeight = androidx.compose.ui.text.font.FontWeight.SemiBold)
            // Whose settings (spec 0027): everyone's settings.json, or this device's own over them.
            SingleChoiceSegmentedButtonRow {
                SegmentedButton(!store.settingsDevice, { store.settingsDevice = false }, SegmentedButtonDefaults.itemShape(0, 2)) { Text("Everyone") }
                SegmentedButton(store.settingsDevice, {
                    store.settingsDevice = true
                    scope.launch { offerDevices(store) }
                }, SegmentedButtonDefaults.itemShape(1, 2)) { Text("This device") }
            }
        }
        DevicePrompt(store)
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
        // The taskrc tab is a workspace that takes the height (its editor scrolls inside it, and Apply stays in view); the others scroll.
        if (store.settingsTab == SettingsTab.Taskrc) {
            if (store.settingsDevice) {
                Text("Set under Everyone: one for every device.", Modifier.padding(horizontal = 16.dp, vertical = 8.dp).clickable { store.settingsDevice = false }, color = MaterialTheme.colorScheme.primary, fontSize = 13.sp)
            }
            Box(Modifier.weight(1f).widthIn(max = 1100.dp).padding(horizontal = 16.dp).padding(bottom = 72.dp)) { TaskrcPanel(store, state.taskrc) }
            return@Column
        }
        // What a device can't have of its own shows under This device as it is, read only.
        val shared = store.settingsDevice && store.settingsTab !in DEVICE_OWN
        Column(Modifier.weight(1f).widthIn(max = 820.dp).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp)) {
            if (shared) {
                Text(
                    "Set under Everyone: one for every device.",
                    Modifier.padding(top = 8.dp).clickable { store.settingsDevice = false },
                    color = MaterialTheme.colorScheme.primary,
                    fontSize = 13.sp,
                )
            }
            Box(Modifier.then(if (shared) Modifier.alpha(0.6f).readOnly() else Modifier)) { Column {
            when (store.settingsTab) {
                SettingsTab.General -> GeneralPanel(store, state.general)
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
            } }
            UpdateRow(store.updates)
            if (store.settingsTab == SettingsTab.General) SettingsFilePanel(store, state.general)
            if (APP_VERSION.isNotEmpty()) Dim("Meghnaad $APP_VERSION", Modifier.padding(vertical = 16.dp), 12)
            // Room under the last control, so the floating + never covers it (the web's 72 px under every page).
            Box(Modifier.height(72.dp))
        }
    }
}

/** The app's version, set by the platform that builds it. */
var APP_VERSION: String = ""
