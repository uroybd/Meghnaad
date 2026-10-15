package me.utsob.meghnaad.app

import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** How many are drawn at once: a broad search is cut short, and typing more narrows it. */
private const val SHOWN = 48

/** Pick a Lucide icon by looking at it, or by its name or what it is for ("cancel" finds the circled cross). An empty value is no icon (IconPicker.svelte). */
@Composable
fun IconPicker(value: String, onChange: (String) -> Unit, label: String = "Icon") {
    var open by remember { mutableStateOf(false) }
    OutlinedButton(
        onClick = { open = true },
        modifier = Modifier.size(52.dp).semantics { contentDescription = if (value.isNotEmpty()) "$label: $value" else "$label: none, choose one" },
        contentPadding = androidx.compose.foundation.layout.PaddingValues(0.dp),
    ) { CommandIcon(value.ifEmpty { null }, 18) }
    if (value.isNotEmpty() && LucideLib.loaded && !LucideLib.has(value)) Text("No Lucide icon is called “$value”.", color = MaterialTheme.colorScheme.error, fontSize = 12.sp)
    if (open) {
        var query by remember { mutableStateOf("") }
        LaunchedEffect(Unit) { LucideLib.load() }
        val matches = if (LucideLib.loaded) searchIcons(LucideLib.tags, query) else emptyList()
        val shown = matches.take(SHOWN)
        val typed = query.trim().lowercase()
        // A name is taken as typed only while the list of icons is not there to check it against (the web's, when it cannot be fetched).
        val usable = ICON_NAME.matches(typed) && !LucideLib.loaded
        AlertDialog(
            onDismissRequest = { open = false },
            title = { Text("Choose an icon") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        query, { query = it },
                        Modifier.fillMaxWidth().onPreviewKeyEvent { e ->
                            if (e.type == KeyEventType.KeyDown && e.key == Key.Enter && usable) { onChange(typed); open = false; true } else false
                        },
                        singleLine = true,
                        placeholder = { Text(if (LucideLib.loaded) "Search ${LucideLib.names().size} icons" else "Search icons, or type a name", fontSize = 14.sp) },
                        keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                    )
                    LazyVerticalGrid(GridCells.Adaptive(48.dp), Modifier.fillMaxWidth().heightIn(max = 260.dp)) {
                        items(shown) { name ->
                            Box(
                                Modifier.padding(2.dp).size(44.dp).clip(ControlShape)
                                    .background(if (name == value) MaterialTheme.colorScheme.primary.copy(alpha = 0.18f) else Color.Transparent)
                                    .clickable { onChange(name); open = false }
                                    .semantics { contentDescription = name },
                                contentAlignment = Alignment.Center,
                            ) { LucideIcon(name, size = 20.dp) }
                        }
                    }
                    Dim(
                        when {
                            !LucideLib.loaded -> "Loading icons…"
                            matches.isEmpty() -> "Nothing is called that."
                            matches.size > shown.size -> "${shown.size} of ${matches.size}: type more to narrow it."
                            else -> "${matches.size} found."
                        },
                        size = 12,
                    )
                    if (usable) TextButton(onClick = { onChange(typed); open = false }) { Text("Use “$typed”") }
                }
            },
            confirmButton = { TextButton(onClick = { open = false }) { Text("Close") } },
            dismissButton = if (value.isNotEmpty()) ({ TextButton(onClick = { onChange(""); open = false }) { Text("No icon") } }) else null,
        )
    }
}
