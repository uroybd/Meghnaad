package me.utsob.meghnaad.app

import android.net.Uri
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

    private val open = activity.registerForActivityResult(ActivityResultContracts.OpenDocument()) { reading?.complete(it) }
    private val create = activity.registerForActivityResult(ActivityResultContracts.CreateDocument("*/*")) { writing?.complete(it) }

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
}
