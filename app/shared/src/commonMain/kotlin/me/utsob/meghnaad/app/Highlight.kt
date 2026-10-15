package me.utsob.meghnaad.app

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

// Syntax highlighting for a line of Taskwarrior syntax (web/src/lib/highlight.ts: the same rules, and the same test table).
// It looks at the shape of each word only: a command is a word the vocabulary knows, `name:value` is an attribute, `+tag` a tag.

enum class Hl { Keyword, Command, Attr, Punct, Value, Str, Tag, Id, Op, Config, Text }

/** One piece of the line, `[start, end)`, and what it is. Spaces between pieces are not pieces. */
class HlToken(val start: Int, val end: Int, val kind: Hl)

private val OPERATORS = setOf("and", "or", "xor", "not", "!")

/** After these the rest of the line is a description, where `and` is a word and not an operator. */
private val DESCRIBING = setOf("add", "log", "annotate")
private val TAG = Regex("^[+-][A-Za-z_][A-Za-z0-9_.-]*$")
private val NUMBERS = Regex("^[0-9]+([,-][0-9]+)*$")
private val UUID = Regex("^[0-9a-fA-F]{8}(-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})?$")
private val ATTRIBUTE = Regex("^([A-Za-z_][A-Za-z0-9_.]*)([:=])(.*)$", RegexOption.DOT_MATCHES_ALL)

/** The words of the line as `[start, end)`: split at spaces, except inside quotes, and after a backslash. */
private fun words(line: String): List<IntRange> {
    val out = mutableListOf<IntRange>()
    var i = 0
    while (i < line.length) {
        if (line[i].isWhitespace()) {
            i++
            continue
        }
        val start = i
        var quote: Char? = null
        while (i < line.length) {
            val c = line[i]
            if (c == '\\' && i + 1 < line.length) {
                i += 2
                continue
            }
            if (quote != null) {
                if (c == quote) quote = null
            } else if (c == '"' || c == '\'') {
                quote = c
            } else if (c.isWhitespace()) break
            i++
        }
        out += start until i
    }
    return out
}

/** [commands]: every word that is a command here: Taskwarrior's, the reports, and the user's own. Only the first one found counts. */
fun highlight(line: String, commands: Set<String>): List<HlToken> {
    val out = mutableListOf<HlToken>()
    var commandSeen = false
    var describing = false
    words(line).forEachIndexed { index, range ->
        var s = range.first
        var e = range.last + 1
        if (index == 0 && line.substring(s, e) == "task") {
            out += HlToken(s, e, Hl.Keyword)
            return@forEachIndexed
        }
        // Brackets around a word are operators of their own.
        while (!describing && s < e && line[s] == '(') out += HlToken(s, ++s, Hl.Op)
        val closers = mutableListOf<HlToken>()
        while (!describing && e > s && line[e - 1] == ')') {
            closers += HlToken(e - 1, e, Hl.Op)
            e--
        }
        if (s < e) {
            val w = line.substring(s, e)
            val attribute = ATTRIBUTE.find(w)
            when {
                !describing && w.lowercase() in OPERATORS -> out += HlToken(s, e, Hl.Op)
                w[0] == '"' || w[0] == '\'' -> out += HlToken(s, e, Hl.Str)
                TAG.matches(w) -> out += HlToken(s, e, Hl.Tag)
                NUMBERS.matches(w) || (UUID.matches(w) && w.any { it.isDigit() }) -> out += HlToken(s, e, Hl.Id)
                attribute != null -> {
                    val name = attribute.groupValues[1]
                    val config = name.startsWith("rc.") || name == "rc"
                    out += HlToken(s, s + name.length, if (config) Hl.Config else Hl.Attr)
                    out += HlToken(s + name.length, s + name.length + 1, Hl.Punct)
                    val v = s + name.length + 1
                    if (v < e) out += HlToken(v, e, if (config) Hl.Config else if (line[v] == '"' || line[v] == '\'') Hl.Str else Hl.Value)
                }
                !commandSeen && w in commands -> {
                    commandSeen = true
                    describing = w in DESCRIBING
                    out += HlToken(s, e, Hl.Command)
                }
                else -> out += HlToken(s, e, Hl.Text)
            }
        }
        out += closers.asReversed()
    }
    return out
}

/** The colours of the pieces: the same wherever the line is drawn, lighter on a dark page. */
class HlPalette(dark: Boolean) {
    private val command = if (dark) Color(0xFF79C0FF) else Color(0xFF0969DA)
    private val attr = if (dark) Color(0xFFD2A8FF) else Color(0xFF8250DF)
    private val tag = if (dark) Color(0xFF56D364) else Color(0xFF1A7F37)
    private val id = if (dark) Color(0xFFFFA657) else Color(0xFFBC4C00)
    private val op = if (dark) Color(0xFFFF7B72) else Color(0xFFCF222E)
    private val str = if (dark) Color(0xFFA5D6FF) else Color(0xFF0A3069)
    private val dim = if (dark) Color(0xFF8B949E) else Color(0xFF6E7781)

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
class HighlightTransformation(private val commands: Set<String>, private val palette: HlPalette) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val line = text.text
        val coloured = buildAnnotatedString {
            append(line)
            for (t in highlight(line, commands)) palette.style(t.kind)?.let { addStyle(it, t.start, t.end) }
        }
        return TransformedText(coloured, OffsetMapping.Identity)
    }
}
