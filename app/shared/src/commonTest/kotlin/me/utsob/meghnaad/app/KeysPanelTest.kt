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
        val b = keymapBindings(settings("""{ "tui": { "keys": { "tasks.done": "x" } } }"""))
        assertEquals(listOf("tasks.done"), bindingRows(b.sections["tui"].orEmpty(), false, "").map { it.action })
        assertEquals(emptyList(), bindingRows(b.sections["general"].orEmpty(), false, "").map { it.action })
        val web = bindingRows(b.sections["web"].orEmpty(), true, "")
        assertTrue(web.any { it.action == "app.add" && it.keys == null && it.defaults == listOf("a") })
    }

    @Test
    fun theSearchMatchesANameWhatItDoesOrAKey() {
        val b = keymapBindings(settings("""{ "keys": { "app.prompt": "ctrl+k" }, "tui": { "keys": { "tasks.done": "x", "tasks.start": "z" } } }"""))
        assertEquals(listOf("tasks.start"), bindingRows(b.sections["tui"].orEmpty(), false, "start").map { it.action })
        assertEquals(listOf("app.prompt"), bindingRows(b.sections["general"].orEmpty(), false, "Ctrl+K").map { it.action })
    }

    @Test
    fun aKeyIsSetAndTakenOutAndTheRestStaysInItsOrder() {
        val s = settings("""{ "taskCommands": [], "appearance": {} }""")
        val one = withBinding(s, "web", "app.add", listOf("ctrl+shift+n"))
        assertEquals(listOf("taskCommands", "appearance", "web"), one.keys.toList())
        assertEquals(JsonPrimitive("ctrl+shift+n"), one["web"]!!.jsonObject["keys"]!!.jsonObject["app.add"])
        assertEquals(s, withBinding(one, "web", "app.add", null))
        assertEquals(JsonObject(emptyMap()), withBinding(JsonObject(emptyMap()), "mobile", "app.prompt", null))
    }

    @Test
    fun aTypedKeyIsReadOrSaidWhatIsWrong() {
        assertEquals("ctrl+shift+k" to null, parseKey("Ctrl-K"))
        val (key, problem) = parseKey("hyper+q")
        assertNull(key)
        assertTrue(problem!!.contains("is not a key"))
    }
}
