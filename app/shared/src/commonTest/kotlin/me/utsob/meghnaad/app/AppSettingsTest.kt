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

    @Test
    fun aCommandMayBeSeveralCommandsAndOneWithNothingOnASideOfAnOperatorIsReportedAndLeftOut() {
        val p = parseSettings("""{ "taskCommands": [ { "name": "wrap", "command": "done && modify +failed" }, { "name": "bad", "command": "done &&" } ] }""")
        assertEquals(listOf(TaskCommand("wrap", "done && modify +failed")), p.commands)
        assertEquals(listOf("taskCommands[1] (bad): Nothing after ‘&&’."), p.errors)
        assertNull(commandProblem("done ; modify +x"))
        assertEquals("Nothing before ‘||’.", commandProblem("|| done"))
        assertEquals("needs a command", commandProblem("  "))
    }

    @Test
    fun boardsAndTheHomePageAreReadWithTheRestOfTheFile() {
        val board = """{"id":"week","name":"Week","sections":[{"id":"s","type":"number","x":0,"y":0,"w":3,"h":2}]}"""
        val p = parseSettings("""{"boards":[$board,{"id":"Bad Id","name":"x"}],"homepage":{"board":"week"},"taskCommands":[{"name":"fail","command":"done"}]}""")
        assertEquals(listOf("week"), p.boards.map { it.id })
        // The boards' problems are kept apart from the commands'.
        assertEquals(1, p.boardErrors.size)
        assertEquals(emptyList(), p.errors)
        assertEquals(Homepage(board = "week"), p.homepage)
        assertEquals(listOf("fail"), p.commands.map { it.name })
        assertEquals(emptyList(), parseSettings("{}").boards)
    }

    @Test
    fun aHomePageIsAReportOrABoardOneOfTheTwo() {
        fun home(json: String) = parseHomepage(Json.parseToJsonElement(json))
        assertEquals(Homepage(report = "list"), home("""{"report":" list "}"""))
        assertEquals(Homepage(board = "week"), home("""{"board":"week"}"""))
        for (bad in listOf("""{}""", """{"report":""}""", """{"report":3}""", """{"report":"a","board":"b"}""", """{"report":"a","x":1}""", "\"next\"", "null")) {
            assertNull(home(bad), bad)
        }
    }

    @Test
    fun boardsAndTheHomePageAreSavedBesideTheOtherKeysAndSurviveSavingTheCommands() {
        val raw = parseSettings("""{"taskCommands":[{"name":"fail","command":"done"}],"other":1,"homepage":{"report":"list"}}""").raw
        val board = Board("week", "Week", sections = emptyList())
        // The home page is left as it was unless asked.
        val kept = Json.parseToJsonElement(serializeBoards(raw, listOf(board))) as kotlinx.serialization.json.JsonObject
        assertEquals(setOf("taskCommands", "other", "homepage", "boardsVersion", "boards"), kept.keys)
        // Or moved with a board whose slug changed, or taken out.
        val moved = Json.parseToJsonElement(serializeBoards(raw, listOf(board), Homepage(board = "weekly"), setHomepage = true)).toString()
        assertTrue("\"board\":\"weekly\"" in moved.replace(" ", ""), moved)
        val none = Json.parseToJsonElement(serializeBoards(raw, emptyList(), null, setHomepage = true)) as kotlinx.serialization.json.JsonObject
        assertEquals(setOf("taskCommands", "other"), none.keys)

        val home = Json.parseToJsonElement(serializeHomepage(raw, Homepage(board = "week"))) as kotlinx.serialization.json.JsonObject
        assertEquals(setOf("taskCommands", "other", "homepage"), home.keys)
        val cmds = Json.parseToJsonElement(serializeSettings(parseSettings(serializeBoards(raw, listOf(board))).raw, emptyList())) as kotlinx.serialization.json.JsonObject
        assertTrue("boards" in cmds.keys && "homepage" in cmds.keys, cmds.keys.toString())
    }
}
