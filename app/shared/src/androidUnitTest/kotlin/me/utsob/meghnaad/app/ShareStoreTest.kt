package me.utsob.meghnaad.app

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import java.io.File
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

/** Spec 0016 against the real engine: each form a task is shared in, from the tasks the engine holds. */
class ShareStoreTest {
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

    private val clock = object : Clock {
        override fun utcOffsetSeconds() = 0

        override fun offsetAt(epochSeconds: Long) = 0

        override fun nowSeconds() = System.currentTimeMillis() / 1000.0
    }

    private fun withStore(block: suspend (Store) -> Unit) = runBlocking {
        if (!library()) return@runBlocking
        val engine = LocalEngine(Files.createTempDirectory("meghnaad-share").toFile(), vault)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val store = Store(Tw(engine, clock), engine, vault, SyncController(engine, scope, clock), scope, clock)
        store.loadConfig()
        try {
            block(store)
        } finally {
            engine.close()
            scope.cancel()
        }
    }

    private suspend fun Store.task(filter: String): Row {
        val r = query(listOf(filter, "info"))
        return (r as CliResult.Info).value.tasks.single()
    }

    @Test
    fun eachFormIsMadeFromTheTaskTheEngineHolds() = withStore { store ->
        store.query(listOf("add", "Dep one", "+one"))
        store.query(listOf("add", "Dep two", "+two"))
        val one = store.task("+one").uuid
        val two = store.task("+two").uuid
        store.query(listOf("add", "Buy milk", "project:home", "priority:H", "+errand", "depends:$one,$two"))
        val milk = store.task("project:home")
        store.query(listOf("add", "After milk", "depends:${milk.uuid}"))
        store.query(listOf("+two", "done"))
        assertEquals("Buy milk", milk.description)

        assertEquals("Buy milk\nproject:home priority:H +errand", store.shareText(ShareForm.Short, milk))

        val json = kotlinx.serialization.json.Json.parseToJsonElement(store.shareText(ShareForm.Json, milk)).jsonObject
        assertEquals("Buy milk", json["description"]?.jsonPrimitive?.content)
        assertEquals(milk.uuid, json["uuid"]?.jsonPrimitive?.content)

        val info = store.shareText(ShareForm.Info, milk)
        assertTrue(info.startsWith("Name "), info)
        assertTrue(info.contains(Regex("Description +Buy milk\n")), info)
        // Blocked by the open dependency only (the done one is not listed), and blocking the task that waits on it.
        assertTrue(info.contains(Regex("This task blocked by +\\d+ Dep one\n")), info)
        assertFalse(info.contains("Dep two"), info)
        assertTrue(info.contains(Regex("This task is blocking +\\d+ After milk\n")), info)
        assertTrue(info.contains(Regex("Priority +H\n")), info)
        assertTrue(info.contains("Description set to 'Buy milk'."), info)
    }
}
