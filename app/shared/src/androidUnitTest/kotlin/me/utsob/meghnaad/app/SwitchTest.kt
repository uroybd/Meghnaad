package me.utsob.meghnaad.app

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.yield
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Moving a phone's tasks to another place: the person is asked first, and nothing is kept that did not work. */
class SwitchTest {
    private val clock = object : Clock {
        override fun utcOffsetSeconds() = 0

        override fun offsetAt(epochSeconds: Long) = 0

        override fun nowSeconds() = 1000.0
    }
    private val id = "3f2b8d1e-9c4a-4b7e-8f21-5a6d0c9e1b34"
    private val one = Connection.typedServer("https://one.example.com", id, "secret")
    private val moved = "backend-changed: the tasks on this phone were synced with the sync server at https://one.example.com, " +
        "and the settings now point at the sync server at https://two.example.com"

    private class Kept(var connection: Connection?) : Vault {
        override fun engineKey() = ByteArray(32) { 7 }

        override fun connection() = connection

        override fun saveConnection(connection: Connection) {
            this.connection = connection
        }

        override fun clearConnection() {
            connection = null
        }

        override var localOnly = false
        override var syncOptions = SyncOptions()
    }

    private fun setup(onSync: (String?) -> SyncResult): Triple<FakePort, Kept, Store> {
        val vault = Kept(one)
        lateinit var port: FakePort
        port = FakePort(onSync = { onSync(port.lastSwitch) })
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val store = Store(Tw(port, clock), port, vault, SyncController(port, scope, clock), scope, clock)
        return Triple(port, vault, store)
    }

    private fun two() = Connection.typedServer("https://two.example.com", id, "secret")

    @Test
    fun theTasksMoveOnlyWhenThePersonSaysHow() = runBlocking {
        val (port, vault, store) = setup { switch -> if (switch == null) throw SyncFailed(moved) else SyncResult(true) }
        val change = async { store.changeServer("https://two.example.com", "", "", "", "", "", "", "") }
        while (store.switchAsk == null) yield()
        val question = store.switchAsk!!.message
        assertTrue("sync server at https://one.example.com" in question && question.endsWith("."), question)
        store.answerSwitch("merge")
        assertNull(change.await())
        assertEquals("merge", port.lastSwitch)
        assertEquals(two(), vault.connection)
        assertNull(store.switchAsk)
    }

    @Test
    fun leavingItAsItWasKeepsTheConnectionThatWorked() = runBlocking {
        val (port, vault, store) = setup { switch -> if (switch == null) throw SyncFailed(moved) else SyncResult(true) }
        val change = async { store.changeServer("https://two.example.com", "", "", "", "", "", "", "") }
        while (store.switchAsk == null) yield()
        store.answerSwitch(null)
        assertTrue(change.await()!!.startsWith("Nothing was changed"))
        assertNull(port.lastSwitch)
        assertEquals(one, vault.connection)
    }

    @Test
    fun otherFailuresAreNotQuestions() = runBlocking {
        val (_, vault, store) = setup { throw SyncFailed("could not reach the sync server: timed out") }
        assertEquals("could not reach the sync server: timed out", store.changeServer("https://two.example.com", "", "", "", "", "", "", ""))
        assertNull(store.switchAsk)
        assertEquals(one, vault.connection)
    }

    @Test
    fun aSwitchThatFailsAfterBeingChosenPutsTheOldConnectionBack() = runBlocking {
        val (_, vault, store) = setup { switch -> throw SyncFailed(if (switch == null) moved else "status 403") }
        val change = async { store.changeServer("https://two.example.com", "", "", "", "", "", "", "") }
        while (store.switchAsk == null) yield()
        store.answerSwitch("replace")
        assertNotNull(change.await())
        assertEquals(one, vault.connection)
    }
}
