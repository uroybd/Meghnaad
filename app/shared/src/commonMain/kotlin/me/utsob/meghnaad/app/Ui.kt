package me.utsob.meghnaad.app

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.RowScope
import androidx.compose.material3.ButtonColors
import androidx.compose.material3.ButtonDefaults
import androidx.compose.foundation.layout.PaddingValues
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
    /** The touch area: smaller where the icon sits inside a field. */
    box: Dp = 44.dp,
) {
    IconButton(onClick = onClick, enabled = enabled, modifier = modifier.size(box).semantics { contentDescription = description }) {
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

/** The app's mark, set by the platform that builds it (the web's logo, which stands beside the name). */
var APP_LOGO: androidx.compose.ui.graphics.ImageBitmap? = null

@Composable
fun Logo(size: Dp, modifier: Modifier = Modifier) {
    APP_LOGO?.let { Image(it, contentDescription = null, modifier = modifier.size(size)) }
}

/**
 * What opens over a page: the whole screen on a phone and, where there is room (the web's width), what the web draws: a drawer
 * on the right for a task's details, or a card in the middle for a form, over a dimmed page when [scrim] says so.
 */
@Composable
fun Sheet(drawer: Boolean, width: Dp, color: Color, modifier: Modifier = Modifier, scrim: Boolean = false, content: @Composable () -> Unit) {
    val outline = MaterialTheme.colorScheme.outline
    BoxWithConstraints(Modifier.fillMaxSize()) {
        when {
            maxWidth <= TABLE_FROM -> Surface(modifier.fillMaxSize(), color = color, content = content)
            drawer -> Surface(
                modifier.align(Alignment.CenterEnd).fillMaxHeight().widthIn(max = width),
                color = color, border = BorderStroke(1.dp, outline), shadowElevation = 8.dp, content = content,
            )
            else -> Box(
                Modifier.fillMaxSize().let { if (scrim) it.background(Color.Black.copy(alpha = 0.32f)).clickable(interactionSource = null, indication = null) {} else it },
                contentAlignment = Alignment.Center,
            ) {
                Surface(modifier.widthIn(max = width).fillMaxHeight(0.94f), shape = PanelShape, color = color, border = BorderStroke(1.dp, outline), shadowElevation = 8.dp, content = content)
            }
        }
    }
}

/** The web's corners: a button, field or list row is 6px there, and a card, menu or dialog is its 8px `--radius`. Only chips, tags and status pills are round. */
val ControlShape = RoundedCornerShape(6.dp)
val PanelShape = RoundedCornerShape(8.dp)

// Material's own buttons are fully round, whatever the theme's shapes say; these take the web's corner for the same call.
@Composable
fun Button(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, colors: ButtonColors = ButtonDefaults.buttonColors(), contentPadding: PaddingValues = ButtonDefaults.ContentPadding, content: @Composable RowScope.() -> Unit) =
    androidx.compose.material3.Button(onClick = onClick, modifier = modifier, enabled = enabled, shape = ControlShape, colors = colors, contentPadding = contentPadding, content = content)

@Composable
fun OutlinedButton(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, contentPadding: PaddingValues = ButtonDefaults.ContentPadding, content: @Composable RowScope.() -> Unit) =
    androidx.compose.material3.OutlinedButton(onClick = onClick, modifier = modifier, enabled = enabled, shape = ControlShape, contentPadding = contentPadding, content = content)

@Composable
fun TextButton(onClick: () -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true, contentPadding: PaddingValues = ButtonDefaults.TextButtonContentPadding, content: @Composable RowScope.() -> Unit) =
    androidx.compose.material3.TextButton(onClick = onClick, modifier = modifier, enabled = enabled, shape = ControlShape, contentPadding = contentPadding, content = content)

@Composable
fun Card(modifier: Modifier = Modifier, color: Color = MaterialTheme.colorScheme.surfaceContainerLow, content: @Composable () -> Unit) {
    Surface(
        modifier = modifier,
        shape = PanelShape,
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
