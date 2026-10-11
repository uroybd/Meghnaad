package me.utsob.meghnaad

/** The Taskwarrior engine, in Rust (`crates/android`). Every engine is a handle; every call answers in JSON. */
object NativeEngine {
    init {
        System.loadLibrary("tc_android")
    }

    /** An engine keeping its settings in [dir], and the hooks' secrets sealed with [key] (32 bytes). 0 if it could not start. */
    @JvmStatic external fun nativeNew(dir: String, key: ByteArray): Long

    @JvmStatic external fun nativeFree(handle: Long)

    /** `config`, `run`, `import`, `sync`, `changes` or `api`, with its argument as text; a JSON object back. */
    @JvmStatic external fun nativeCall(handle: Long, op: String, arg: String): String

    /** Everything the replica holds, to keep and give to [nativeRestore] next time; null if it could not be saved. */
    @JvmStatic external fun nativeSave(handle: Long): ByteArray?

    /** Go on from saved bytes: null when it worked, the reason when it did not. */
    @JvmStatic external fun nativeRestore(handle: Long, bytes: ByteArray): String?
}
