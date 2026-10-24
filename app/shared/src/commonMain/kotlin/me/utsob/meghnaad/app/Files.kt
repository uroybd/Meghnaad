package me.utsob.meghnaad.app

/** The most one import takes (the engine's own limit, as the web states it). */
const val MAX_IMPORT_BYTES = 3 * 1024 * 1024

/**
 * A file the person chose. [text] is null when it is more than [MAX_IMPORT_BYTES], which is [size] to say so. [relative] is its path
 * when it came with a folder, the folder's own name first (`dots/holidays/en-US.rc`), as the web's `webkitRelativePath` is.
 */
class PickedFile(val name: String, val size: Long, val text: String?, val relative: String? = null)

/** What the phone does with files: ask for one to read, and offer text to keep as one. */
interface FileHost {
    /** Ask for a file; null when none was chosen. */
    suspend fun pick(): PickedFile?

    /** Offer [text] as a file called [name] to keep; false when it was not kept. */
    suspend fun save(name: String, mime: String, text: String): Boolean

    /** Offer [bytes] as a file called [name] to keep (a zip); false when it was not kept or this build can't. */
    suspend fun saveBytes(name: String, mime: String, bytes: ByteArray): Boolean = false

    /** Ask for several files; none when none were chosen. */
    suspend fun pickMany(): List<PickedFile> = emptyList()

    /** Ask for a folder; its files, each with its [PickedFile.relative] path. None when none was chosen. */
    suspend fun pickFolder(): List<PickedFile> = emptyList()
}
