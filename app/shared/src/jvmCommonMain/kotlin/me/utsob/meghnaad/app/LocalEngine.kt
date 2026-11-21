package me.utsob.meghnaad.app

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import me.utsob.meghnaad.NativeEngine
import java.io.File

/** The system's clock and time zone. */
object SystemClock : Clock {
    override fun utcOffsetSeconds(): Int = java.util.TimeZone.getDefault().getOffset(System.currentTimeMillis()) / 1000

    override fun offsetAt(epochSeconds: Long): Int = java.util.TimeZone.getDefault().getOffset(epochSeconds * 1000) / 1000

    override fun nowSeconds(): Double = System.currentTimeMillis() / 1000.0
}

/**
 * The engine of the app: the native one, with its replica kept in a file so that the tasks are there next time. A write is
 * kept before the call returns. Its folder and file names are the ones the earlier (Capacitor) app used, so an update finds the tasks where they were left.
 */
class LocalEngine(dir: File, private val vault: Vault) : EnginePort {
    private val folder = File(dir, "engine").apply { mkdirs() }
    private val replica = File(folder, "replica.bin")

    /** The key of the last sync, kept by the engine between launches (sealed with [Vault.engineKey]). */
    private val keyFile = File(folder, "sync-key.enc")
    private var handle = NativeEngine.nativeNew(folder.path, vault.engineKey())

    /**
     * One sync at a time (the app's and the background worker's share this engine): the second finds the key the first
     * kept, and the engine is never freed while a sync runs with it but not holding it.
     */
    private val syncing = Mutex()

    /** The engine's change counter when it was last saved. */
    private var saved = -1L

    /** Whether the replica was found when the engine started. */
    override var restored = false
        private set

    init {
        check(handle != 0L) { "the engine could not start" }
        restore()
    }

    private fun restore() {
        if (!replica.exists()) return
        if (NativeEngine.nativeRestore(handle, replica.readBytes()) == null) {
            saved = changes()
            restored = true
        } else {
            // An unusable copy (another version of the engine wrote it): start again from nothing.
            replica.delete()
        }
    }

    private fun changes(): Long =
        engineJson.parseToJsonElement(NativeEngine.nativeCall(handle, "changes", "")).jsonObject["changes"]
            ?.jsonPrimitive?.content?.toLongOrNull() ?: -1

    /** Keep the replica if the tasks changed since it was last kept: written beside, then moved into place. */
    @Synchronized
    fun save() {
        val now = changes()
        if (now == saved) return
        val bytes = NativeEngine.nativeSave(handle) ?: return
        val side = File(folder, "replica.tmp")
        side.writeBytes(bytes)
        if (!side.renameTo(replica)) {
            replica.writeBytes(bytes)
            side.delete()
        }
        saved = now
    }

    override suspend fun call(op: String, arg: String): String = withContext(Dispatchers.Default) {
        val asked = Timings.mark()
        synchronized(this@LocalEngine) {
            Timings.since(asked, "engine $op: waited")
            val held = Timings.mark()
            val answer = NativeEngine.nativeCall(handle, op, arg)
            if (op == "run" || op == "import") save()
            Timings.since(held, "engine $op: ran")
            answer
        }
    }

    override suspend fun sync(switch: String?): SyncResult = withContext(Dispatchers.Default) {
        // Read before the engine is taken: the Keystore can be slow, and nothing else should wait on it.
        val connection = vault.connection() ?: run {
            // Not connected (or no longer): the key the syncs kept goes too.
            keyFile.delete()
            return@withContext SyncResult(false)
        }
        val files = vault.syncOptions.files()
        val settings = connection.toSyncJson(files, switch)
        syncing.withLock {
            // The engine is taken only for a moment to say where the sync starts from. The key (derived, or the kept one
            // checked against the store) and what is new are then found with it free, so a tap meanwhile is answered; it
            // is taken again to apply that and push what is waiting, the one part that must hold it; and the settings
            // files are kept in step with it free, taken only for each file written.
            var at = Timings.mark()
            val source = synchronized(this@LocalEngine) { handleOf(NativeEngine.nativeSource(handle, settings), "source") }
            Timings.since(at, "sync: source (engine held)")
            at = Timings.mark()
            val fetched = if (source == 0L) 0L else handleOf(NativeEngine.nativeFetch(source), "fetched")
            Timings.since(at, "sync: key and fetch (engine free)")
            at = Timings.mark()
            val changed = synchronized(this@LocalEngine) {
                val before = changes()
                engineJson.parseToJsonElement(NativeEngine.nativeSyncFetched(handle, settings, fetched)).jsonObject["error"]
                    ?.let { throw SyncFailed(it.jsonPrimitive.content) }
                // A switch put another replica in place of this one: it is the one to keep, and the screen is drawn again.
                if (switch != null) saved = -1
                save()
                switch != null || changes() != before
            }
            Timings.since(at, "sync: apply and push (engine held)")
            at = Timings.mark()
            val answer = engineJson.parseToJsonElement(NativeEngine.nativeSyncFiles(handle, settings)).jsonObject
            answer["error"]?.let { throw SyncFailed(it.jsonPrimitive.content) }
            Timings.since(at, "sync: settings files (engine free)")
            SyncResult(changed, (answer["settings"] as? JsonArray)?.map { it.jsonPrimitive.content } ?: emptyList())
        }
    }

    /** The handle named [key] in a native answer, 0 when there is none; its `error` is thrown. */
    private fun handleOf(answer: String, key: String): Long {
        val o = engineJson.parseToJsonElement(answer).jsonObject
        o["error"]?.let { throw SyncFailed(it.jsonPrimitive.content) }
        return o[key]?.jsonPrimitive?.content?.toLongOrNull() ?: 0L
    }

    override suspend fun forget() = withContext(Dispatchers.Default) {
        syncing.withLock { forgetNow() }
    }

    private fun forgetNow() {
        // The kept key stays: it is checked against the store before it is used again.
        synchronized(this@LocalEngine) {
            replica.delete()
            NativeEngine.nativeFree(handle)
            handle = NativeEngine.nativeNew(folder.path, vault.engineKey())
            saved = -1
            restored = false
        }
    }

    @Synchronized
    fun close() {
        save()
        NativeEngine.nativeFree(handle)
        handle = 0
    }
}
