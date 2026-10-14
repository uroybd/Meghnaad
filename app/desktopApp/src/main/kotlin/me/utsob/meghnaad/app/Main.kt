package me.utsob.meghnaad.app

import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import kotlinx.coroutines.runBlocking
import java.io.File
import java.security.SecureRandom

/**
 * What the preview keeps: an engine key in its own folder (nothing it seals matters), and no saved connection. A bucket can
 * be tried by setting MEGHNAAD_ENDPOINT, MEGHNAAD_BUCKET, MEGHNAAD_ACCESS_KEY, MEGHNAAD_SECRET_KEY and
 * MEGHNAAD_ENCRYPTION_SECRET (and MEGHNAAD_REGION) in the environment: they are read once, held in memory and never written.
 */
class PreviewVault(private val dir: File) : Vault {
    private var connection: Connection? = fromEnvironment()

    override fun engineKey(): ByteArray {
        val file = File(dir, "engine.key")
        if (file.exists() && file.length() == 32L) return file.readBytes()
        dir.mkdirs()
        return ByteArray(32).also { SecureRandom().nextBytes(it) }.also { file.writeBytes(it) }
    }

    override fun connection(): Connection? = connection

    override fun saveConnection(connection: Connection) {
        this.connection = connection
    }

    override fun clearConnection() {
        connection = null
    }

    override var localOnly: Boolean = connection == null
    override var syncOptions: SyncOptions = SyncOptions()

    private fun fromEnvironment(): Connection? {
        fun env(name: String) = System.getenv("MEGHNAAD_$name")?.takeIf { it.isNotBlank() }
        return Connection.typed(
            env("ENDPOINT") ?: return null,
            env("BUCKET") ?: return null,
            env("REGION") ?: "auto",
            env("ACCESS_KEY") ?: return null,
            env("SECRET_KEY") ?: return null,
            env("ENCRYPTION_SECRET") ?: return null,
        )
    }
}

/** A few tasks to look at, for a preview that starts with none. */
private suspend fun seed(tw: Tw) {
    tw.start()
    val tasks = listOf(
        listOf("add", "Write the quarterly report", "project:Work", "priority:H", "due:friday", "+next"),
        listOf("add", "Call the dentist", "project:Health", "due:today", "+phone"),
        listOf("add", "Buy milk", "project:Home", "+errand", "due:tomorrow", "priority:M"),
        listOf("add", "Fix the bike tyre", "project:Home.Garage", "+errand"),
        listOf("add", "Renew passport", "project:Admin", "due:yesterday"),
        listOf("add", "Water the plants", "project:Home", "recur:weekly", "due:sunday"),
        listOf("add", "Read Designing Data-Intensive Applications", "+reading", "priority:L"),
        listOf("add", "Plan the trip", "project:Travel", "wait:3days"),
    )
    for (args in tasks) tw.run(args = args)
    tw.run(args = listOf("2", "start"))
}

fun main() = application {
    val dir = File(System.getProperty("meghnaad.data") ?: "build/preview-data").apply { mkdirs() }
    val scope = rememberCoroutineScope()
    val store = remember {
        val vault = PreviewVault(dir)
        val engine = LocalEngine(dir, vault)
        val tw = Tw(engine, SystemClock)
        if (!engine.restored && vault.connection() == null) runBlocking { seed(tw) }
        Store(tw, engine, vault, SyncController(engine, scope, SystemClock), scope, SystemClock).also { it.begin() }
    }
    // The size of a phone (a Pixel's 412 x 892 dp), and not resizable: the app has one layout, the phone's.
    Window(
        onCloseRequest = ::exitApplication,
        title = "Meghnaad (phone preview)",
        state = rememberWindowState(width = 412.dp, height = 892.dp),
        resizable = false,
    ) {
        App(store)
    }
}
