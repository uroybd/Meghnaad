package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.heightIn
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.VerticalDivider
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isAltPressed
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

@Composable
fun Shell(store: Store) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        // The web's two layouts: with room, a sidebar and tabs along the top; without, the menu over the page and the phone's bar.
        if (maxWidth > TABLE_FROM) WideShell(store, labels = maxWidth > 1100.dp, buttonLabels = maxWidth > 1400.dp) else PhoneShell(store)
        ToastHost(store)
        Dialogs(store)
    }
}

/** `/` focuses the prompt from anywhere that isn't a text field (a hardware keyboard). */
private fun slashKey(store: Store) = Modifier.onPreviewKeyEvent { e ->
    val slash = e.type == KeyEventType.KeyDown && e.key == Key.Slash && !e.isCtrlPressed && !e.isMetaPressed && !e.isAltPressed
    if (slash && !TypingFocus.typing && store.detail == null) {
        store.focusPrompt()
        true
    } else false
}

/** The pages along the top of the wide layout, as the web's tabs. */
private class Tab(val view: View, val label: String, val icon: String)

private val TABS = listOf(
    Tab(View.Tasks, "Tasks", "list-checks"),
    Tab(View.Projects, "Projects", "folder"),
    Tab(View.Tags, "Tags", "tag"),
    Tab(View.Summary, "Summary", "chart-column"),
    Tab(View.Calendar, "Calendar", "calendar-days"),
    Tab(View.Burndown, "Burndown", "trending-down"),
    Tab(View.Boards, "Boards", "layout-dashboard"),
    Tab(View.Console, "Console", "terminal"),
)

/**
 * The layout with room for it (App.svelte, over 760 px): the bar along the top with a tab for each page, the list of reports in
 * a sidebar that is always there, the page beside it and the prompt under both. There is no menu button; the floating + is the
 * same as on a phone.
 */
@Composable
private fun WideShell(store: Store, labels: Boolean, buttonLabels: Boolean) {
    val colors = MaterialTheme.colorScheme
    Column(Modifier.fillMaxSize().then(slashKey(store))) {
        WideTopBar(store, labels, buttonLabels)
        Banners(store)
        Row(Modifier.weight(1f).fillMaxWidth()) {
            Column(Modifier.width(270.dp).fillMaxHeight().background(colors.surfaceContainer).padding(top = 12.dp)) {
                NavMenu(store, onNavigate = {}, pages = false)
            }
            VerticalDivider(color = colors.outlineVariant)
            Box(Modifier.weight(1f).fillMaxHeight()) {
                MainView(store)
                if (store.view != View.Console) NewTaskButton(store, Modifier.align(Alignment.BottomEnd))
            }
        }
        if (store.view != View.Console) Dock(store)
    }
}

@Composable
private fun WideTopBar(store: Store, labels: Boolean, buttonLabels: Boolean) {
    val colors = MaterialTheme.colorScheme
    @Composable
    fun Button(icon: String, label: String, on: Boolean, showLabel: Boolean, enabled: Boolean = true, onClick: () -> Unit) {
        Row(
            Modifier.heightIn(min = 36.dp).clip(ControlShape)
                .background(if (on) colors.primary.copy(alpha = 0.12f) else androidx.compose.ui.graphics.Color.Transparent)
                .clickable(enabled = enabled, onClick = onClick)
                .semantics { contentDescription = label }
                .padding(horizontal = if (showLabel) 12.dp else 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            LucideIcon(icon, size = 16.dp, tint = if (on) colors.primary else colors.onSurface.copy(alpha = if (enabled) 1f else 0.4f))
            if (showLabel) Text(label, fontSize = 13.5.sp, fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal, color = if (on) colors.primary else colors.onSurface.copy(alpha = if (enabled) 1f else 0.4f))
        }
    }
    Column(Modifier.fillMaxWidth().background(colors.surfaceContainer)) {
        Row(Modifier.fillMaxWidth().height(52.dp).padding(horizontal = 12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(2.dp)) {
            Logo(28.dp, Modifier.padding(end = 8.dp))
            Text("Meghnaad", Modifier.padding(end = 14.dp), fontWeight = FontWeight.Bold, fontSize = 17.sp)
            for (t in TABS) Button(t.icon, t.label, store.view == t.view, labels) { store.view = t.view }
            Box(Modifier.weight(1f))
            TimerChip(store, Modifier)
            store.config?.config?.activeContext?.let { Pill("context: $it", Modifier.padding(horizontal = 8.dp)) }
            Button("clipboard-check", "Review", store.review.phase != ReviewPhase.Idle, buttonLabels, enabled = store.review.phase == ReviewPhase.Idle) { store.go { store.review.start() } }
            Button(if (store.notifier.settings.enabled) "bell" else "bell-off", "Reminders (${if (store.notifier.settings.enabled) "on" else "off"})", false, false) { store.notifyOpen = true }
            Button("settings", "Settings", store.view == View.Settings, buttonLabels) { store.openSettings() }
        }
        HorizontalDivider(color = colors.outlineVariant)
    }
}

/**
 * The phone's layout (a window of up to 760 dp): the menu slides over from the left, the prompt is docked at the bottom where a thumb
 * reaches it, the new-task button floats over the list, and a page is a column of cards that scrolls as one.
 */
@Composable
private fun PhoneShell(store: Store) {
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val close: () -> Unit = { store.menuOpen = false }
    // The store knows whether the menu is open (so the back button can close it); the drawer follows it, and the other way.
    LaunchedEffect(store.menuOpen) { if (store.menuOpen) drawer.open() else drawer.close() }
    LaunchedEffect(drawer.currentValue) { store.menuOpen = drawer.currentValue == DrawerValue.Open }
    ModalNavigationDrawer(
        drawerState = drawer,
        drawerContent = {
            ModalDrawerSheet(drawerContainerColor = MaterialTheme.colorScheme.surfaceContainer) {
                Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Meghnaad", Modifier.weight(1f), fontWeight = FontWeight.Bold, fontSize = 17.sp)
                    IconAction("x", "Close menu", close)
                }
                NavMenu(store, onNavigate = close)
            }
        },
    ) {
        Column(Modifier.fillMaxSize().then(slashKey(store))) {
            TopBar(store, onMenu = { scope.launch { store.menuOpen = true } })
            Banners(store)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                MainView(store)
                if (store.view != View.Console) NewTaskButton(store, Modifier.align(Alignment.BottomEnd))
            }
            if (store.view != View.Console) Dock(store)
        }
    }
}

