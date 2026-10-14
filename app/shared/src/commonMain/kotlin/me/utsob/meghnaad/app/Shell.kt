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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

@Composable
fun Shell(store: Store) {
    Box(Modifier.fillMaxSize()) {
        PhoneShell(store)
        ToastHost(store)
        Dialogs(store)
    }
}

/**
 * The layout: a phone's, and the only one. The menu slides over from the left, the prompt is docked at the bottom where a thumb
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
                QuickAdd(store)
                NavMenu(store, onNavigate = close)
            }
        },
    ) {
        Column(Modifier.fillMaxSize()) {
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
        Text("Meghnaad", Modifier.padding(horizontal = 4.dp), fontWeight = FontWeight.Bold, fontSize = 17.sp)
        Box(Modifier.weight(1f))
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
