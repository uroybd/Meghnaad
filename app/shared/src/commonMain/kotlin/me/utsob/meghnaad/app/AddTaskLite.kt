package me.utsob.meghnaad.app

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import kotlinx.coroutines.launch

/** The add form until the full one: a line of Taskwarrior syntax (`Buy milk project:Home due:tomorrow`), run as `add`. */
@Composable
fun AddTaskLite(store: Store, ref: AddRef) {
    var text by remember(ref.n) { mutableStateOf(ref.description) }
    val scope = rememberCoroutineScope()
    AlertDialog(
        onDismissRequest = { store.adding = null },
        title = { Text("New task") },
        text = { OutlinedTextField(text, { text = it }, label = { Text("Description and fields") }, singleLine = true) },
        confirmButton = {
            TextButton(
                enabled = text.isNotBlank(),
                onClick = {
                    store.adding = null
                    store.go { act(live, splitWords("add $text")) }
                },
            ) { Text("Add") }
        },
        dismissButton = { TextButton(onClick = { store.adding = null }) { Text("Cancel") } },
    )
}
