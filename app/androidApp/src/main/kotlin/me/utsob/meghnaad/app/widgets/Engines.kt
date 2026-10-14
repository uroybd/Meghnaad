package me.utsob.meghnaad.app.widgets

import android.content.Context
import kotlinx.coroutines.runBlocking
import me.utsob.meghnaad.app.AppEngine
import me.utsob.meghnaad.app.AndroidVault
import me.utsob.meghnaad.app.SystemClock
import me.utsob.meghnaad.app.Tw

/**
 * The app's one engine, as the widgets and the background work ask of it: plain calls that wait for the answer (they run off the
 * main thread already). Its settings are read once per process, as the screen does before its first command.
 */
object Engines {
    @Volatile
    private var started = false

    fun get(context: Context): WidgetEngine = WidgetEngine(context.applicationContext)

    internal fun start(context: Context) {
        if (started) return
        synchronized(this) {
            if (started) return
            runBlocking { Tw(AppEngine.get(context), SystemClock).start() }
            started = true
        }
    }
}

class WidgetEngine(private val context: Context) {
    val vault: AndroidVault = AppEngine.vault(context)

    /** One call to the engine, answered as JSON text. */
    fun call(op: String, arg: String): String {
        Engines.start(context)
        return runBlocking { AppEngine.get(context).call(op, arg) }
    }

    /** Pull and push with the bucket. Throws [me.utsob.meghnaad.app.SyncFailed] with why not. */
    fun sync() {
        Engines.start(context)
        runBlocking { AppEngine.get(context).sync() }
    }
}
