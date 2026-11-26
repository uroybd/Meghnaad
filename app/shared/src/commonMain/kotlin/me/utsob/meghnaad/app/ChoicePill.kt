package me.utsob.meghnaad.app

import androidx.compose.foundation.BorderStroke
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.unit.dp

/**
 * A UDA's value that is one of its choices (spec 0034; web ChoicePill.svelte): a pill in the value's own colour, the same wherever it is
 * shown; grey for a value not among the choices.
 */
@Composable
fun ChoicePill(store: Store, name: String, value: String, choice: Int) {
    val colors = MaterialTheme.colorScheme
    val hue = choiceColor(baseColumn(name), value, choice, store.config?.colors ?: emptyMap(), extras.dark, colors.background)
    Pill(
        value,
        height = 21.dp,
        background = hue?.copy(alpha = 0.16f) ?: colors.surfaceVariant,
        color = if (hue != null) colors.onSurface else colors.onSurfaceVariant,
        border = BorderStroke(1.dp, hue?.copy(alpha = 0.45f) ?: colors.outlineVariant),
    )
}
