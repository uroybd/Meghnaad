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
        val b = keymapBindings(settings("""{ "version": 2, "tui": { "keys": { "tasks": { "x": "tasks.done" } } } }"""))
        assertEquals(listOf("tasks.done"), bindingRows(b.sections["tui"].orEmpty(), false, "").map { it.action })
        assertEquals(emptyList(), bindingRows(b.sections["general"].orEmpty(), false, "").map { it.action })
        val web = bindingRows(b.sections["web"].orEmpty(), true, "")
        assertTrue(web.any { it.action == "app.add" && it.keys == null && it.defaults == listOf("a") })
    }

    @Test
    fun theSearchMatchesANameWhatItDoesOrAKey() {
        val b = keymapBindings(settings("""{ "version": 2, "keys": { "app": { "ctrl+k": "app.prompt" } }, "tui": { "keys": { "tasks": { "x": "tasks.done", "z": "tasks.start" } } } }"""))
        assertEquals(listOf("tasks.start"), bindingRows(b.sections["tui"].orEmpty(), false, "start").map { it.action })
        assertEquals(listOf("app.prompt"), bindingRows(b.sections["general"].orEmpty(), false, "Ctrl+K").map { it.action })
    }

    @Test
    fun aKeyIsSetAndTakenOutAndTheRestStaysInItsOrder() {
        val s = settings("""{ "version": 2, "taskCommands": [], "appearance": {} }""")
        val one = withBinding(s, "web", "app.add", listOf("ctrl+shift+n"))
        assertEquals(listOf("version", "taskCommands", "appearance", "web"), one.keys.toList())
        val app = one["web"]!!.jsonObject["keys"]!!.jsonObject["app"]!!.jsonObject
        assertEquals(JsonPrimitive("app.add"), app["ctrl+shift+n"])
        assertEquals(s, withBinding(one, "web", "app.add", null))
        assertEquals(settings("""{ "version": 2 }"""), withBinding(JsonObject(emptyMap()), "mobile", "app.prompt", null))
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
}
