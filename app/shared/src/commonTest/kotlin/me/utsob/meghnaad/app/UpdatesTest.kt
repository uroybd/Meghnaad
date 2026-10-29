package me.utsob.meghnaad.app

import kotlinx.coroutines.runBlocking
import kotlin.test.AfterTest
import kotlin.test.BeforeTest
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

// Looking for a newer version (Updates.kt): what GitHub's latest release says, which version is later, and when the app asks.
class UpdatesTest {
    private class FixedClock(var now: Double = 1_000_000.0) : Clock {
        override fun utcOffsetSeconds() = 0

        override fun offsetAt(epochSeconds: Long) = 0

        override fun nowSeconds() = now
    }

    @BeforeTest
    fun set() {
        UPDATE_REPO = "me/meghnaad"
        APP_VERSION = "0.6.35"
    }

    @AfterTest
    fun reset() {
        UPDATE_REPO = ""
        APP_VERSION = ""
    }

    private fun release(tag: String, vararg assets: Pair<String, String>) = buildString {
        append("""{"tag_name":"$tag","html_url":"https://github.com/me/meghnaad/releases/tag/$tag","assets":[""")
        append(assets.joinToString(",") { """{"name":"${it.first}","browser_download_url":"${it.second}"}""" })
        append("]}")
    }

    private val apk = "meghnaad-0.6.36.apk" to "https://github.com/me/meghnaad/releases/download/v0.6.36/meghnaad-0.6.36.apk"

    @Test
    fun theLatestReleaseHasAVersionAnApkAndAPage() {
        val r = parseRelease(release("v0.6.36", "meghnaad-0.6.36.apk.sha256" to "https://x/y.sha", apk))!!
        assertEquals("0.6.36", r.version)
        assertEquals(apk.second, r.apkUrl)
        assertEquals("https://github.com/me/meghnaad/releases/tag/v0.6.36", r.pageUrl)
        // A release whose APK is not attached yet has none; a debug-signed one is only the second choice.
        assertNull(parseRelease(release("v0.6.36"))!!.apkUrl)
        val both = parseRelease(release("v0.6.36", "meghnaad-0.6.36-debug.apk" to "https://x/debug.apk", apk))!!
        assertEquals(apk.second, both.apkUrl)
        assertEquals("https://x/debug.apk", parseRelease(release("v0.6.36", "meghnaad-0.6.36-debug.apk" to "https://x/debug.apk"))!!.apkUrl)
    }

    @Test
    fun onlyAnHttpsAddressIsEverOpened() {
        assertNull(parseRelease(release("v0.6.36", "a.apk" to "http://x/a.apk"))!!.apkUrl)
        assertNull(parseRelease(release("v0.6.36", "a.apk" to "intent://x/a.apk"))!!.apkUrl)
        assertNull(parseRelease("""{"tag_name":"v1.0.0","html_url":"javascript:alert(1)","assets":[]}"""))
    }

    @Test
    fun whatIsNotAReleaseIsNotOne() {
        for (text in listOf("", "[]", "{}", """{"message":"Not Found"}""", """{"tag_name":"","html_url":"https://x"}""")) assertNull(parseRelease(text), text)
    }

    @Test
    fun aLaterVersionIsNewer() {
        assertTrue(isNewer("0.6.36", "0.6.35"))
        assertTrue(isNewer("0.7.0", "0.6.99"))
        assertTrue(isNewer("1.0.0", "0.99.99"))
        assertTrue(isNewer("0.6.10", "0.6.9"))
        assertFalse(isNewer("0.6.35", "0.6.35"))
        assertFalse(isNewer("0.6.34", "0.6.35"))
        // A release is later than a build made for it before the tag, and a build of a version is not later than the release.
        assertTrue(isNewer("0.6.36", "0.6.36-dev.abc1234"))
        assertFalse(isNewer("0.6.36-rc1", "0.6.36"))
        assertFalse(isNewer("soon", "0.6.35"))
        assertFalse(isNewer("0.6.36", "unknown"))
    }

    @Test
    fun anAnswerIsJudgedAgainstTheVersionInUse() {
        val ok = { body: String, status: Int -> judge(HttpAnswer(status, body), "0.6.35") }
        assertEquals("0.6.36", (ok(release("v0.6.36", apk), 200) as UpdateStatus.Available).release.version)
        assertEquals(UpdateStatus.UpToDate("0.6.35"), ok(release("v0.6.35"), 200))
        assertEquals(UpdateStatus.Failed("No release has been published yet."), ok("", 404))
        assertEquals(UpdateStatus.Failed("GitHub answered 403."), ok("", 403))
        assertEquals(UpdateStatus.Failed("The answer was not a release."), ok("<html>", 200))
    }

    @Test
    fun theAppAsksOnceADayAndSaysAVersionOnce() = runBlocking {
        val clock = FixedClock()
        val asked = mutableListOf<String>()
        val updates = UpdateState(MemoryPrefs(), clock) { url, _ ->
            asked += url
            HttpAnswer(200, release("v0.6.36", apk))
        }
        val first = updates.checkWhenDue()
        assertEquals("0.6.36", first?.version)
        assertEquals(listOf("https://api.github.com/repos/me/meghnaad/releases/latest"), asked)
        // Not again the same day, and not again for the same version.
        assertNull(updates.checkWhenDue())
        assertEquals(1, asked.size)
        clock.now += 25 * 3600
        assertNull(updates.checkWhenDue())
        assertEquals(2, asked.size)
        // Its status stays, for Settings to show.
        assertTrue(updates.status is UpdateStatus.Available)
    }

    @Test
    fun aCheckThatCannotBeMadeIsAStatusNotACrash() = runBlocking {
        val updates = UpdateState(MemoryPrefs(), FixedClock()) { _, _ -> throw IllegalStateException("no network") }
        updates.check()
        assertEquals(UpdateStatus.Failed("no network"), updates.status)
        assertNull(updates.checkWhenDue())
    }

    @Test
    fun aBuildWithNoRepositoryDoesNotCheck() = runBlocking {
        UPDATE_REPO = ""
        var asked = false
        val updates = UpdateState(MemoryPrefs(), FixedClock()) { _, _ ->
            asked = true
            HttpAnswer(200, "")
        }
        assertFalse(updates.canCheck)
        updates.check()
        assertNull(updates.checkWhenDue())
        assertFalse(asked)
        assertEquals(UpdateStatus.Idle, updates.status)
    }
}
