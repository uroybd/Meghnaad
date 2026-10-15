package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/**
 * A text field that offers names as you type (a project, or a list of tags) and takes anything else too: the offers are a help,
 * and a name nobody has used yet is how a new one is made. A tap takes an offer (SuggestInput.svelte).
 */
@Composable
fun SuggestField(
    value: String,
    onChange: (String) -> Unit,
    names: List<String>,
    noun: String,
    modifier: Modifier = Modifier,
    list: Boolean = false,
    placeholder: String = "",
) {
    var tfv by remember { mutableStateOf(TextFieldValue(value, TextRange(value.length))) }
    var focused by remember { mutableStateOf(false) }
    if (tfv.text != value) tfv = TextFieldValue(value, TextRange(value.length))
    val word = wordAt(tfv.text, tfv.selection.end, list)
    // In a list, what is already there is not offered again.
    val rest = tfv.text.substring(0, word.start) + " " + tfv.text.substring(word.end)
    val taken = if (list) rest.split(Regex("[\\s,]+")).map { it.removePrefix("+") }.toSet() else emptySet()
    val options = if (focused) suggest(names, word.text, taken) else emptyList()

    Column(modifier) {
        OutlinedTextField(
            value = tfv,
            onValueChange = {
                tfv = it
                onChange(it.text)
            },
            modifier = Modifier.fillMaxWidth().onFocusChanged { focused = it.isFocused },
            placeholder = { Text(placeholder, fontSize = 14.sp) },
            singleLine = true,
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
        )
        if (options.isNotEmpty()) {
            Surface(Modifier.fillMaxWidth().padding(top = 2.dp), shape = MaterialTheme.shapes.medium, tonalElevation = 3.dp, shadowElevation = 4.dp, color = MaterialTheme.colorScheme.surfaceContainerHigh) {
                Column {
                    options.forEachIndexed { i, o ->
                        Text(
                            o.value,
                            Modifier.fillMaxWidth().clickable {
                                val r = choose(tfv.text, word, o.value, list)
                                tfv = TextFieldValue(r.value, TextRange(r.caret))
                                onChange(r.value)
                            }.padding(horizontal = 12.dp, vertical = 10.dp),
                            fontSize = 14.sp,
                        )
                        if (i < options.lastIndex) HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.5f))
                    }
                }
            }
        }
        if (isNew(names, word.text)) Dim("“${word.text}” will be a new $noun.", Modifier.padding(top = 2.dp, start = 4.dp), 12)
    }
}
