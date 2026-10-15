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

/** The Settings tabs against the real engine: they are made when the page is opened, which can be before the saved settings are read. */
class SettingsStateTest {
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
    fun theCommandsTabShowsCommandsThatWereReadAfterItWasMade() = runBlocking {
        if (!library()) return@runBlocking
        val clock = object : Clock {
            override fun utcOffsetSeconds() = 0

            override fun offsetAt(epochSeconds: Long) = 0

            override fun nowSeconds() = System.currentTimeMillis() / 1000.0
        }
        val engine = LocalEngine(Files.createTempDirectory("meghnaad-settings").toFile(), vault)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val store = Store(Tw(engine, clock), engine, vault, SyncController(engine, scope, clock), scope, clock)
        store.loadConfig()
        store.request("PUT", "/api/settings", """{"taskCommands":[{"name":"fail","command":"done outcome:fail"},{"name":"wait3","command":"modify wait:3d","icon":"clock"}]}""")

        // The page is opened before settings.json has been read: the tab starts with nothing…
        val state = CommandsState(store)
        assertEquals(0, state.rows.size)
        // …and takes the commands when they arrive, as it does the first time it is drawn.
        store.loadAppSettings()
        state.follow()
        assertEquals(listOf("fail", "wait3"), state.rows.map { it.name })
        assertEquals(false, state.edited)

        // An edit is not thrown away by a later change of what is saved.
        state.rows[0].command = "done outcome:failed"
        store.request("PUT", "/api/settings", """{"taskCommands":[{"name":"fail","command":"done outcome:fail"}]}""")
        store.loadAppSettings()
        state.follow()
        assertEquals(listOf("fail", "wait3"), state.rows.map { it.name })
        assertEquals("done outcome:failed", state.rows[0].command)
        engine.close()
        scope.cancel()
    }

    @Test
    fun aConnectionThatDoesNotWorkIsNotKept() = runBlocking {
        if (!library()) return@runBlocking
        val clock = object : Clock {
            override fun utcOffsetSeconds() = 0

            override fun offsetAt(epochSeconds: Long) = 0

            override fun nowSeconds() = System.currentTimeMillis() / 1000.0
        }
        val worked = Connection("http://127.0.0.1:9", "tasks", "auto", "AKIAEXAMPLEKEY1", "secret-one", "encryption-one")
        var kept: Connection? = worked
        val connected = object : Vault {
            override fun engineKey() = ByteArray(32) { 7 }

            override fun connection(): Connection? = kept

            override fun saveConnection(connection: Connection) {
                kept = connection
            }

            override fun clearConnection() {
                kept = null
            }

            override var localOnly = false
            override var syncOptions = SyncOptions()
        }
        val engine = LocalEngine(Files.createTempDirectory("meghnaad-connection").toFile(), connected)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val store = Store(Tw(engine, clock), engine, connected, SyncController(engine, scope, clock), scope, clock)
        store.loadConfig()

        // What the tab may show has no secret in it, and only a hint of the key's name.
        val info = store.connectionInfo!!
        assertEquals("http://127.0.0.1:9", info.endpoint)
        assertEquals("AKIA…KEY1", info.accessKeyHint)

        // Another address that cannot be reached: it says why, and the connection that was there is still the one kept.
        val why = store.changeConnection("http://127.0.0.1:10", "", "", "", "", "")
        assertEquals(true, why != null)
        assertEquals(worked, kept)
        // Blanks keep what was there; a connection with nothing changed is not tried.
        assertEquals(null, store.changeConnection("", "", "auto", "", "", ""))
        engine.close()
        scope.cancel()
    }
}
