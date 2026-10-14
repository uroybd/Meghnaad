package me.utsob.meghnaad.app

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
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

/** The width below which the layout is a phone's: a slide-over menu, cards, the prompt docked at the bottom. */
private val PHONE = 760.dp

@Composable
fun Shell(store: Store) {
    BoxWithConstraints(Modifier.fillMaxSize()) {
        if (maxWidth > PHONE) WideShell(store) else PhoneShell(store)
        ToastHost(store)
        Dialogs(store)
    }
}

@Composable
private fun PhoneShell(store: Store) {
    val drawer = rememberDrawerState(DrawerValue.Closed)
    val scope = rememberCoroutineScope()
    val close: () -> Unit = { scope.launch { drawer.close() } }
    ModalNavigationDrawer(
        drawerState = drawer,
        drawerContent = {
            ModalDrawerSheet(drawerContainerColor = MaterialTheme.colorScheme.surfaceContainer) {
                Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 8.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("Meghnaad", Modifier.weight(1f), fontWeight = FontWeight.Bold, fontSize = 17.sp)
                    IconAction("x", "Close menu", close)
                }
                QuickAdd(store)
                NavMenu(store, onNavigate = close)
            }
        },
    ) {
        Column(Modifier.fillMaxSize()) {
            TopBar(store, wide = false, onMenu = { scope.launch { drawer.open() } })
            Banners(store)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                MainView(store)
                NewTaskButton(store, Modifier.align(Alignment.BottomEnd))
            }
            if (store.view != View.Console) Dock(store)
        }
    }
}

@Composable
private fun WideShell(store: Store) {
    Row(Modifier.fillMaxSize()) {
        Column(Modifier.width(264.dp).fillMaxHeight().background(MaterialTheme.colorScheme.surfaceContainer)) {
            Text("Meghnaad", Modifier.padding(16.dp), fontWeight = FontWeight.Bold, fontSize = 17.sp)
            QuickAdd(store)
            NavMenu(store, onNavigate = {}, pages = false)
        }
        Column(Modifier.weight(1f).fillMaxHeight()) {
            TopBar(store, wide = true, onMenu = {})
            Banners(store)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                MainView(store)
                NewTaskButton(store, Modifier.align(Alignment.BottomEnd))
            }
            if (store.view != View.Console) Dock(store)
        }
    }
}

private class Tab(val view: View, val label: String, val icon: String)

private val TABS = listOf(
    Tab(View.Tasks, "Tasks", "list-checks"),
    Tab(View.Projects, "Projects", "folder"),
    Tab(View.Tags, "Tags", "tag"),
    Tab(View.Summary, "Summary", "chart-column"),
    Tab(View.Calendar, "Calendar", "calendar-days"),
    Tab(View.Burndown, "Burndown", "trending-down"),
    Tab(View.Console, "Console", "terminal"),
)

/** The bar over every page: on a phone the menu button and the name; on a wide window the pages too (TopBar of App.svelte). */
@Composable
private fun TopBar(store: Store, wide: Boolean, onMenu: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().background(colors.surfaceContainer).height(52.dp).padding(horizontal = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        if (!wide) IconAction("menu", "Menu", onMenu, size = 20.dp)
        Text("Meghnaad", Modifier.padding(horizontal = 6.dp), fontWeight = FontWeight.Bold, fontSize = 16.sp)
        if (wide) {
            for (t in TABS) {
                val on = store.view == t.view
                Row(
                    Modifier.clip(RoundedCornerShape(8.dp)).background(if (on) colors.primary.copy(alpha = 0.12f) else androidx.compose.ui.graphics.Color.Transparent)
                        .clickable { store.view = t.view }.padding(horizontal = 10.dp, vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    LucideIcon(t.icon, size = 15.dp, tint = if (on) colors.primary else colors.onSurface)
                    Text(t.label, fontSize = 13.sp, color = if (on) colors.primary else colors.onSurface)
                }
            }
        }
        Box(Modifier.weight(1f))
        store.config?.config?.activeContext?.let { Pill("context: $it") }
        if (wide) {
            IconAction("clipboard-check", "Review", { store.notify("Review is not in this build yet.") }, size = 16.dp)
            IconAction("bell-off", "Reminders (off)", { store.notifyOpen = true }, size = 17.dp)
            IconAction("settings", "Settings", { store.openSettings() }, size = 16.dp, tint = if (store.view == View.Settings) colors.primary else colors.onSurface)
        }
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
        onClick = { store.adding = AddRef("", (store.adding?.n ?: 0) + 1) },
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
    when (store.view) {
        View.Tasks -> TasksView(store)
        View.Console -> ConsoleView(store)
        else -> ComingSoon(store.view.name)
    }
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
            shape = RoundedCornerShape(10.dp),
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
    store.adding?.let { AddTaskLite(store, it) }
    store.detail?.let { d ->
        AlertDialog(
            onDismissRequest = { store.detail = null },
            title = { Text("Task") },
            text = { Dim("The detail view is not in this build yet. (${d.uuid.take(8)})", size = 14) },
            confirmButton = { TextButton(onClick = { store.detail = null }) { Text("Close") } },
        )
    }
    if (store.notifyOpen) {
        AlertDialog(
            onDismissRequest = { store.notifyOpen = false },
            title = { Text("Reminders") },
            text = { Dim("Reminders are not in this build yet.", size = 14) },
            confirmButton = { TextButton(onClick = { store.notifyOpen = false }) { Text("Close") } },
        )
    }
}
