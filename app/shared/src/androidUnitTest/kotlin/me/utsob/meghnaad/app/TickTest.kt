package me.utsob.meghnaad.app

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.runBlocking
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

/** Ticking a box in a string UDA is an ordinary `modify <uda>:<text>` of the whole new text. */
class TickTest {
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

    @Test
    fun theCommandIsAModifyWithTheWholeChangedText() = runBlocking {
        val sent = mutableListOf<String>()
        val port = FakePort(
            answer = { op, arg ->
                if (op == "run") sent += arg
                """{"result":{"kind":"changed","message":"ok","tasks":[]},"wrote":false,"feedback":[]}"""
            },
        )
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val store = Store(Tw(port, clock), port, vault, SyncController(port, scope, clock), scope, clock)
        store.tickBox(null, "u1", "plan", "- [ ] a\n- [ ] b", 1)
        assertEquals(1, sent.size)
        assertTrue("\"modify\"" in sent[0] && "plan:- [ ] a\\n- [x] b" in sent[0], sent[0])
        // A line that is not a box sends nothing.
        store.tickBox(null, "u1", "plan", "words", 0)
        assertEquals(1, sent.size)
    }
}
