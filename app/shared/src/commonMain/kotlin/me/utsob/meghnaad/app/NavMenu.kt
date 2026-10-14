package me.utsob.meghnaad.app

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The pages a menu lists after Tasks: the same in the same order as the web's. */
private class MenuItem(val view: View, val label: String, val icon: String)

private val VIEWS = listOf(
    MenuItem(View.Projects, "Projects", "folder"),
    MenuItem(View.Tags, "Tags", "tag"),
    MenuItem(View.Summary, "Summary", "chart-column"),
    MenuItem(View.Calendar, "Calendar", "calendar-days"),
    MenuItem(View.Burndown, "Burndown", "trending-down"),
    MenuItem(View.Console, "Console", "terminal"),
)

/** One line of the menu: an icon, a label that takes the room, and a small state at the end. */
@Composable
private fun Item(icon: String, label: String, on: Boolean, state: String? = null, enabled: Boolean = true, trailing: @Composable () -> Unit = {}, onClick: () -> Unit) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().heightIn(min = 44.dp).clip(RoundedCornerShape(10.dp))
            .background(if (on) colors.primary.copy(alpha = 0.12f) else androidx.compose.ui.graphics.Color.Transparent)
            .clickable(enabled = enabled, onClick = onClick)
            .padding(horizontal = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp),
    ) {
        LucideIcon(icon, size = 18.dp, tint = if (on) colors.primary else colors.onSurface.copy(alpha = if (enabled) 1f else 0.4f))
        Text(label, Modifier.weight(1f), fontSize = 15.sp, fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal, color = if (enabled) colors.onSurface else colors.onSurface.copy(alpha = 0.4f))
        if (!state.isNullOrEmpty()) Text(state, fontSize = 12.sp, color = colors.onSurfaceVariant)
        trailing()
    }
}

/**
 * The menu: Tasks, which opens into the list of reports (closed to begin with: it is a long list, and the pages should not sit
 * below it), then the pages, then Review, Reminders and Settings. On a wide window only the list of reports shows, always open
 * (the pages are in the top bar). Mirrors NavMenu.svelte.
 */
@Composable
fun NavMenu(store: Store, onNavigate: () -> Unit, pages: Boolean = true) {
    var reportsOpen by remember { mutableStateOf(!pages) }
    val colors = MaterialTheme.colorScheme

    Column(Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 8.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        if (pages) {
            Item(
                "list-checks", "Tasks", store.view == View.Tasks,
                state = if (store.view == View.Tasks) store.report else null,
                trailing = { LucideIcon("chevron-down", Modifier.rotate(if (reportsOpen) 180f else 0f), size = 16.dp) },
                onClick = { reportsOpen = !reportsOpen },
            )
        }
        AnimatedVisibility(reportsOpen) {
            Column(Modifier.padding(start = 12.dp, bottom = 6.dp)) {
                Row(Modifier.fillMaxWidth().padding(horizontal = 6.dp, vertical = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                    Text("REPORTS", Modifier.weight(1f), fontSize = 11.sp, letterSpacing = 0.8.sp, color = colors.onSurfaceVariant)
                    if (store.hasHiddenReports) {
                        IconAction(
                            if (store.showHiddenReports) "eye" else "eye-off",
                            if (store.showHiddenReports) "Hide the reports named _something" else "Show the reports named _something",
                            { store.showHidden(!store.showHiddenReports) },
                            size = 14.dp,
                        )
                    }
                }
                for (r in store.offeredReports) {
                    val on = store.view == View.Tasks && store.report == r.name
                    Text(
                        r.name,
                        Modifier.fillMaxWidth().heightIn(min = 38.dp).clip(RoundedCornerShape(8.dp))
                            .background(if (on) colors.primary.copy(alpha = 0.12f) else androidx.compose.ui.graphics.Color.Transparent)
                            .clickable {
                                store.report = r.name
                                store.view = View.Tasks
                                onNavigate()
                            }
                            .padding(horizontal = 10.dp, vertical = 9.dp),
                        fontSize = 14.sp,
                        color = if (on) colors.primary else colors.onSurface,
                        fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal,
                    )
                }
            }
        }
        if (pages) {
            for (v in VIEWS) {
                Item(v.icon, v.label, store.view == v.view) {
                    store.view = v.view
                    onNavigate()
                }
            }
            HorizontalDivider(Modifier.padding(vertical = 6.dp), color = colors.outlineVariant)
            Item("clipboard-check", "Review", false) {
                store.notify("Review is not in this build yet.")
                onNavigate()
            }
            Item("bell-off", "Reminders", false, state = "off") {
                store.notifyOpen = true
                onNavigate()
            }
            Item("settings", "Settings", store.view == View.Settings) {
                store.openSettings()
                onNavigate()
            }
        }
        store.configError?.let { Text(it, color = colors.error, fontSize = 13.sp, modifier = Modifier.padding(10.dp)) }
    }
}
