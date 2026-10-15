package me.utsob.meghnaad.app

import kotlin.test.Test
import kotlin.test.assertEquals

class LuaHighlightTest {
    private fun pieces(s: String) = luaTokens(s).map { s.substring(it.start, it.end) to it.kind }

    @Test
    fun keywordsStringsNumbersAndAtomsAreTold() {
        assertEquals(
            listOf("local" to Lua.Keyword, "\"a b\"" to Lua.Str, "42" to Lua.Num, "nil" to Lua.Atom, "if" to Lua.Keyword, "true" to Lua.Atom, "then" to Lua.Keyword, "end" to Lua.Keyword),
            pieces("local x = \"a b\" + 42 nil\nif true then end").filter { it.first != "x" },
        )
    }

    @Test
    fun aCommentRunsToTheEndOfTheLineAndALongOneUntilItsCloser() {
        assertEquals(listOf("-- hi there" to Lua.Comment, "end" to Lua.Keyword), pieces("-- hi there\nend"))
        assertEquals(listOf("--[[ one\ntwo ]]" to Lua.Comment, "end" to Lua.Keyword), pieces("--[[ one\ntwo ]] end"))
        assertEquals(listOf("--[==[ a ]] b ]==]" to Lua.Comment), pieces("--[==[ a ]] b ]==]"))
    }

    @Test
    fun aStringKeepsItsEscapedQuoteAndALongStringItsBrackets() {
        assertEquals(listOf("'it\\'s'" to Lua.Str), pieces("'it\\'s'"))
        assertEquals(listOf("[[a\nb]]" to Lua.Str), pieces("[[a\nb]]"))
        // A string that was never closed ends with its line, so the next line is coloured as itself.
        assertEquals(listOf("\"open" to Lua.Str, "end" to Lua.Keyword), pieces("\"open\nend"))
    }

    @Test
    fun theNameAfterFunctionIsTheFunctionsWhole() {
        assertEquals(listOf("function" to Lua.Keyword, "on_add" to Lua.Func), pieces("function on_add(task)"))
        assertEquals(listOf("function" to Lua.Keyword, "M.helper:run" to Lua.Func), pieces("function M.helper:run()"))
        assertEquals(listOf("function" to Lua.Keyword), pieces("function(x) return x end").take(1))
    }

    @Test
    fun numbersTakeAHexFormAFractionAndAnExponent() {
        assertEquals(listOf("0xFF" to Lua.Num, "3.5" to Lua.Num, "1e-3" to Lua.Num, ".5" to Lua.Num), pieces("0xFF 3.5 1e-3 .5"))
    }

    @Test
    fun aLineIsFoundByItsNumber() {
        assertEquals(0 to 3, lineRange("abc\ndef", 1))
        assertEquals(4 to 7, lineRange("abc\ndef", 2))
        assertEquals(null, lineRange("abc\ndef", 3))
        assertEquals(null, lineRange("abc\n\ndef", 2))
    }
}
