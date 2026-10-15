package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.material3.Checkbox
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LocalMinimumInteractiveComponentSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.LinkAnnotation
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLinkStyles
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.text.withLink
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.contentOrNull

// The Markdown in a note or a string UDA. The engine parses it (`tc_core::markdown`, the same for every front end) and changes
// the text when a checkbox is ticked (web/src/lib/markdown.ts is the web's side); what is here is the shape of its tree, a
// call each, and the drawing.

/**
 * A piece of the tree: a block (`p`, `h`, `ul`, `ol`, `li`, `q`, `pre`, `hr`) or an inline (`text`, `b`, `i`, `del`, `code`,
 * `a`, `br`). Only what a kind uses is there.
 */
@Serializable
data class Md(
    val t: String,
    /** The text of `text`, `code` and `pre`. */
    val s: String? = null,
    val c: List<Md> = emptyList(),
    /** The level of `h`; the number a `ol` starts at. */
    val n: Int? = null,
    val href: String? = null,
    /** On an `li` that is a checklist item: whether it is ticked. */
    val done: Boolean? = null,
    /** On a checklist item: the line of the text it is on, which is what [toggledBox] takes. */
    val line: Int? = null,
    val lang: String? = null,
)

/** The trees of some texts, in order; null for one that is only words (it is drawn as it is). */
fun markdownTrees(texts: List<String>): List<List<Md>?> {
    if (texts.isEmpty()) return emptyList()
    val out = callHelper("markdown", JsonArray(texts.map(::JsonPrimitive)))
    return engineJson.decodeFromJsonElement(ListSerializer(ListSerializer(Md.serializer()).nullable), out)
}

/** The text with the checkbox on [line] flipped, or null when that line is not a checklist item. */
fun toggledBox(text: String, line: Int): String? =
    (callHelper("markdown_toggle", JsonPrimitive(text), JsonPrimitive(line)) as? JsonPrimitive)?.contentOrNull

private class Look(val color: Color, val size: TextUnit, val codeBackground: Color, val link: Color, val struck: Boolean)

private fun AnnotatedString.Builder.inlines(nodes: List<Md>, look: Look) {
    for (n in nodes) when (n.t) {
        "text" -> append(n.s.orEmpty())
        "br" -> append('\n')
        "b" -> withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { inlines(n.c, look) }
        "i" -> withStyle(SpanStyle(fontStyle = FontStyle.Italic)) { inlines(n.c, look) }
        "del" -> withStyle(SpanStyle(textDecoration = TextDecoration.LineThrough)) { inlines(n.c, look) }
        "code" -> withStyle(SpanStyle(fontFamily = FontFamily.Monospace, background = look.codeBackground)) { append(n.s.orEmpty()) }
        "a" -> {
            val href = n.href
            if (href == null) inlines(n.c, look) else {
                val style = TextLinkStyles(SpanStyle(color = look.link, textDecoration = TextDecoration.Underline))
                withLink(LinkAnnotation.Url(href, style)) { inlines(n.c, look) }
            }
        }
    }
}

@Composable
private fun Block(n: Md, look: Look, onToggle: ((Int) -> Unit)?) {
    val colors = MaterialTheme.colorScheme
    when (n.t) {
        "p", "h" -> Text(
            buildAnnotatedString { inlines(n.c, look) },
            color = look.color,
            fontSize = look.size,
            fontWeight = if (n.t == "h") FontWeight.Bold else null,
            textDecoration = if (look.struck) TextDecoration.LineThrough else null,
        )
        "ul", "ol" -> Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            n.c.forEachIndexed { i, item -> Item(item, if (n.t == "ol") "${(n.n ?: 1) + i}." else "•", look, onToggle) }
        }
        "q" -> Row(Modifier.height(IntrinsicSize.Min)) {
            Box(Modifier.width(3.dp).fillMaxHeight().background(colors.outline))
            Spacer(Modifier.width(8.dp))
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) { for (b in n.c) Block(b, look, onToggle) }
        }
        "pre" -> Text(
            n.s.orEmpty(),
            Modifier.fillMaxWidth().background(look.codeBackground).horizontalScroll(rememberScrollState()).padding(8.dp),
            color = look.color,
            fontSize = look.size * 0.92f,
            fontFamily = FontFamily.Monospace,
        )
        "hr" -> HorizontalDivider(Modifier.padding(vertical = 4.dp))
    }
}

@Composable
private fun Item(item: Md, marker: String, look: Look, onToggle: ((Int) -> Unit)?) {
    val checked = item.done
    // A ticked item is struck through and quieter, like the web's.
    val inner = if (checked == true) Look(look.color.copy(alpha = 0.6f), look.size, look.codeBackground, look.link, true) else look
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        if (checked == null) {
            Text(marker, color = look.color, fontSize = look.size, modifier = Modifier.width(18.dp))
        } else {
            // Smaller than the box that selects a task, so a checklist reads as part of the text.
            CompositionLocalProvider(LocalMinimumInteractiveComponentSize provides 20.dp) {
                Checkbox(
                    checked,
                    onCheckedChange = { item.line?.let { l -> onToggle?.invoke(l) } },
                    modifier = Modifier.size(20.dp).scale(0.75f),
                    enabled = onToggle != null,
                )
            }
        }
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) { for (b in item.c) Block(b, inner, onToggle) }
    }
}

/**
 * A tree drawn. Nothing in it can add anything but styled text: a link is only one the engine allowed (http, https, mailto).
 * With [onToggle] the checkboxes work, and it is told the line of the one clicked; without it (a note) they are only shown.
 */
@Composable
fun MarkdownView(
    nodes: List<Md>,
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.onSurface,
    size: TextUnit = 14.sp,
    onToggle: ((Int) -> Unit)? = null,
) {
    val colors = MaterialTheme.colorScheme
    val look = Look(color, size, colors.onSurface.copy(alpha = 0.09f), colors.primary, false)
    Column(modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) { for (n in nodes) Block(n, look, onToggle) }
}

private var ticking = false

/**
 * Tick or untick a checkbox in a string UDA. The engine changes the text, and the task is told the new one with an ordinary
 * `modify <uda>:<text>`. One at a time, so a second click never works from the text the first one has just changed.
 */
suspend fun Store.tickBox(from: Entry?, uuid: String, uda: String, text: String, line: Int) {
    if (ticking) return
    val next = toggledBox(text, line) ?: return
    ticking = true
    try {
        act(from, listOf(uuid, "modify", "$uda:$next"), again = true)
    } finally {
        ticking = false
    }
}

/** A note under a description: when it was made, then its text (as Markdown when it uses any). */
@Composable
fun NoteLine(note: NoteCell, modifier: Modifier = Modifier) {
    val dim = MaterialTheme.colorScheme.onSurfaceVariant
    if (note.blocks == null) {
        Dim("${note.at} ${note.text}", modifier)
    } else {
        Column(modifier) {
            Dim(note.at)
            MarkdownView(note.blocks, color = dim, size = 13.sp)
        }
    }
}
