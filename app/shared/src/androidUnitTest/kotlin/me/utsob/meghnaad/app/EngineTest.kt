package me.utsob.meghnaad.app

import kotlinx.coroutines.runBlocking
import java.io.File
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertIs
import kotlin.test.assertTrue

/** The real engine, through the real JNI library (a host build of it): skipped when `cargo build --profile android -p tc-android` has not run. */
class EngineTest {
    /** A vault with nothing in it: local use, and a fixed key. */
    private val secrets = object : Vault {
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
    fun commandsRunAndTheTasksSurviveARestart() = runBlocking {
        if (!library()) {
            println("skipped: the engine library is not built")
            return@runBlocking
        }
        val dir = Files.createTempDirectory("meghnaad-test").toFile()
        val clock = object : Clock {
            override fun utcOffsetSeconds() = 21_600

            override fun offsetAt(epochSeconds: Long) = 21_600

            override fun nowSeconds() = System.currentTimeMillis() / 1000.0
        }
        val first = LocalEngine(dir, secrets)
        val tw = Tw(first, clock)
        assertTrue(tw.start().reports.any { it.name == "next" })
        val added = tw.run(args = listOf("add", "buy milk", "project:Home", "+errand"))
        assertTrue(added.wrote)
        val listed = assertIs<CliResult.Report>(tw.run(args = listOf("next")).result).value
        assertEquals(listOf("buy milk"), listed.rows.map { it.description })
        assertEquals("Home", listed.rows.single().project)
        first.close()

        // Another engine on the same folder finds the tasks where they were left.
        val second = LocalEngine(dir, secrets)
        assertTrue(second.restored)
        val again = Tw(second, clock)
        again.start()
        val back = assertIs<CliResult.Report>(again.run(args = listOf("next")).result).value
        assertEquals(listOf("buy milk"), back.rows.map { it.description })
        second.close()
    }

    @Test
    fun aQuestionComesBackAsOne() = runBlocking {
        if (!library()) return@runBlocking
        val dir = Files.createTempDirectory("meghnaad-test").toFile()
        val clock = object : Clock {
            override fun utcOffsetSeconds() = 0

            override fun offsetAt(epochSeconds: Long) = 0

            override fun nowSeconds() = System.currentTimeMillis() / 1000.0
        }
        val engine = LocalEngine(dir, secrets)
        val tw = Tw(engine, clock)
        tw.start()
        tw.run(args = listOf("add", "one"))
        tw.run(args = listOf("add", "two"))
        // A command with no filter that would change everything asks first.
        val got = tw.run(args = listOf("modify", "+x"))
        assertIs<CliResult.Confirm>(got.result)
        engine.close()
    }
}
