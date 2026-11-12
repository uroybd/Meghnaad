package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonObject
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

// The behaviour is the engine's (tc_core::keymap, tested there). These check that the app carries the answers across.
class KeymapTest {
    @Test
    fun theDefaultKeysAreCarried() {
        assertEquals(listOf("/"), resolveKeymap(JsonObject(emptyMap())).keys["app.prompt"])
        assertEquals(emptyMap(), resolveKeymap(JsonObject(emptyMap()), JsonObject(emptyMap())).keys)
    }

    @Test
    fun slashIsThePromptUnlessTheSettingsSayOtherwise() {
        val none = resolveKeymap(parseSettings("").raw)
        assertEquals(listOf("/"), none.keys["app.prompt"])
        assertEquals("app.prompt", none.action(keyChord("/", emptyList())))
        val own = resolveKeymap(parseSettings("""{ "keys": { "app.prompt": "/" }, "mobile": { "keys": { "app.prompt": "ctrl+k" } } }""").raw)
        assertNull(own.action(keyChord("/", emptyList())))
        assertEquals("app.prompt", own.action(keyChord("k", listOf("ctrl"))))
        assertEquals(emptyList(), own.problems)
    }

    @Test
    fun whatIsWrongIsSaidAndKeysAreShownAsHintsShowThem() {
        val km = resolveKeymap(parseSettings("""{ "mobile": { "keys": { "tasks.done": "d" } } }""").raw)
        assertEquals(listOf("mobile.keys: the mobile app has no “tasks.done”"), km.problems)
        assertEquals("Ctrl+K", keyLabel("ctrl+k"))
        assertEquals("down", keyChord("down", emptyList()))
    }
}