/** The bar over every page: the menu button and the name; the pages are in the menu (TopBar of App.svelte, phone widths). */
@Composable
private fun TopBar(store: Store, onMenu: () -> Unit) {
    Row(
        Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surfaceContainer).height(56.dp).padding(horizontal = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        IconAction("menu", "Menu", onMenu, size = 22.dp)
        Logo(28.dp)
        Text("Meghnaad", Modifier.padding(horizontal = 6.dp), fontWeight = FontWeight.Bold, fontSize = 17.sp)
        Box(Modifier.weight(1f))
        TimerChip(store, Modifier.weight(3f, fill = false))
        store.config?.config?.activeContext?.let { Pill("context: $it", Modifier.padding(end = 8.dp)) }
    }
}

/** What needs saying under the bar: unreadable settings, and a sync that failed (with the same two choices as the web). */
@Composable
private fun Banners(store: Store) {
    val colors = MaterialTheme.colorScheme
    val sync = store.sync
    store.config?.configError?.let {
        Row(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
            LucideIcon("triangle-alert", size = 15.dp, tint = colors.error)
            Text(
                "  Your saved taskrc settings couldn't be read, so UDAs and custom reports are off for now. Nothing was deleted.",
                Modifier.weight(1f), color = colors.error, fontSize = 13.sp,
            )
            TextButton(onClick = { store.openSettings(SettingsTab.Taskrc) }) { Text("Fix it") }
        }
    }
    if (sync.status == SyncStatus.Failed && !store.localOnly) {
        Column(Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 4.dp)) {
            Row(verticalAlignment = Alignment.Top) {
                LucideIcon("triangle-alert", Modifier.padding(top = 2.dp), size = 15.dp, tint = colors.error)
                Text(
                    "  Couldn't sync with the server (${sync.error}). What you change is kept on this phone and goes up when it works again.",
                    color = colors.error, fontSize = 13.sp,
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                TextButton(onClick = { sync.request() }) { Text("Retry") }
                TextButton(onClick = {
                    // Throwing the copy away is not undone, so it is asked first.
                    store.requestStartOver()
                }) { Text("Start over from the server") }
            }
        }
    }
}

@Composable
private fun NewTaskButton(store: Store, modifier: Modifier) {
    FloatingActionButton(
        onClick = { store.quickAdd() },
        modifier = modifier.padding(16.dp),
        containerColor = MaterialTheme.colorScheme.primary,
        contentColor = MaterialTheme.colorScheme.onPrimary,
    ) { LucideIcon("plus", size = 22.dp, tint = MaterialTheme.colorScheme.onPrimary) }
}

/** The prompt under every page but the Console (which carries its own). */
@Composable
private fun Dock(store: Store) {
    Surface(Modifier.fillMaxWidth(), color = MaterialTheme.colorScheme.surfaceContainer, tonalElevation = 2.dp) {
        ConsoleInput(store)
    }
}

