package me.utsob.meghnaad.app

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation

// Lua in colour, for the Hooks editor: the pieces the web's editor colours (web/src/lib/hookEditor.ts): keywords, strings, numbers,
// comments, the names of functions and `true`, `false` and `nil`. It looks at the shape of the text only, so a half-typed script is coloured too.

enum class Lua { Keyword, Str, Num, Comment, Func, Atom }

class LuaToken(val start: Int, val end: Int, val kind: Lua)

private val KEYWORDS = setOf(
    "and", "break", "do", "else", "elseif", "end", "for", "function", "goto", "if", "in", "local", "not", "or", "repeat", "return",
    "then", "until", "while",
)
private val ATOMS = setOf("true", "false", "nil")
private val NUMBER = Regex("""0[xX][0-9a-fA-F]+|(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?""")

private fun isIdentStart(c: Char) = c == '_' || c.isLetter()
private fun isIdent(c: Char) = c == '_' || c.isLetterOrDigit()

/** Where a long bracket (`[[`, `[==[`) that starts at [i] ends, or -1 when [i] does not start one. A bracket never closed runs to the end. */
private fun longEnd(s: String, i: Int): Int {
    if (i >= s.length || s[i] != '[') return -1
    var j = i + 1
    while (j < s.length && s[j] == '=') j++
    if (j >= s.length || s[j] != '[') return -1
    val close = "]" + "=".repeat(j - i - 1) + "]"
    val at = s.indexOf(close, j + 1)
    return if (at < 0) s.length else at + close.length
}

fun luaTokens(s: String): List<LuaToken> {
    val out = mutableListOf<LuaToken>()
    var i = 0
    var afterFunction = false
    while (i < s.length) {
        val c = s[i]
        when {
            c.isWhitespace() -> i++
            c == '-' && s.startsWith("--", i) -> {
                val long = longEnd(s, i + 2)
                val e = if (long >= 0) long else s.indexOf('\n', i).let { if (it < 0) s.length else it }
                out += LuaToken(i, e, Lua.Comment)
                i = e
            }
            c == '"' || c == '\'' -> {
                var j = i + 1
                while (j < s.length && s[j] != c && s[j] != '\n') j += if (s[j] == '\\' && j + 1 < s.length) 2 else 1
                val e = if (j < s.length && s[j] == c) j + 1 else minOf(j, s.length)
                out += LuaToken(i, e, Lua.Str)
                i = e
            }
            c == '[' && longEnd(s, i) >= 0 -> {
                val e = longEnd(s, i)
                out += LuaToken(i, e, Lua.Str)
                i = e
            }
            c.isDigit() || (c == '.' && i + 1 < s.length && s[i + 1].isDigit()) -> {
                val m = NUMBER.matchAt(s, i)
                val e = if (m != null) i + m.value.length else i + 1
                out += LuaToken(i, e, Lua.Num)
                i = e
            }
            isIdentStart(c) -> {
                var j = i + 1
                while (j < s.length && isIdent(s[j])) j++
                val w = s.substring(i, j)
                when {
                    afterFunction -> {
                        // `function a.b:c(...)`: the whole name is the function's.
                        while (j < s.length && (s[j] == '.' || s[j] == ':') && j + 1 < s.length && isIdentStart(s[j + 1])) {
                            j++
                            while (j < s.length && isIdent(s[j])) j++
                        }
                        out += LuaToken(i, j, Lua.Func)
                        afterFunction = false
                    }
                    w in KEYWORDS -> {
                        out += LuaToken(i, j, Lua.Keyword)
                        afterFunction = w == "function"
                    }
                    w in ATOMS -> out += LuaToken(i, j, Lua.Atom)
                }
                i = j
            }
            else -> {
                // `function(` has no name; anything else between `function` and a name ends the wait.
                afterFunction = false
                i++
            }
        }
    }
    return out
}

/** The colours of the pieces, from the app's own (the web's editor takes them from the page's variables). */
class LuaPalette(private val accent: Color, private val ok: Color, private val warn: Color, private val dim: Color, val problem: Color) {
    fun style(kind: Lua): SpanStyle = when (kind) {
        Lua.Keyword -> SpanStyle(color = accent, fontWeight = FontWeight.SemiBold)
        Lua.Str -> SpanStyle(color = ok)
        Lua.Num, Lua.Atom -> SpanStyle(color = warn)
        Lua.Comment -> SpanStyle(color = dim, fontStyle = FontStyle.Italic)
        Lua.Func -> SpanStyle(color = accent)
    }
}

/** The script coloured, character for character (so the cursor and the selection stay where they are), the line that is wrong tinted. */
class LuaTransformation(private val palette: LuaPalette, private val problemLine: Int?) : VisualTransformation {
    override fun filter(text: AnnotatedString): TransformedText {
        val src = text.text
        val coloured = buildAnnotatedString {
            append(src)
            for (t in luaTokens(src)) addStyle(palette.style(t.kind), t.start, t.end)
            problemLine?.let { n -> lineRange(src, n)?.let { (a, b) -> addStyle(SpanStyle(background = palette.problem.copy(alpha = 0.16f)), a, b) } }
        }
        return TransformedText(coloured, OffsetMapping.Identity)
    }
}

/** The characters of line [n] (counted from 1), or null when the script has no such line. */
fun lineRange(src: String, n: Int): Pair<Int, Int>? {
    if (n < 1) return null
    var start = 0
    repeat(n - 1) {
        val nl = src.indexOf('\n', start)
        if (nl < 0) return null
        start = nl + 1
    }
    val end = src.indexOf('\n', start).let { if (it < 0) src.length else it }
    return if (end > start) start to end else null
}
