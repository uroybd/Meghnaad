package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

/** The page the app opens on, kept in `settings.json` (GeneralPanel.svelte): any report or board, `next` when nothing is chosen. */
class GeneralState(private val store: Store) {
    private val saved get() = store.homepage

    /** The choice: a report, a board, or nothing (the default, `next`). */
    var choice by mutableStateOf<Homepage?>(saved)

    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var done by mutableStateOf(false)

    /** What the choice was last made from: when the saved one changes and the choice was not touched, it follows. */
    private var base = saved

    fun follow() {
        val now = saved
        if (now == base) return
        val untouched = choice == base
        base = now
        if (untouched) choice = now
    }

    val edited get() = choice != saved

    /** The saved home page names a report or board that is not there (any more), so the app opens on `next`. */
    val missing: Boolean
        get() = saved?.let { h -> if (h.report != null) store.reports.none { it.name == h.report } else store.boards.none { it.id == h.board } } ?: false

    suspend fun apply() {
        busy = true
        error = null
        done = false
        try {
            store.saveHomepage(choice)
            // What is saved now is what the choice was made from.
            base = saved
            done = true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }
}

@Composable
private fun Choice(label: String, selected: Boolean, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 44.dp).clickable(onClick = onClick), verticalAlignment = Alignment.CenterVertically) {
        RadioButton(selected, onClick = null, modifier = Modifier.padding(horizontal = 8.dp))
        Text(label, fontSize = 15.sp, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal)
    }
}

@Composable
fun GeneralPanel(store: Store, state: GeneralState) {
    state.follow()
    val scope = rememberCoroutineScope()
    Column(Modifier.padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("Home page", fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
        Dim(
            "The page the app opens on: any report, including your own, or any of your boards. Left alone it is next. " +
                "It is kept in this app's own settings.json, not in your taskrc.",
        )
        Choice("next (the default)", state.choice == null) { state.choice = null; state.done = false }
        store.offeredReports.filter { it.name != "next" }.forEach { r ->
            Choice(r.name, state.choice?.report == r.name) { state.choice = Homepage(report = r.name); state.done = false }
        }
        if (store.boards.isNotEmpty()) {
            Dim("Boards", Modifier.padding(top = 6.dp))
            store.boards.forEach { b ->
                Choice(b.name, state.choice?.board == b.id) { state.choice = Homepage(board = b.id); state.done = false }
            }
        }
        if (state.missing) Text("The saved home page is not there any more, so the app opens on next.", color = MaterialTheme.colorScheme.error, fontSize = 13.sp)
        state.error?.let { Text(it, color = MaterialTheme.colorScheme.error, fontSize = 13.sp) }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            Button({ scope.launch { state.apply() } }, enabled = !state.busy && state.edited) { Text("Apply") }
            if (state.done) Dim("Saved")
        }
    }
}
