package me.utsob.meghnaad

import android.content.Context
import android.util.Log
import org.json.JSONObject
import java.io.File

/** The one engine of the app, shared by the page, the widget and the background sync. */
object Engines {
    private var engine: Engine? = null

    @Synchronized
    fun get(context: Context): Engine = engine ?: Engine(context.applicationContext).also { engine = it }
}

class SyncFailed(message: String) : Exception(message)

class Engine(private val context: Context) {
    private val dir = File(context.filesDir, "engine").apply { mkdirs() }
    private val replica = File(dir, "replica.bin")
    val vault = Vault(context)
    private var handle = NativeEngine.nativeNew(dir.path, vault.engineKey())

    /** The engine's change counter when it was last saved. */
    private var saved = -1L

    /** The counter as the page last heard of it: what the widget changes in between is news to the page. */
    private var reported = -1L

    /** Whether the replica was found on the phone when the engine started. */
    var restored = false
        private set

    init {
        check(handle != 0L) { "the engine could not start" }
        restore()
    }

    private fun restore() {
        if (!replica.exists()) return
        val failed = NativeEngine.nativeRestore(handle, replica.readBytes())
        if (failed == null) {
            saved = changes()
            restored = true
        } else {
            // An unusable copy (another version of the engine wrote it): start from the bucket.
            replica.delete()
        }
    }

    private fun changes(): Long = JSONObject(NativeEngine.nativeCall(handle, "changes", "")).optLong("changes", -1)

    /** Keep the replica on the phone if the tasks changed since it was last kept. Written beside, then moved into place. */
    @Synchronized
    fun save() {
        val now = changes()
        if (now == saved) return
        val bytes = NativeEngine.nativeSave(handle) ?: return
        val side = File(dir, "replica.tmp")
        side.writeBytes(bytes)
        side.renameTo(replica)
        saved = now
    }

    /**
     * One call to the engine, answered as JSON text. A write is kept on the phone before this returns, and the widget is
     * told afterwards: telling it while the engine is held would wait for the widget to read the engine, which waits for
     * this call, until the system gives up after twenty seconds.
     */
    fun call(op: String, arg: String): String {
        var wrote = false
        val answer = synchronized(this) {
            val before = changes()
            val answer = NativeEngine.nativeCall(handle, op, arg)
            if (op == "run" || op == "import") {
                wrote = changes() != before
                if (wrote) save()
                reported = changes()
            }
            answer
        }
        if (wrote) Widgets.refreshAll(context)
        return answer
    }

    /**
     * Pull and push with the bucket. Whether anything of the tasks changed; throws [SyncFailed] with why not. A phone that
     * is not connected has nothing to sync with, which is a way of using it and not a failure.
     */
    fun sync(): Boolean {
        // Read before the engine is taken: the Keystore can be slow, and nothing else should wait on it.
        val connection = vault.connection() ?: return false
        val news = synchronized(this) {
            val before = changes()
            val started = System.currentTimeMillis()
            val answer = JSONObject(NativeEngine.nativeCall(handle, "sync", connection.toSyncJson()))
            Log.d("Engine", "sync took ${System.currentTimeMillis() - started} ms: ${answer.optString("error", "ok")}")
            if (answer.has("error")) throw SyncFailed(answer.getString("error"))
            save()
            // Changed since the page last heard: by the sync, or by the widget while the page was not looking.
            val news = changes() != reported || changes() != before
            reported = changes()
            news
        }
        Widgets.refreshAll(context)
        return news
    }

    /** Throw away the copy kept on the phone; the next start fetches the tasks again from the bucket. */
    @Synchronized
    fun forget() {
        replica.delete()
        NativeEngine.nativeFree(handle)
        handle = NativeEngine.nativeNew(dir.path, vault.engineKey())
        saved = -1
        restored = false
    }
}
