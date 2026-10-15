package me.utsob.meghnaad.app

import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isAltPressed
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.input.key.utf16CodePoint
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The review with buttons, over the page: the tasksh menu's choices, one task at a time (ReviewCard.svelte, ReviewBar.svelte). */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ReviewCard(store: Store) {
    val review = store.review
    val colors = MaterialTheme.colorScheme
    var args by remember { mutableStateOf("") }
    val choosing = review.phase == ReviewPhase.Running && !review.busy

    // The keys of tasksh's menu (and y, n to a question) on a hardware keyboard, unless something is being typed; Enter is the focused
    // button's. Escape ends the review, or cancels the change being typed (ReviewCard.svelte, ReviewBar.svelte).
    val keys = Modifier.onPreviewKeyEvent { e ->
        if (e.type != KeyEventType.KeyDown || e.isCtrlPressed || e.isMetaPressed || e.isAltPressed) return@onPreviewKeyEvent false
        if (e.key == Key.Escape) {
            when (review.phase) {
                ReviewPhase.Modifying -> { args = ""; store.go { review.modify("") } }
                ReviewPhase.Done -> review.close()
                else -> review.quit()
            }
            return@onPreviewKeyEvent true
        }
        if (TypingFocus.typing) return@onPreviewKeyEvent false
        val ch = e.utf16CodePoint
        if (ch <= 32 || ch > 126) return@onPreviewKeyEvent false
        store.go { review.press(ch.toChar().toString()) }
        true
    }
    Sheet(drawer = false, width = 760.dp, color = colors.surfaceContainerLow, modifier = keys, scrim = true) {
        Column(Modifier.fillMaxSize()) {
            Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 4.dp, top = 6.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                if (review.phase in setOf(ReviewPhase.Running, ReviewPhase.Asking, ReviewPhase.Modifying, ReviewPhase.Editing)) {
                    Text(
                        "[${review.current + 1} of ${review.total}]", fontFamily = FontFamily.Monospace, fontSize = 13.sp, color = colors.onPrimary,
                        modifier = Modifier.background(colors.primary, RoundedCornerShape(4.dp)).padding(horizontal = 7.dp, vertical = 1.dp),
                    )
                }
                Text(review.row?.description ?: "Review", Modifier.weight(1f), fontWeight = FontWeight.SemiBold, fontSize = 18.sp, maxLines = 3)
                IconAction("x", "Quit the review", { review.quit() }, size = 20.dp)
            }
            Column(Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(horizontal = 16.dp, vertical = 8.dp)) {
                if (review.phase == ReviewPhase.Done) {
                    Text(review.message, color = if (review.messageBad) colors.error else colors.onSurface, modifier = Modifier.padding(vertical = 14.dp))
                    Button(onClick = { review.close() }) { Text("Close") }
                } else {
                    val row = review.row
                    if (row != null) TaskInfo(store, row) { u -> store.openDetail(u) }
                    else if (review.phase != ReviewPhase.Setup) Dim(review.message.ifEmpty { "Looking for tasks to review…" }, size = 14)

                    if (review.phase == ReviewPhase.Setup || review.phase == ReviewPhase.Asking) {
                        Text(review.question ?: "", Modifier.padding(vertical = 12.dp), fontSize = 15.sp)
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            val yes = { if (review.phase == ReviewPhase.Setup) store.go { review.answerSetup(true) } else store.go { review.answer(true) } }
                            val no = { if (review.phase == ReviewPhase.Setup) store.go { review.answerSetup(false) } else store.go { review.answer(false) } }
                            Button(onClick = { yes() }, enabled = !review.busy) { Text("Yes") }
                            OutlinedButton(onClick = { no() }, enabled = !review.busy) { Text("No") }
                        }
                    }
                    if (review.phase == ReviewPhase.Modifying) {
                        Dim(REVIEW_MODIFY_PROMPT, Modifier.padding(top = 12.dp), 13)
                        OutlinedTextField(
                            args, { args = it }, Modifier.fillMaxWidth().padding(top = 6.dp), singleLine = true,
                            placeholder = { Text("+tag -tag /teh/the/ project:X", fontSize = 13.sp) },
                            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                        )
                        Row(Modifier.padding(top = 6.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            Button(enabled = !review.busy, onClick = { val w = args; args = ""; store.go { review.modify(w) } }) { Text("Modify") }
                            OutlinedButton(enabled = !review.busy, onClick = { args = ""; store.go { review.modify("") } }) { Text("Cancel") }
                        }
                    }
                    if (review.message.isNotEmpty() && review.phase != ReviewPhase.Setup && review.phase != ReviewPhase.Asking) {
                        Text(review.message, Modifier.padding(top = 10.dp), color = if (review.messageBad) colors.error else colors.onSurfaceVariant, fontSize = 13.5.sp)
                    }
                }
            }
            if (review.phase == ReviewPhase.Running) {
                FlowRow(
                    Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    fun pick(k: ReviewKey) = store.go { review.act(k) }
                    Button(onClick = { pick(ReviewKey.Reviewed) }, enabled = choosing) { LucideIcon("check", size = 15.dp, tint = colors.onPrimary); Text("  Reviewed") }
                    OutlinedButton(onClick = { pick(ReviewKey.Skip) }, enabled = choosing) { LucideIcon("skip-forward", size = 14.dp); Text("  Skip") }
                    OutlinedButton(onClick = { pick(ReviewKey.Edit) }, enabled = choosing) { LucideIcon("square-pen", size = 14.dp); Text("  Edit") }
                    OutlinedButton(onClick = { pick(ReviewKey.Modify) }, enabled = choosing) { LucideIcon("pencil", size = 14.dp); Text("  Modify") }
                    OutlinedButton(onClick = { pick(ReviewKey.Complete) }, enabled = choosing) { LucideIcon("check", size = 14.dp); Text("  Complete") }
                    OutlinedButton(onClick = { pick(ReviewKey.Delete) }, enabled = choosing) { LucideIcon("trash-2", size = 14.dp, tint = colors.error); Text("  Delete", color = colors.error) }
                    TextButton(onClick = { pick(ReviewKey.Quit) }, enabled = !review.busy) { Text("Quit") }
                }
            }
        }
    }
}
