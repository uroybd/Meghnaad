package me.utsob.meghnaad.app

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Spec 0009: saving keeps what this app does not use, tested against the cases the web app and the terminal app share. */
class SettingsKeptTest {
    private val fixture: JsonObject by lazy {
        var dir: File? = File(System.getProperty("user.dir")).absoluteFile
        while (dir != null && !File(dir, "fixtures/settings-kept.json").exists()) dir = dir.parentFile
        Json.parseToJsonElement(File(dir, "fixtures/settings-kept.json").readText()).jsonObject
    }

    /** The same value, whatever order its keys are in. */
    private fun same(a: JsonElement?, b: JsonElement?): Boolean = when {
        a is JsonObject && b is JsonObject -> a.keys == b.keys && a.keys.all { same(a[it], b[it]) }
        a is JsonArray && b is JsonArray -> a.size == b.size && a.indices.all { same(a[it], b[it]) }
        else -> a == b
    }

    private fun at(v: JsonElement?, path: JsonArray): JsonElement? = path.fold(v) { o, k ->
        val key = k.jsonPrimitive
        if (key.isString) (o as? JsonObject)?.get(key.content) else (o as? JsonArray)?.getOrNull(key.int)
    }

    @Test
    fun theMergeIsTheOneEveryFrontEndMakes() {
        for (c in fixture["merge"]!!.jsonArray) {
            val o = c.jsonObject
            val got = mergeSettings(o["old"]!!, o["base"], o["next"]!!)
            assertTrue(same(o["expect"], got), "${o["name"]}: $got")
        }
    }

    @Test
    fun everyUnknownKeyAndUnusableEntryIsKeptThroughEachKindOfSave() {
        val file = fixture["file"]!!.jsonObject
        val read = parseSettings(file.toString())
        assertEquals(1, read.errors.size, "the task command named “two words”")
        assertEquals(1, read.boardErrors.size, "the board with a bad id")
        val saves = mapOf(
            "commands" to serializeSettings(read.raw, read.commands + TaskCommand("late", "+OVERDUE list")),
            "commands, one edited" to serializeSettings(read.raw, listOf(TaskCommand("fail", "done outcome:failed"))),
            "boards" to serializeBoards(read.raw, read.boards),
            "boards, one renamed" to serializeBoards(read.raw, listOf(read.boards[0].copy(name = "This week"))),
            "homepage" to serializeHomepage(read.raw, Homepage(board = "week")),
            "appearance" to serializeAppearance(read.raw, read.appearance.copy(progress = read.appearance.progress + ("pct" to Progress(0.0, 1.0)))),
            "appearance, cleared" to serializeAppearance(read.raw, Appearance()),
        )
        for ((what, text) in saves) {
            val out = Json.parseToJsonElement(text)
            for (path in fixture["kept"]!!.jsonArray.map { it.jsonArray }) {
                // Clearing the appearance takes the transformation it understood, with what was in it.
                if (what == "appearance, cleared" && path.getOrNull(1) == JsonPrimitive("transformations")) continue
                assertTrue(same(at(file, path), at(out, path)), "$what: $path in $text")
            }
        }
        val edited = at(Json.parseToJsonElement(saves.getValue("commands, one edited")), Json.parseToJsonElement("""["taskCommands",0]""").jsonArray)
        assertTrue(same(Json.parseToJsonElement("""{"name":"fail","command":"done outcome:failed","x-kept":2}"""), edited), "$edited")
        val cleared = Json.parseToJsonElement(saves.getValue("appearance, cleared")).jsonObject["appearance"]
        assertTrue(same(Json.parseToJsonElement("""{"x-kept":true}"""), cleared), "$cleared")
    }

    @Test
    fun theLintStopsAtAKnownKeyInTheWrongShapeAndListsTheRest() {
        assertTrue(lintSettings("{ nope").errors.single().startsWith("Not valid JSON"))
        assertTrue(lintSettings("[1]").errors.single().contains("must be a JSON object"))
        assertEquals(Lint(emptyList(), emptyList()), lintSettings(""))
        val wrong = lintSettings("""{"homepage":"next","appearance":{"progress":[],"transformations":{}},"taskCommands":{},"boards":[3,{"id":"week","name":"Week"}],"tui":[]}""")
        assertEquals(
            listOf("homepage must be an object", "tui must be an object", "taskCommands must be a list"),
            wrong.errors.take(3),
        )
        assertTrue(wrong.errors[3].startsWith("appearance.progress must be an object"))
        assertTrue(wrong.errors[4].startsWith("appearance.transformations must be a list"))
        assertEquals(listOf("boards[0] must be an object", "boards[1] (week): needs a sections list"), wrong.errors.drop(5))
        val lint = lintSettings(fixture["file"].toString()) { listOf("keys: no action is called app.nope") }
        assertEquals(emptyList(), lint.errors)
        assertEquals(
            listOf(
                "taskCommands[1] (two words): a name is one word (no spaces)",
                "board 'Not An Id': needs an id of lower case letters, digits, - and _",
                "keys: no action is called app.nope",
            ),
            lint.warnings,
        )
    }
}
