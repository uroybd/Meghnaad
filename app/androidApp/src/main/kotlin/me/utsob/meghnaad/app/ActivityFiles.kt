package me.utsob.meghnaad.app

import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Files through the system's own pickers (no storage permission): one to read, one to keep. The launchers are registered when the
 * activity is created, as the system asks.
 */
class ActivityFiles(private val activity: ComponentActivity) : FileHost {
    private var reading: CompletableDeferred<Uri?>? = null
    private var writing: CompletableDeferred<Uri?>? = null
    private var readingMany: CompletableDeferred<List<Uri>>? = null
    private var readingFolder: CompletableDeferred<Uri?>? = null

    private val open = activity.registerForActivityResult(ActivityResultContracts.OpenDocument()) { reading?.complete(it) }
    private val create = activity.registerForActivityResult(ActivityResultContracts.CreateDocument("*/*")) { writing?.complete(it) }
    private val openMany = activity.registerForActivityResult(ActivityResultContracts.OpenMultipleDocuments()) { readingMany?.complete(it) }
    private val openFolder = activity.registerForActivityResult(ActivityResultContracts.OpenDocumentTree()) { readingFolder?.complete(it) }

    override suspend fun pick(): PickedFile? {
        val asked = CompletableDeferred<Uri?>().also { reading = it }
        open.launch(arrayOf("application/json", "text/plain", "*/*"))
        val uri = asked.await() ?: return null
        return withContext(Dispatchers.IO) {
            val resolver = activity.contentResolver
            var name = uri.lastPathSegment ?: "file"
            var size = -1L
            resolver.query(uri, null, null, null, null)?.use { c ->
                if (c.moveToFirst()) {
                    c.getColumnIndex(OpenableColumns.DISPLAY_NAME).takeIf { it >= 0 }?.let { name = c.getString(it) }
                    c.getColumnIndex(OpenableColumns.SIZE).takeIf { it >= 0 && !c.isNull(it) }?.let { size = c.getLong(it) }
                }
            }
            if (size > MAX_IMPORT_BYTES) return@withContext PickedFile(name, size, null)
            val bytes = resolver.openInputStream(uri)?.use { it.readBytes() } ?: return@withContext null
            if (bytes.size > MAX_IMPORT_BYTES) PickedFile(name, bytes.size.toLong(), null) else PickedFile(name, bytes.size.toLong(), bytes.decodeToString())
        }
    }

    override suspend fun save(name: String, mime: String, text: String): Boolean {
        val asked = CompletableDeferred<Uri?>().also { writing = it }
        create.launch(name)
        val uri = asked.await() ?: return false
        return withContext(Dispatchers.IO) {
            try {
                activity.contentResolver.openOutputStream(uri)?.use { it.write(text.encodeToByteArray()) } != null
            } catch (_: Exception) {
                false
            }
        }
    }

    override suspend fun pickMany(): List<PickedFile> {
        val asked = CompletableDeferred<List<Uri>>().also { readingMany = it }
        openMany.launch(arrayOf("text/plain", "*/*"))
        val uris = asked.await()
        return withContext(Dispatchers.IO) { uris.mapNotNull { read(it, null) } }
    }

    override suspend fun pickFolder(): List<PickedFile> {
        val asked = CompletableDeferred<Uri?>().also { readingFolder = it }
        openFolder.launch(null)
        val tree = asked.await() ?: return emptyList()
        return withContext(Dispatchers.IO) {
            val root = DocumentsContract.getTreeDocumentId(tree)
            val out = ArrayList<PickedFile>()
            walk(tree, root, DocumentsContract.getTreeDocumentId(tree).substringAfterLast('/').substringAfterLast(':').ifEmpty { "folder" }, 0, out)
            out
        }
    }

    /** The files below a folder, each with its path from the folder's own name (as the web's `webkitRelativePath`). Hidden folders are not entered. */
    private fun walk(tree: Uri, parentId: String, path: String, depth: Int, out: MutableList<PickedFile>) {
        if (depth > MAX_DEPTH || out.size > MAX_ENTRIES) return
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId)
        val columns = arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE)
        val entries = ArrayList<Triple<String, String, Boolean>>()
        activity.contentResolver.query(children, columns, null, null, null)?.use { c ->
            while (c.moveToNext()) {
                entries.add(Triple(c.getString(0), c.getString(1), c.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR))
            }
        }
        for ((id, name, dir) in entries) {
            if (dir) {
                if (!name.startsWith(".")) walk(tree, id, "$path/$name", depth + 1, out)
            } else if (!name.startsWith(".") || name == ".taskrc") {
                read(DocumentsContract.buildDocumentUriUsingTree(tree, id), "$path/$name")?.let { out.add(it) }
            } else {
                out.add(PickedFile(name, 0, "", "$path/$name"))
            }
        }
    }

    private fun read(uri: Uri, relative: String?): PickedFile? {
        val resolver = activity.contentResolver
        var name = relative?.substringAfterLast('/') ?: uri.lastPathSegment ?: "file"
        var size = -1L
        resolver.query(uri, null, null, null, null)?.use { c ->
            if (c.moveToFirst()) {
                if (relative == null) c.getColumnIndex(OpenableColumns.DISPLAY_NAME).takeIf { it >= 0 }?.let { name = c.getString(it) }
                c.getColumnIndex(OpenableColumns.SIZE).takeIf { it >= 0 && !c.isNull(it) }?.let { size = c.getLong(it) }
            }
        }
        // A taskrc file is small: a large one is not read, and the screen says so.
        if (size > MAX_IMPORT_BYTES) return PickedFile(name, size, null, relative)
        val bytes = resolver.openInputStream(uri)?.use { it.readBytes() } ?: return null
        return if (bytes.size > MAX_IMPORT_BYTES) PickedFile(name, bytes.size.toLong(), null, relative) else PickedFile(name, bytes.size.toLong(), bytes.decodeToString(), relative)
    }

    private companion object {
        const val MAX_DEPTH = 6
        const val MAX_ENTRIES = 300
    }
}
