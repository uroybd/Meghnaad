package me.utsob.meghnaad.app

import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsFocusedAsState
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextFieldDefaults
import androidx.compose.material3.ProvideTextStyle
import androidx.compose.material3.TextFieldColors
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

// Text fields as tall as the select buttons and the pills beside them (Material's own is 56 dp, which is a lot of a phone's
// screen in a form). Same parameters as Material's `OutlinedTextField`, so a screen only has to not import that one. A label
// is drawn above the field instead of floating in its border, which would need the extra height back.

/** The height of a one-line field: that of the select buttons (`Choice`). */
private val FIELD_HEIGHT = 40.dp

/** A one-line field is as round as the select beside it; a taller one is rounded less. */
private val PILL = ControlShape
private val BOX = ControlShape

@Composable
fun OutlinedTextField(
    value: TextFieldValue,
    onValueChange: (TextFieldValue) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    readOnly: Boolean = false,
    textStyle: TextStyle = MaterialTheme.typography.bodyMedium,
    label: @Composable (() -> Unit)? = null,
    placeholder: @Composable (() -> Unit)? = null,
    leadingIcon: @Composable (() -> Unit)? = null,
    trailingIcon: @Composable (() -> Unit)? = null,
    prefix: @Composable (() -> Unit)? = null,
    suffix: @Composable (() -> Unit)? = null,
    supportingText: @Composable (() -> Unit)? = null,
    isError: Boolean = false,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    singleLine: Boolean = false,
    maxLines: Int = if (singleLine) 1 else Int.MAX_VALUE,
    minLines: Int = 1,
    interactionSource: MutableInteractionSource? = null,
    shape: Shape? = null,
    colors: TextFieldColors = OutlinedTextFieldDefaults.colors(),
) {
    val source = interactionSource ?: remember { MutableInteractionSource() }
    val focused by source.collectIsFocusedAsState()
    // A field is a pill while its text is on one line, and a rounded box once it wraps.
    var lines by remember { mutableStateOf(1) }
    val oneLine = singleLine || (minLines == 1 && lines <= 1)
    val color = when {
        !enabled -> colors.disabledTextColor
        isError -> colors.errorTextColor
        focused -> colors.focusedTextColor
        else -> colors.unfocusedTextColor
    }
    val field: @Composable () -> Unit = {
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            modifier = modifier.defaultMinSize(minHeight = FIELD_HEIGHT),
            enabled = enabled,
            readOnly = readOnly,
            textStyle = textStyle.merge(TextStyle(color = color)),
            cursorBrush = SolidColor(if (isError) colors.errorCursorColor else colors.cursorColor),
            visualTransformation = visualTransformation,
            keyboardOptions = keyboardOptions,
            keyboardActions = keyboardActions,
            interactionSource = source,
            singleLine = singleLine,
            maxLines = maxLines,
            minLines = minLines,
            onTextLayout = { lines = it.lineCount },
            decorationBox = { inner ->
                OutlinedTextFieldDefaults.DecorationBox(
                    value = value.text,
                    innerTextField = inner,
                    enabled = enabled,
                    singleLine = singleLine,
                    visualTransformation = visualTransformation,
                    interactionSource = source,
                    isError = isError,
                    // Material draws a placeholder in its own larger style; it is the field's, so the text and it match.
                    placeholder = placeholder?.let { p -> { ProvideTextStyle(textStyle) { p() } } },
                    leadingIcon = leadingIcon,
                    trailingIcon = trailingIcon,
                    prefix = prefix,
                    suffix = suffix,
                    supportingText = supportingText,
                    colors = colors,
                    contentPadding = PaddingValues(horizontal = 16.dp, vertical = 8.dp),
                    container = { OutlinedTextFieldDefaults.Container(enabled = enabled, isError = isError, interactionSource = source, colors = colors, shape = shape ?: if (oneLine) PILL else BOX) },
                )
            },
        )
    }
    if (label == null) field() else {
        Column {
            ProvideTextStyle(LocalTextStyle.current.merge(TextStyle(fontSize = 13.sp, color = MaterialTheme.colorScheme.onSurfaceVariant))) {
                Column(Modifier.padding(start = 4.dp, bottom = 3.dp)) { label() }
            }
            field()
        }
    }
}

@Composable
fun OutlinedTextField(
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    readOnly: Boolean = false,
    textStyle: TextStyle = MaterialTheme.typography.bodyMedium,
    label: @Composable (() -> Unit)? = null,
    placeholder: @Composable (() -> Unit)? = null,
    leadingIcon: @Composable (() -> Unit)? = null,
    trailingIcon: @Composable (() -> Unit)? = null,
    prefix: @Composable (() -> Unit)? = null,
    suffix: @Composable (() -> Unit)? = null,
    supportingText: @Composable (() -> Unit)? = null,
    isError: Boolean = false,
    visualTransformation: VisualTransformation = VisualTransformation.None,
    keyboardOptions: KeyboardOptions = KeyboardOptions.Default,
    keyboardActions: KeyboardActions = KeyboardActions.Default,
    singleLine: Boolean = false,
    maxLines: Int = if (singleLine) 1 else Int.MAX_VALUE,
    minLines: Int = 1,
    interactionSource: MutableInteractionSource? = null,
    shape: Shape? = null,
    colors: TextFieldColors = OutlinedTextFieldDefaults.colors(),
) {
    // A String field keeps its own cursor and selection, as Material's does.
    var state by remember { mutableStateOf(TextFieldValue(value)) }
    if (state.text != value) state = state.copy(text = value, selection = androidx.compose.ui.text.TextRange(value.length))
    OutlinedTextField(
        state, { state = it; if (it.text != value) onValueChange(it.text) }, modifier, enabled, readOnly, textStyle, label, placeholder, leadingIcon,
        trailingIcon, prefix, suffix, supportingText, isError, visualTransformation, keyboardOptions, keyboardActions, singleLine, maxLines, minLines,
        interactionSource, shape, colors,
    )
}

/**
 * What stands beside a field in a row, centred on the field itself: [fieldHeight] is the field's own height in pixels (from its
 * `onSize`), so a select or an icon lines up with it at any font size, and whatever hangs under the field (a completion menu) does
 * not move it.
 */
@Composable
fun BesideField(fieldHeight: Int, modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    val height = with(LocalDensity.current) { fieldHeight.toDp() }
    Box(modifier.let { if (fieldHeight > 0) it.height(height) else it.heightIn(min = FIELD_HEIGHT) }, contentAlignment = Alignment.Center) { content() }
}
