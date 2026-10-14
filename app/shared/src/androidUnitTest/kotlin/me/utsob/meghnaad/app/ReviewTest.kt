package me.utsob.meghnaad.app

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import java.io.File
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

/** A review through the real engine: the setup tasksh asks for, then each task marked reviewed or skipped. Skipped when the engine library is not built. */
class ReviewTest {
    private val vault = object : Vault {
        override fun engineKey() = ByteArray(32) { 7 }

        override fun connection(): Connection? = null

        override fun saveConnection(connection: Connection) {}

        override fun clearConnection() {}

        override var localOnly = true
        override var syncOptions = SyncOptions()
    }

    private fun library(): Boolean {
        val dir = File(System.getProperty("tc.engine.dir") ?: return false)
        return listOf("libtc_android.dylib", "libtc_android.so", "tc_android.dll").any { File(dir, it).exists() }
    }

    @Test
    fun setsUpThenWalksTheTasks() = runBlocking {
        if (!library()) return@runBlocking
        val clock = object : Clock {
            override fun utcOffsetSeconds() = 0

            override fun offsetAt(epochSeconds: Long) = 0

            override fun nowSeconds() = System.currentTimeMillis() / 1000.0
        }
        val engine = LocalEngine(Files.createTempDirectory("meghnaad-review").toFile(), vault)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val store = Store(Tw(engine, clock), engine, vault, SyncController(engine, scope, clock), scope, clock)
        store.loadConfig()
        store.call(listOf("add", "first"))
        store.call(listOf("add", "second"))
        val review = store.review

        // Neither the `reviewed` attribute nor the `_reviewed` report is there: it asks before writing each.
        review.start()
        assertEquals(ReviewPhase.Setup, review.phase)
        assertTrue("'reviewed' UDA" in review.question!!, review.question)
        review.answerSetup(true)
        assertEquals(ReviewPhase.Setup, review.phase)
        assertTrue("'_reviewed' report" in review.question!!, review.question)
        review.answerSetup(true)
        assertEquals(ReviewPhase.Running, review.phase, review.message)
        assertEquals(2, review.total)
        val first = review.row!!.description

        review.act(ReviewKey.Reviewed)
        assertEquals(1, review.reviewed)
        assertEquals(1, review.current)
        assertTrue(review.row!!.description != first)

        // A skipped task is not reviewed, and the end says so.
        review.act(ReviewKey.Skip)
        assertEquals(ReviewPhase.Done, review.phase)
        assertEquals("End of review. 1 out of 2 tasks reviewed.", review.message)
        review.close()

        // Only the skipped one is due again.
        review.start()
        assertEquals(ReviewPhase.Running, review.phase, review.message)
        assertEquals(1, review.total)
        review.act(ReviewKey.Modify)
        assertEquals(ReviewPhase.Modifying, review.phase)
        review.modify("+checked")
        assertEquals(ReviewPhase.Running, review.phase)
        val tags = assertIs<CliResult.Info>(store.call(listOf(review.row!!.uuid, "info")).result).value.tasks.single().tags
        assertTrue("checked" in tags, tags.toString())
        review.act(ReviewKey.Quit)
        assertEquals(ReviewPhase.Done, review.phase)
        engine.close()
        scope.cancel()
    }
}
