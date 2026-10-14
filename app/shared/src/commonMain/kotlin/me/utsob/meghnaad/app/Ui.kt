package me.utsob.meghnaad.app

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.defaultMinSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.IconButton
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** A button that is only an icon: the quiet kind the web calls `ghost`. */
@Composable
fun IconAction(
    icon: String,
    description: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    tint: Color = LocalContentColor.current,
    size: Dp = 18.dp,
    enabled: Boolean = true,
) {
    IconButton(onClick = onClick, enabled = enabled, modifier = modifier.size(44.dp).semantics { contentDescription = description }) {
        LucideIcon(icon, size = size, tint = if (enabled) tint else tint.copy(alpha = 0.38f))
    }
}

/** A small rounded label: a tag, a count, a state. */
@Composable
fun Pill(
    text: String,
    modifier: Modifier = Modifier,
    background: Color = MaterialTheme.colorScheme.surfaceVariant,
    color: Color = MaterialTheme.colorScheme.onSurface,
    onClick: (() -> Unit)? = null,
    border: BorderStroke? = null,
    mono: Boolean = false,
    height: Dp = 22.dp,
    bold: Boolean = false,
) {
    val base = modifier.defaultMinSize(minHeight = height).clip(CircleShape).background(background)
    Row(
        (if (onClick != null) base.clickable(onClick = onClick) else base).padding(horizontal = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text,
            color = color,
            fontSize = 12.sp,
            fontWeight = if (bold) FontWeight.SemiBold else FontWeight.Normal,
            fontFamily = if (mono) androidx.compose.ui.text.font.FontFamily.Monospace else null,
            maxLines = 1,
        )
    }
}

@Composable
fun Card(modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.surfaceContainerLow, content: @Composable () -> Unit) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(14.dp),
        color = color,
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.outline),
        content = content,
    )
}

@Composable
fun Dim(text: String, modifier: Modifier = Modifier, size: Int = 13) {
    Text(text, modifier, color = MaterialTheme.colorScheme.onSurfaceVariant, fontSize = size.sp)
}

@Composable
fun Spacer4(w: Dp = 4.dp) = Box(Modifier.size(w))

val GapSmall = Arrangement.spacedBy(6.dp)
