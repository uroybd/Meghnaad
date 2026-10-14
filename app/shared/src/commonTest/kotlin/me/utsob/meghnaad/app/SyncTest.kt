package me.utsob.meghnaad.app

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class SyncTest {
    private class FixedClock(var now: Double = 1000.0) : Clock {
        override fun utcOffsetSeconds() = 0

        override fun nowSeconds() = now
    }

    private val scope = CoroutineScope(Dispatchers.Unconfined)

    @Test
    fun aSyncThatWorksClearsTheError() = runBlocking {
        val clock = FixedClock()
        var fail = true
        val port = FakePort(onSync = { if (fail) throw SyncFailed("Server Error: could not reach the server") else SyncResult(false) })
        val sync = SyncController(port, scope, clock)
        assertTrue(sync.request().await().isFailure)
        assertEquals(SyncStatus.Failed, sync.status)
        assertEquals("could not reach the server", sync.error)
        fail = false
        assertTrue(sync.request().await().isSuccess)
        assertEquals(SyncStatus.Idle, sync.status)
        assertNull(sync.error)
        assertEquals(1000.0, sync.lastSync)
    }

    @Test
    fun aSyncAskedForWhileOneRunsRunsOnceMore() = runBlocking {
        val gate = CompletableDeferred<Unit>()
        val port = FakePort(onSync = {
            if (gate.isActive) gate.await()
            SyncResult(false)
        })
        val sync = SyncController(port, scope, FixedClock())
        val first = sync.request()
        // Two more while it runs: they are one more sync, not two.
        sync.request()
        sync.request()
        gate.complete(Unit)
        first.await()
        assertEquals(2, port.syncs)
    }

    @Test
    fun changesAndSettingsAreToldAbout() = runBlocking {
        val port = FakePort(onSync = { SyncResult(true, listOf("web/config.json")) })
        val sync = SyncController(port, scope, FixedClock())
        var changed = 0
        var settings = 0
        sync.onChanged = { changed++ }
        sync.onSettingsChanged = { settings++ }
        sync.request().await()
        assertEquals(1, changed)
        assertEquals(1, settings)
    }

    @Test
    fun aPullRightAfterAnotherIsSkipped() = runBlocking {
        val clock = FixedClock()
        val port = FakePort()
        val sync = SyncController(port, scope, clock)
        sync.request().await()
        clock.now += 2
        sync.pullIfStale()
        assertEquals(1, port.syncs)
        clock.now += 10
        sync.pullIfStale()
        assertEquals(2, port.syncs)
    }

    @Test
    fun aConnectionIsTypedAsItIsMeant() {
        val c = Connection.typed(" https://x.example/ ", " tasks ", " ", " key ", " secret ", " pw ")
        assertEquals("https://x.example", c.endpoint)
        assertEquals("tasks", c.bucket)
        assertEquals("auto", c.region)
        assertNull(c.problem())
        assertNotNull(Connection.typed("", "b", "", "k", "s", "p").problem())
        val json = c.toSyncJson(SyncOptions(settings = true, hooks = true).files())
        assertTrue("\"access_key\":\"key\"" in json && "web/hooks.lua" in json && "web/settings.json" in json)
    }
}
