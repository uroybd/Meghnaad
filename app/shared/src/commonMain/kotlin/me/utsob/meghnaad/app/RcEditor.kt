package me.utsob.meghnaad.app

import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextDecoration
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

// The editor of the taskrc files: line numbers and colour for what each line is (RcEditor in web/src/lib/rcEditor.ts). What a piece
// of a line is (a setting's name, an `include`, a comment, a line the app will comment out) is the engine's (`tc_core::rc_highlight`,
// through `callHelper`), the same on every front end; how it is drawn is here.

enum class Rc { Comment, Marker, Keyword, Path, Key, Punct, Value, Bad, Secret }

/** One piece of the text, `[start, end)`, and what it is. */
class RcToken(val start: Int, val end: Int, val kind: Rc)

fun rcTokens(text: String): List<RcToken> =
    callHelper("taskrc_highlight", JsonPrimitive(text)).jsonArray.map { t ->
        val o = t.jsonObject
        RcToken(
            o.getValue("start").jsonPrimitive.int,
            o.getValue("end").jsonPrimitive.int,
            Rc.valueOf(o.getValue("kind").jsonPrimitive.content.replaceFirstChar { it.uppercase() }),
        )
    }

/** The colours of the pieces: the console line's, so a setting's name looks as an attribute does there. */
class RcPalette(hl: HlPalette, private val warn: Color, private val keyword: Color, private val secret: Color) {
    private val dim = hl.dim
    private val key = hl.attr
    private val path = hl.str

    fun style(kind: Rc): SpanStyle? = when (kind) {
        Rc.Comment -> SpanStyle(color = dim, fontStyle = FontStyle.Italic)
        Rc.Marker -> SpanStyle(color = warn, fontWeight = FontWeight.SemiBold, fontStyle = FontStyle.Normal)
        Rc.Keyword -> SpanStyle(color = keyword, fontWeight = FontWeight.SemiBold)
        Rc.Path -> SpanStyle(color = path)
        Rc.Key -> SpanStyle(color = key)
        Rc.Punct -> SpanStyle(color = dim)
        Rc.Bad -> SpanStyle(textDecoration = TextDecoration.Underline, color = warn)
        Rc.Secret -> SpanStyle(color = secret, textDecoration = TextDecoration.LineThrough)
        Rc.Value -> null
    }
}

/** The text coloured, character for character (so the cursor and the selection stay where they are). */
class RcTransformation(private val palette: RcPalette) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val src = text.text
        val coloured = buildAnnotatedString {
            append(src)
            for (t in runCatching { rcTokens(src) }.getOrDefault(emptyList())) {
                if (t.end <= src.length) palette.style(t.kind)?.let { addStyle(it, t.start, t.end) }
            }
        }
        return TransformedText(coloured, OffsetMapping.Identity)
    }
}

/**
 * The line numbers of a text laid out in [lineStarts] (the offset each drawn line begins at): a line that wraps takes the number
 * once, so a column of these beside the text stays level with it. The number of a line that is the continuation of one is null.
 */
fun lineNumbers(text: String, lineStarts: List<Int>): List<Int?> {
    var n = 0
    return lineStarts.map { start ->
        if (start == 0 || text.getOrNull(start - 1) == '\n') ++n else null
    }
}

/** The text of a taskrc file, with its line numbers beside it and the lines of [problems] (counted from 1) marked in the numbers. */
@Composable
fun RcTextField(
    value: String,
    onChange: (String) -> Unit,
    enabled: Boolean,
    placeholder: String,
    problems: Set<Int> = emptySet(),
    modifier: Modifier = Modifier,
) {
    val colors = MaterialTheme.colorScheme
    val x = extras
    val palette = remember(colors, x) { RcPalette(HlPalette(x.dark), x.warn, colors.primary, colors.error) }
    val transformation = remember(palette) { RcTransformation(palette) }
    var layout by remember { mutableStateOf<TextLayoutResult?>(null) }
    val style = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp, lineHeight = 17.sp, color = colors.onSurface)

    // The numbers go by how the text is drawn, once it has been (a long line wraps); before that, by its lines.
    val laidOut = layout?.takeIf { it.layoutInput.text.text.length == value.length }
    val starts = laidOut?.let { l -> (0 until l.lineCount).map { l.getLineStart(it) } }
        ?: buildList { add(0); value.forEachIndexed { i, c -> if (c == '\n') add(i + 1) } }
    val numbers = lineNumbers(value, starts)
    val digits = (numbers.lastOrNull { it != null } ?: 1).toString().length
    val gutter = buildAnnotatedString {
        numbers.forEachIndexed { i, n ->
            if (i > 0) append("\n")
            if (n != null) {
                if (n in problems) withStyle(SpanStyle(color = x.warn, fontWeight = FontWeight.Bold)) { append(n.toString()) } else append(n.toString())
            }
        }
    }

    Row(modifier.border(1.dp, colors.outline, androidx.compose.foundation.shape.RoundedCornerShape(4.dp)).padding(horizontal = 8.dp, vertical = 10.dp)) {
        Text(
            gutter,
            Modifier.width((digits * 8 + 4).dp).padding(end = 8.dp),
            style = style.copy(color = colors.onSurfaceVariant, textAlign = TextAlign.End),
        )
        BasicTextField(
            value, onChange, Modifier.weight(1f).heightIn(min = (17 * 12).dp),
            enabled = enabled, textStyle = style, cursorBrush = SolidColor(colors.primary),
            visualTransformation = transformation, onTextLayout = { layout = it },
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
            decorationBox = { inner ->
                Box {
                    if (value.isEmpty()) Text(placeholder, style = style.copy(color = colors.onSurfaceVariant))
                    inner()
                }
            },
        )
    }
}
