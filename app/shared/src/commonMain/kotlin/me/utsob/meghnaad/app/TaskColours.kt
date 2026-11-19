package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * The colours of tasks. A theme is saved as its own file, `themes/<id>.rc`, which the main taskrc includes in place of its own colour
 * lines; the switch is `color=off` in the main taskrc. Each change is saved at once, as one save (the theme's file and the main taskrc
 * together), so Restore previous on the taskrc tab undoes it (TaskColours.svelte).
 */
class TaskColoursState(private val store: Store) {
    var text by mutableStateOf<String?>(null)
    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var said by mutableStateOf<String?>(null)

    val off get() = text?.let { colourOff(it) } ?: false
    val chosen get() = text?.let { chosenTheme(it) }

    suspend fun load() {
        try {
            text = store.request("GET", "/api/config/taskrc")
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        }
    }

    private suspend fun save(next: String, what: String, theme: Pair<String, String>? = null) {
        busy = true
        error = null
        said = null
        try {
            if (theme != null) {
                // The theme's file and the main taskrc together: one save, one step for Restore previous.
                val body = kotlinx.serialization.json.buildJsonObject {
                    put("mode", kotlinx.serialization.json.JsonPrimitive("merge"))
                    put("files", kotlinx.serialization.json.buildJsonArray {
                        for ((path, t) in listOf(theme, RC_ROOT to next)) add(kotlinx.serialization.json.buildJsonObject {
                            put("path", kotlinx.serialization.json.JsonPrimitive(path))
                            put("text", kotlinx.serialization.json.JsonPrimitive(t))
                        })
                    })
                }
                store.request("POST", "/api/config/files/import", body.toString())
                text = store.request("GET", "/api/config/taskrc")
            } else {
                text = engineJson.decodeFromString(TaskrcResponse.serializer(), store.request("PUT", "/api/config/taskrc", next)).text
            }
            said = "$what Restore previous on the taskrc tab undoes it."
            store.settingsChanged()
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }

    suspend fun pickTheme(id: String) {
        val now = text ?: return
        val label = THEMES.firstOrNull { it.id == id }?.label ?: id
        try {
            val chosen = chooseTheme(now, id, if (id == "default") "" else loadTheme(id))
            save(chosen.main, chosen.theme?.let { "Saved the $label colours, in ${it.first}." } ?: "Saved the $label colours.", chosen.theme)
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        }
    }

    suspend fun colours(on: Boolean) {
        val now = text ?: return
        save(withColour(now, on), if (on) "Colours are on." else "Colours are off.")
    }
}

@Composable
fun TaskColours(store: Store) {
    val st = remember(store) { TaskColoursState(store) }
    // Read the main taskrc once shown, and again when the settings change elsewhere.
    LaunchedEffect(store.config) { if (!st.busy) st.load() }
    val colors = MaterialTheme.colorScheme
    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("Task colours", fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
        Dim("A theme is kept as its own file, themes/<name>.rc, which your taskrc includes in place of its own color.* lines; the Meghnaad theme is what applies when there are none. Saved at once.")
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Dim("Colour theme")
            val current = THEMES.firstOrNull { it.id == st.chosen }?.label ?: if (st.text != null) "your own, or the default" else "choose…"
            Choice(current, THEMES.map { it.label }, hints = THEMES.map { it.note }) { i -> store.go { st.pickTheme(THEMES[i].id) } }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Switch(checked = !st.off, enabled = !st.busy && st.text != null, onCheckedChange = { on -> store.go { st.colours(on) } })
            Text("Colours", fontSize = 14.sp)
        }
        st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        st.said?.let { Text(it, color = extras.ok, fontSize = 13.sp) }
    }
}
