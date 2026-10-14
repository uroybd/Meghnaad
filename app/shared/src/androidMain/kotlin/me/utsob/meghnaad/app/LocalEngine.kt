package me.utsob.meghnaad.app

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import me.utsob.meghnaad.NativeEngine
import java.io.File

/** Keeps what must not be read from a copy of the app's files. */
interface SecretStore {
    /** The key that seals the hooks' secrets (32 bytes), made on first use. */
    fun engineKey(): ByteArray
}

/** The system's clock and time zone. */
object SystemClock : Clock {
    override fun utcOffsetSeconds(): Int = java.util.TimeZone.getDefault().getOffset(System.currentTimeMillis()) / 1000

    override fun nowSeconds(): Double = System.currentTimeMillis() / 1000.0
}

/**
 * The engine of the app: the native one, with its replica kept in a file so that the tasks are there next time. A write is
 * kept before the call returns. Mirrors `Engine` of the Capacitor app (android/…/Engines.kt) without the phone in it.
 */
class LocalEngine(dir: File, secrets: SecretStore) : EnginePort {
    private val folder = File(dir, "engine").apply { mkdirs() }
    private val replica = File(folder, "replica.bin")
    private var handle = NativeEngine.nativeNew(folder.path, secrets.engineKey())

    /** The engine's change counter when it was last saved. */
    private var saved = -1L

    /** Whether the replica was found when the engine started. */
    var restored = false
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
        synchronized(this@LocalEngine) {
            val answer = NativeEngine.nativeCall(handle, op, arg)
            if (op == "run" || op == "import") save()
            answer
        }
    }

    @Synchronized
    fun close() {
        save()
        NativeEngine.nativeFree(handle)
        handle = 0
    }
}
