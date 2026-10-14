package me.utsob.meghnaad.app

/** The most one import takes (the engine's own limit, as the web states it). */
const val MAX_IMPORT_BYTES = 3 * 1024 * 1024

/** A file the person chose. [text] is null when it is more than [MAX_IMPORT_BYTES], which is [size] to say so. */
class PickedFile(val name: String, val size: Long, val text: String?)

/** What the phone does with files: ask for one to read, and offer text to keep as one. */
interface FileHost {
    /** Ask for a file; null when none was chosen. */
    suspend fun pick(): PickedFile?

    /** Offer [text] as a file called [name] to keep; false when it was not kept. */
    suspend fun save(name: String, mime: String, text: String): Boolean
}
