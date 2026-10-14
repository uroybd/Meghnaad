package me.utsob.meghnaad.app

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class AppSettingsTest {
    @Test
    fun readsTheTaskCommands() {
        val p = parseSettings(
            """{ "taskCommands": [ { "name": "fail", "command": "done outcome:fail", "icon": "circle-x" }, { "name": "late", "command": " +OVERDUE list " } ] }""",
        )
        assertEquals(emptyList(), p.errors)
        assertEquals(listOf(TaskCommand("fail", "done outcome:fail", "circle-x"), TaskCommand("late", "+OVERDUE list")), p.commands)
    }

    @Test
    fun takesNothingOrAnObjectWithNoCommandsAsNoCommands() {
        for (t in listOf("", "  \n", "{}", """{ "other": 1 }""")) {
            val p = parseSettings(t)
            assertEquals(emptyList(), p.commands, t)
            assertEquals(emptyList(), p.errors, t)
        }
    }

    @Test
    fun saysWhatIsWrongAndKeepsWhatIsRight() {
        val p = parseSettings(
            """{"taskCommands":[
              {"name":"ok","command":"done"},
              {"name":"two words","command":"done"},
              {"name":"a:b","command":"done"},
              {"name":"12","command":"done"},
              {"name":"nocmd"},
              {"name":"badicon","command":"done","icon":"Circle X"},
              {"name":"ok","command":"stop"},
              "text",
              {"command":"done"}
            ]}""",
        )
        assertEquals(listOf(TaskCommand("ok", "done")), p.commands)
        assertEquals(8, p.errors.size)
        val all = p.errors.joinToString("\n")
        assertTrue(Regex("two words.*one word").containsMatchIn(all))
        assertTrue("needs a command" in all)
        assertTrue("Lucide icon name" in all)
        assertTrue("already a command named ok" in all)
    }

    @Test
    fun refusesWhatIsNotAJsonObject() {
        assertTrue(parseSettings("{ taskCommands: [] }").errors[0].startsWith("Not valid JSON"))
        assertTrue("must be a JSON object" in parseSettings("[]").errors[0])
        assertTrue("must be a list" in parseSettings("""{ "taskCommands": 3 }""").errors[0])
    }

    @Test
    fun writesTheCommandsAndKeepsEveryOtherKey() {
        val p = parseSettings("""{ "future": { "a": 1 }, "taskCommands": [ { "name": "x", "command": "done" } ] }""")
        val text = serializeSettings(p.raw, listOf(TaskCommand("fail", "done outcome:fail", "circle-x")))
        val back = Json.parseToJsonElement(text).jsonObject
        assertEquals("""{"a":1}""", back["future"].toString())
        assertEquals("""[{"name":"fail","command":"done outcome:fail","icon":"circle-x"}]""", back["taskCommands"].toString())
        assertTrue(text.endsWith("}\n"))
        assertTrue("\n  \"future\"" in text) // two spaces, as the web writes it
    }

    @Test
    fun leavesTheKeyOutWhenThereAreNoneAndReadsBackWhatItWrote() {
        assertEquals(JsonObject(emptyMap()), Json.parseToJsonElement(serializeSettings(parseSettings("""{"taskCommands":[{}]}""").raw, emptyList())))
        val cmds = listOf(TaskCommand("a", "b c"))
        assertEquals(cmds, parseSettings(serializeSettings(JsonObject(emptyMap()), cmds)).commands)
    }

    @Test
    fun acceptsAWordAndRefusesWhatCouldBeAFilter() {
        for (n in listOf("fail", "done2", "my_cmd", "x.y")) assertNull(nameProblem(n), n)
        for (n in listOf("", "a b", "+x", "-x", "a:b", "a=b", "5", "1,2", "/x", "task", "--")) assertNotNull(nameProblem(n), n)
    }
}
