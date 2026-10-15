package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

/** One line for the common case; "more fields…" opens the full form with this text carried over (QuickAdd.svelte). */
@Composable
fun QuickAdd(store: Store, modifier: Modifier = Modifier) {
    var description by remember { mutableStateOf("") }
    var busy by remember { mutableStateOf(false) }
    val scope = rememberCoroutineScope()

    fun add() {
        if (description.isBlank() || busy) return
        busy = true
        store.go {
            // `description:` keeps words like "project:foo" in the text from being read as fields.
            val res = act(if (view == View.Tasks) live else null, listOf("add", "description:${description.trim()}"))
            busy = false
            if (res != null && res.result !is CliResult.Error) description = ""
        }
    }

    Column(modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                value = description,
                onValueChange = { description = it },
                modifier = Modifier.weight(1f),
                singleLine = true,
                placeholder = { Text("Add a task…", fontSize = 14.sp) },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                keyboardActions = KeyboardActions(onSend = { add() }),
            )
            Button(onClick = ::add, enabled = !busy && description.isNotBlank()) {
                LucideIcon("plus", size = 16.dp, tint = MaterialTheme.colorScheme.onPrimary)
            }
        }
        TextButton(onClick = {
            store.adding = AddRef(description.trim(), (store.adding?.n ?: 0) + 1)
            description = ""
        }) {
            LucideIcon("sliders-horizontal", size = 13.dp)
            Text("  more fields…", fontSize = 13.sp)
        }
    }
}
