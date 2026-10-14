package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** The options of a completion, as a list to tap: the highlighted one is what Tab or the arrows chose. */
@Composable
fun CompletionMenu(completer: Completer, onPick: (Int) -> Unit, modifier: Modifier = Modifier) {
    val menu = completer.menu ?: return
    val state = rememberLazyListState()
    LaunchedEffect(menu.sel) { if (menu.sel >= 0) state.animateScrollToItem(menu.sel) }
    Surface(
        modifier = modifier.fillMaxWidth().heightIn(max = 240.dp),
        shape = MaterialTheme.shapes.medium,
        tonalElevation = 3.dp,
        shadowElevation = 6.dp,
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
    ) {
        LazyColumn(state = state) {
            itemsIndexed(menu.options) { i, o ->
                Row(
                    Modifier.fillMaxWidth()
                        .background(if (i == menu.sel) MaterialTheme.colorScheme.primary.copy(alpha = 0.14f) else androidx.compose.ui.graphics.Color.Transparent)
                        .clickable { onPick(i) }
                        .padding(horizontal = 12.dp, vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(o.value, fontFamily = FontFamily.Monospace, fontSize = 13.sp)
                    o.hint?.let { Text(it, fontSize = 12.sp, color = MaterialTheme.colorScheme.onSurfaceVariant, maxLines = 1) }
                }
                if (i < menu.options.lastIndex) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
            }
        }
    }
}

/**
 * A line of Taskwarrior syntax with completion: Tab completes the shared prefix and further Tabs cycle the options (a
 * hardware keyboard); the Complete button does the same on a phone, and a tap on an option takes it. [onKey] sees every key
 * first (the console's history); [onSubmit] is the keyboard's Send.
 */
@Composable
fun CompletingField(
    value: String,
    onValueChange: (String) -> Unit,
    vocab: Vocab,
    modifier: Modifier = Modifier,
    placeholder: String = "",
    label: String? = null,
    completer: Completer = remember { Completer() },
    onSubmit: (() -> Unit)? = null,
    onKey: ((androidx.compose.ui.input.key.KeyEvent, String) -> Boolean)? = null,
    onFocus: (Boolean) -> Unit = {},
    trailing: @Composable (complete: () -> Unit) -> Unit = {},
    enabled: Boolean = true,
    singleLine: Boolean = true,
    focusRequester: androidx.compose.ui.focus.FocusRequester? = null,
    /** Counts up each time the Complete button is pressed (the first value is the start). */
    completeSignal: Int = 0,
    mono: Boolean = true,
) {
    var tfv by remember { mutableStateOf(TextFieldValue(value, TextRange(value.length))) }
    // An outside change (history, a request to put text here): take it, with the cursor after it.
    if (tfv.text != value) tfv = TextFieldValue(value, TextRange(value.length))

    fun apply(edit: Edit?) {
        if (edit == null) return
        tfv = TextFieldValue(edit.line, TextRange(edit.caret))
        onValueChange(edit.line)
    }

    fun complete() = apply(completer.tab(tfv.text, tfv.selection.end, vocab))

    var seenSignal by remember { mutableStateOf(completeSignal) }
    if (completeSignal != seenSignal) {
        seenSignal = completeSignal
        complete()
    }

    Column(modifier) {
        OutlinedTextField(
            value = tfv,
            onValueChange = {
                val changed = it.text != tfv.text
                tfv = it
                if (changed) {
                    completer.close()
                    onValueChange(it.text)
                }
            },
            modifier = Modifier.fillMaxWidth()
                .let { m -> if (focusRequester != null) m.focusRequester(focusRequester) else m }
                .onFocusChanged {
                    onFocus(it.isFocused)
                    if (!it.isFocused) completer.close()
                }
                .onPreviewKeyEvent { e ->
                    if (e.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                    if (onKey?.invoke(e, tfv.text) == true) return@onPreviewKeyEvent true
                    val open = completer.menu != null
                    when {
                        e.key == Key.Tab -> {
                            val r = completer.tab(tfv.text, tfv.selection.end, vocab, e.isShiftPressed)
                            if (r != null) apply(r)
                            r != null
                        }
                        (e.key == Key.DirectionDown || e.key == Key.DirectionUp) && open -> {
                            apply(completer.move(if (e.key == Key.DirectionDown) 1 else -1))
                            true
                        }
                        e.key == Key.Enter && open && (completer.menu?.sel ?: -1) >= 0 -> {
                            completer.close()
                            true
                        }
                        e.key == Key.Escape && open -> {
                            completer.close()
                            true
                        }
                        else -> false
                    }
                },
            label = label?.let { { Text(it) } },
            placeholder = { Text(placeholder, fontSize = 13.sp) },
            singleLine = singleLine,
            enabled = enabled,
            textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = if (mono) FontFamily.Monospace else null),
            keyboardOptions = KeyboardOptions(
                autoCorrectEnabled = false,
                capitalization = KeyboardCapitalization.None,
                imeAction = if (onSubmit != null) ImeAction.Send else ImeAction.Default,
            ),
            keyboardActions = KeyboardActions(onSend = { onSubmit?.invoke() }),
            trailingIcon = { trailing(::complete) },
        )
        CompletionMenu(completer, onPick = { apply(completer.pick(it)) }, modifier = Modifier.padding(top = 2.dp))
    }
}