/** The page the menu is on. */
@Composable
private fun MainView(store: Store) {
    // Once opened, Settings keeps its edits while another page is shown.
    val settingsState = remember(store) { SettingsState(store) }
    // Pull down on a page to sync and draw it again; not on the console, which scrolls on its own.
    val scope = rememberCoroutineScope()
    var refreshing by remember { mutableStateOf(false) }
    val page: @Composable () -> Unit = {
        when (store.view) {
            View.Tasks -> TasksView(store)
            View.Console -> ConsoleView(store)
            View.Projects -> ProjectsView(store)
            View.Tags -> TagsView(store)
            View.Summary -> SummaryPage(store)
            View.Settings -> SettingsPage(store, settingsState)
            View.Calendar -> CalendarPage(store)
            View.Burndown -> BurndownPage(store)
            View.Boards -> BoardsPage(store)
            else -> ComingSoon(store.view.name)
        }
    }
    if (store.view == View.Console) page() else PullToRefreshBox(
        isRefreshing = refreshing,
        onRefresh = {
            refreshing = true
            scope.launch {
                store.go { pullRefresh() }.join()
                refreshing = false
            }
        },
        modifier = Modifier.fillMaxSize(),
    ) { page() }
}

@Composable
private fun ComingSoon(name: String) {
    Box(Modifier.fillMaxSize(), Alignment.Center) { Dim("$name is not in this build yet.", size = 15) }
}

/** A message that comes and goes by itself: how an action went (a tap, a hook's words). */
@Composable
private fun ToastHost(store: Store) {
    val t = store.toast ?: return
    val colors = MaterialTheme.colorScheme
    Box(Modifier.fillMaxSize().padding(bottom = 96.dp, start = 16.dp, end = 16.dp), Alignment.BottomCenter) {
        Surface(
            shape = PanelShape,
            color = if (t.bad) colors.errorContainer else colors.inverseSurface,
            shadowElevation = 6.dp,
            modifier = Modifier.clickable { store.dismissToast() },
        ) {
            Text(t.text, Modifier.padding(horizontal = 14.dp, vertical = 10.dp), color = if (t.bad) colors.onErrorContainer else colors.inverseOnSurface, fontSize = 14.sp)
        }
    }
}

/** What opens over the page: a question, the add form, the detail of a task. */
@Composable
private fun Dialogs(store: Store) {
    store.ask?.let { a ->
        AlertDialog(
            onDismissRequest = { store.answerAsk(false) },
            text = { Text(a.message) },
            confirmButton = { TextButton(onClick = { store.answerAsk(true) }) { Text(a.yes) } },
            dismissButton = { TextButton(onClick = { store.answerAsk(false) }) { Text(a.no) } },
        )
    }
    store.switchAsk?.let { q ->
        AlertDialog(
            onDismissRequest = { store.answerSwitch(null) },
            title = { Text("Sync somewhere else?") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                    Text(q.message)
                    Text("What should happen to the tasks on this phone?", fontWeight = FontWeight.SemiBold)
                    Button(onClick = { store.answerSwitch("merge") }, modifier = Modifier.fillMaxWidth()) { Text("Keep them and add them there") }
                    Dim("They are added to what is already there, and everything is then on both.", size = 12)
                    OutlinedButton(onClick = { store.answerSwitch("replace") }, modifier = Modifier.fillMaxWidth()) { Text("Start from what is there") }
                    Dim("The tasks on this phone are let go, and only what the new place holds is kept.", size = 12)
                }
            },
            confirmButton = {},
            dismissButton = { TextButton(onClick = { store.answerSwitch(null) }) { Text("Cancel") } },
        )
    }
    // A review is a card over the page; the editor and a task's details open over it.
    if (store.review.phase != ReviewPhase.Idle) ReviewCard(store)
    store.editing?.let { e -> key(e.row.uuid) { TaskEditor(store, e.row, e.from, onClose = { store.closeEditor() }) } }
    store.adding?.let { a ->
        key(a.n) {
            if (a.quick) {
                QuickAdd(store, store.live, a.line, onClose = { store.adding = null })
            } else {
                TaskEditor(
                    store, null, store.live, a.seed,
                    onClose = { store.adding = null },
                    onAdded = { again -> if (again) store.addWithForm() else store.adding = null },
                )
            }
        }
    }
    // The detail of a task is the whole screen (a phone's drawer); the editor opens over it.
    store.detail?.let { d -> key(d.uuid) { DetailScreen(store, d) } }
    if (store.notifyOpen) NotifyDialog(store)
}
