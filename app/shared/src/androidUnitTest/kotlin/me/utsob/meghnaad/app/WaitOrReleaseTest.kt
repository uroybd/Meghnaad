package me.utsob.meghnaad.app

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** A form waits for a quick piece of work, and lets go of a slow one (a hook) with a toast that says it is being made (waitOrRelease in store.svelte.ts). */
class WaitOrReleaseTest {
    private val clock = object : Clock {
        override fun utcOffsetSeconds() = 0

        override fun offsetAt(epochSeconds: Long) = 0

        override fun nowSeconds() = 1000.0
    }

    private val vault = object : Vault {
        override fun engineKey() = ByteArray(32) { 7 }

        override fun connection(): Connection? = null

        override fun saveConnection(connection: Connection) {}

        override fun clearConnection() {}

        override var localOnly = true
        override var syncOptions = SyncOptions()
    }

    private fun withStore(block: suspend (Store) -> Unit) = runBlocking {
        val port = FakePort(answer = { _, _ -> "{}" })
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        block(Store(Tw(port, clock), port, vault, SyncController(port, scope, clock), scope, clock))
    }

    @Test
    fun aQuickPieceOfWorkIsWaitedForWithTheFormStillThere() = withStore { store ->
        var released = false
        val got = store.waitOrRelease("Creating…", { released = true }, afterMs = 500) { delay(10); 7 }
        assertEquals(Released(7, false), got)
        assertFalse(released)
        assertNull(store.toast)
    }

    @Test
    fun aSlowOneLetsTheFormGoSaysSoAndTakesTheToastBackWhenItIsSilent() = withStore { store ->
        var releasedWhileToastWas: Toast? = null
        val got = store.waitOrRelease("Creating…", { releasedWhileToastWas = store.toast }, afterMs = 30) {
            delay(250)
            "late"
        }
        assertEquals(Released("late", true), got)
        // The form was let go before the toast went up, and the toast was the busy one, which no timer takes down.
        assertNull(releasedWhileToastWas)
        assertNull(store.toast)
    }

    @Test
    fun theBusyToastIsUpWhileASlowOneRunsAndTheOutcomeItAnnouncesStays() = withStore { store ->
        var during: Toast? = null
        val got = store.waitOrRelease("Creating…", {}, afterMs = 30) {
            delay(100)
            during = store.toast
            store.notify("Created task 7.")
            1
        }
        assertEquals(1, got.value)
        assertTrue(during?.busy == true && during?.text == "Creating…", "$during")
        assertEquals("Created task 7.", store.toast?.text)
        assertFalse(store.toast?.busy == true)
    }
}
