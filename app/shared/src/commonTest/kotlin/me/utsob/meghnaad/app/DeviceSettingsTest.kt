package me.utsob.meghnaad.app

import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlin.random.Random
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertSame
import kotlin.test.assertTrue

// The same cases as web/src/lib/deviceSettings.test.ts.
class DeviceSettingsTest {
    private fun obj(text: String): JsonObject = engineJson.parseToJsonElement(text).jsonObject

    private val base = obj(
        """{"version":2,"homepage":{"report":"next"},"boards":[{"id":"week"}],
           "keys":{"tasks":{"x":"done","y":"start"}},"web":{"keys":{"app":{"ctrl+k":"app.prompt"}},"other":1}}""",
    )

    @Test
    fun nothingOfItsOwnKeepsEveryones() {
        assertSame(base, mergeDevice(base, null))
        assertEquals(base, mergeDevice(base, obj("{}")))
    }

    @Test
    fun theHomePageWholeAndTheKeysOneByOne() {
        val device = obj(
            """{"deviceId":"phone","homepage":{"board":"week"},"keys":{"tasks":{"x":null,"z":"done"}},
               "tui":{"keys":{"app":{"q":"app.quit"}}},"boards":[{"id":"mine"}]}""",
        )
        val out = mergeDevice(base, device)
        assertEquals(obj("""{"board":"week"}"""), out["homepage"])
        assertEquals(obj("""{"tasks":{"x":null,"y":"start","z":"done"}}"""), out["keys"])
        assertEquals(base["web"], out["web"])
        assertEquals(obj("""{"keys":{"app":{"q":"app.quit"}}}"""), out["tui"])
        assertEquals(base["boards"], out["boards"])
    }

    @Test
    fun onlyWhatDiffersIsTheDevicesKeys() {
        val edited = JsonObject(mergeDevice(base, obj("{}")) + ("keys" to obj("""{"tasks":{"y":"start","z":"done"}}""")))
        val device = withDeviceKeys(obj("""{"deviceId":"a"}"""), base, edited)
        assertEquals(obj("""{"deviceId":"a","keys":{"tasks":{"x":null,"z":"done"}}}"""), device)
        assertEquals(obj("""{"tasks":{"x":null,"y":"start","z":"done"}}"""), mergeDevice(base, device)["keys"])
        assertEquals(obj("""{"deviceId":"a"}"""), withDeviceKeys(device, base, mergeDevice(base, obj("{}"))))
    }

    @Test
    fun itsOwnHomePageMadeAndTakenAway() {
        val d = withDeviceHomepage(obj("""{"deviceId":"a"}"""), null)
        assertEquals(obj("""{"deviceId":"a","homepage":null}"""), d)
        assertTrue(hasOverrides(d))
        assertEquals(obj("""{"deviceId":"a"}"""), withDeviceHomepage(d, null, own = false))
        assertFalse(hasOverrides(obj("""{"deviceId":"a"}""")))
    }

    @Test
    fun remindersOverTheDefaultsAndCarriedIntoTheFirstFile() {
        assertNull(deviceReminders(obj("{}")))
        assertEquals(NotifySettings(enabled = true), deviceReminders(obj("""{"reminders":{"enabled":true,"leadMinutes":"x"}}""")))
        val had = NotifySettings(enabled = true, leadMinutes = 30)
        val file = deviceFile(obj("""{"homepage":null}"""), "b", base["version"], had)
        assertEquals(had, deviceReminders(file))
        assertEquals(obj("""{"enabled":false}"""), deviceFile(obj("""{"reminders":{"enabled":false}}"""), "b", null, had)["reminders"])
    }

    @Test
    fun idsAreFileNames() {
        assertNull(deviceIdProblem("laptop-2_b"))
        assertNotNull(deviceIdProblem(""))
        assertNotNull(deviceIdProblem("my laptop"))
        assertNotNull(deviceIdProblem("a".repeat(65)))
        assertNull(deviceIdProblem(newDeviceId(random = Random(1))))
    }
}
