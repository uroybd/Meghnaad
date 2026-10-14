package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

/** What can be done to the ticked tasks all at once (the first of it: BulkBar.svelte has more, to come). */
@Composable
fun BulkBar(store: Store) {
    val n = store.selection.count
    if (n == 0) return
    val scope = rememberCoroutineScope()
    Row(Modifier.fillMaxWidth().padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        Text("$n selected", fontSize = 14.sp, color = MaterialTheme.colorScheme.onSurface, modifier = Modifier.weight(1f))
        TextButton(onClick = {
            store.go {
                act(live, selection.uuids.toList() + "done")
                selection.clear()
            }
        }) { Text("Done") }
        TextButton(onClick = { store.selection.clear() }) { Text("Clear") }
    }
}
