package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.jsonObject
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

// The Keybindings tab (KeysPanel.test.ts): what is the engine's is checked there; these check what the tab does with it.
class KeysPanelTest {
    private fun settings(text: String) = parseSettings(text).raw

    @Test
    fun onlyYourOwnKeysUntilTheDefaultsAreAskedFor() {
        val b = keymapBindings(settings("""{ "version": 6, "tui": { "keys": { "tasks": { "x": "tasks.done" } } } }"""))
        assertEquals(listOf("tasks.done"), bindingRows(b.sections["tui"].orEmpty(), false, "").map { it.action })
        assertEquals("done", b.sections["tui"].orEmpty().first().command, "the command it runs")
        assertEquals(emptyList(), bindingRows(b.sections["general"].orEmpty(), false, "").map { it.action })
        val web = bindingRows(b.sections["web"].orEmpty(), true, "")
        assertTrue(web.any { it.action == "app.add" && it.keys == null && it.defaults == listOf("a") })
    }

    @Test
    fun theSearchMatchesANameWhatItDoesOrAKey() {
        val b = keymapBindings(settings("""{ "version": 6, "keys": { "app": { "ctrl+k": "app.prompt" } }, "tui": { "keys": { "tasks": { "x": "tasks.done", "z": "tasks.start" } } } }"""))
        assertEquals(listOf("tasks.start"), bindingRows(b.sections["tui"].orEmpty(), false, "start").map { it.action })
        assertEquals(listOf("app.prompt"), bindingRows(b.sections["general"].orEmpty(), false, "Ctrl+K").map { it.action })
    }

    @Test
    fun aKeyIsSetAndTakenOutAndTheRestStaysInItsOrder() {
        val s = settings("""{ "version": 6, "taskCommands": [], "appearance": {} }""")
        val one = withBinding(s, "web", "app.add", listOf("ctrl+shift+n"))
        assertEquals(listOf("version", "taskCommands", "appearance", "web"), one.keys.toList())
        val app = one["web"]!!.jsonObject["keys"]!!.jsonObject["app"]!!.jsonObject
        assertEquals(JsonPrimitive("app.add"), app["ctrl+shift+n"])
        assertEquals(s, withBinding(one, "web", "app.add", null))
        assertEquals(settings("""{ "version": 6 }"""), withBinding(JsonObject(emptyMap()), "mobile", "app.prompt", null))
    }

    @Test
    fun anOldFileIsBroughtUpToDateOnce() {
        val next = migrateSettings(settings("""{ "web": { "keys": { "app.help": "h" } } }"""))!!
        assertEquals(JsonPrimitive("app.help"), next["web"]!!.jsonObject["keys"]!!.jsonObject["app"]!!.jsonObject["h"])
        assertNull(migrateSettings(next))
    }

    @Test
    fun theCommandsAreListedAndSearchedByTheirWholeName() {
        val all = keyCommands(version = "998.0.0")
        assertTrue(all.any { it.written == ":tabn[ext]" && "takes" in it.what && it.link.endsWith("#tabnext") })
        assertEquals(listOf(":den[otate]"), commandRows(all, "denot").map { it.written })
        assertEquals(all, commandRows(all, ""))
        assertTrue(haveSince("unreleased", "0.1.0"))
        assertTrue(!haveSince("999.0.0", "0.8.10"))
        assertTrue(haveSince("0.8.10", "0.8.10"))
    }

    @Test
    fun aTypedKeyIsReadOrSaidWhatIsWrong() {
        assertEquals("ctrl+shift+k" to null, parseKey("Ctrl-K"))
        val (key, problem) = parseKey("hyper+q")
        assertNull(key)
        assertTrue(problem!!.contains("is not a key"))
    }

    @Test
    fun aCommandIsCheckedAndSavedAsTyped() {
        assertEquals("done takes no words", checkCommand("done 3").problem)
        val c = checkCommand("filter +home")
        assertEquals(Triple("tasks.filter", listOf("+home"), null as String?), Triple(c.action, c.args, c.problem))
        assertEquals(listOf(KeyPlace("tasks", "tasks.filter")), c.places)
        val s = settings("""{ "version": 6 }""")
        val one = withBinding(s, "general", "filter +home", listOf("g h"), group = "tasks")
        assertEquals(JsonPrimitive("filter +home"), one["keys"]!!.jsonObject["tasks"]!!.jsonObject["g h"])
        assertEquals(listOf("filter +home"), keymapBindings(one).sections["general"].orEmpty().filter { it.keys != null }.map { it.action })
    }

    @Test
    fun aCommandOfSeveralPlacesIsBoundWhereChosen() {
        val c = checkCommand("next")
        assertEquals(listOf("calendar", "boards", "kanban"), c.places.map { it.group })
        val out = withBinding(settings("""{ "version": 6 }"""), "tui", "next", listOf("N"), group = "boards")
        assertEquals(JsonPrimitive("next"), out["tui"]!!.jsonObject["keys"]!!.jsonObject["boards"]!!.jsonObject["N"])
    }

    @Test
    fun aKeyThatRunsNoCommandIsWarnedOfAndTakenOut() {
        val s = settings("""{ "version": 6, "keys": { "tasks": { "z": "done 3", "x": "done" } } }""")
        val bad = keymapBindings(s).bad.single()
        assertEquals("keys.tasks" to "done takes no words", bad.at to bad.why)
        val out = withoutKey(s, bad)
        assertEquals(settings("""{ "version": 6, "keys": { "tasks": { "x": "done" } } }"""), out)
        assertTrue(keymapBindings(out).bad.isEmpty())
    }
}
