package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.withStyle

// Syntax highlighting for a line of Taskwarrior syntax: what each piece is is the engine's, how it is drawn is here.

enum class Hl { Keyword, Command, Attr, Punct, Value, Str, Tag, Id, Op, Config, Text }

/** One piece of the line, `[start, end)`, and what it is. Spaces between pieces are not pieces. */
class HlToken(val start: Int, val end: Int, val kind: Hl)

/**
 * The pieces of a line of Taskwarrior syntax: the engine's own (`tc_core::highlight`, through `callHelper`), which every front end
 * shares. [commands]: every word that is a command here: Taskwarrior's, the reports, and the user's own. Only the first one found counts.
 */
fun highlight(line: String, commands: Set<String>): List<HlToken> =
    callHelper("highlight", JsonPrimitive(line), JsonArray(commands.map { JsonPrimitive(it) })).jsonArray.map { t ->
        val o = t.jsonObject
        HlToken(
            o.getValue("start").jsonPrimitive.int,
            o.getValue("end").jsonPrimitive.int,
            Hl.valueOf(o.getValue("kind").jsonPrimitive.content.replaceFirstChar { it.uppercase() }),
        )
    }

/** The colours of the pieces: the same wherever the line is drawn, lighter on a dark page. */
class HlPalette(dark: Boolean) {
    internal val command = if (dark) Color(0xFF79C0FF) else Color(0xFF0969DA)
    internal val attr = if (dark) Color(0xFFD2A8FF) else Color(0xFF8250DF)
    private val tag = if (dark) Color(0xFF56D364) else Color(0xFF1A7F37)
    private val id = if (dark) Color(0xFFFFA657) else Color(0xFFBC4C00)
    internal val op = if (dark) Color(0xFFFF7B72) else Color(0xFFCF222E)
    internal val str = if (dark) Color(0xFFA5D6FF) else Color(0xFF0A3069)
    internal val dim = if (dark) Color(0xFF8B949E) else Color(0xFF6E7781)

    fun style(kind: Hl): SpanStyle? = when (kind) {
        Hl.Command -> SpanStyle(color = command, fontWeight = FontWeight.SemiBold)
        Hl.Keyword -> SpanStyle(color = dim)
        Hl.Attr -> SpanStyle(color = attr)
        Hl.Punct -> SpanStyle(color = dim)
        Hl.Tag -> SpanStyle(color = tag)
        Hl.Id -> SpanStyle(color = id)
        Hl.Op -> SpanStyle(color = op, fontWeight = FontWeight.SemiBold)
        Hl.Str -> SpanStyle(color = str)
        Hl.Config -> SpanStyle(color = dim, fontStyle = FontStyle.Italic)
        Hl.Value, Hl.Text -> null
    }
}

/** The line coloured, character for character (so the cursor and the selection stay where they are). */
class HighlightTransformation(
    private val commands: Set<String>,
    private val palette: HlPalette,
    /** Date words on a holiday: drawn with a yellow wash and an underline. */
    private val marks: List<ClashMark> = emptyList(),
    private val warn: Color = Color(0xFFE3B341),
) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val line = text.text
        val coloured = buildAnnotatedString {
            append(line)
            for (t in highlight(line, commands)) palette.style(t.kind)?.let { addStyle(it, t.start, t.end) }
            for (m in marks) {
                if (m.end <= line.length) addStyle(SpanStyle(background = warn.copy(alpha = 0.3f), textDecoration = androidx.compose.ui.text.style.TextDecoration.Underline), m.start, m.end)
            }
        }
        return TransformedText(coloured, OffsetMapping.Identity)
    }
}
