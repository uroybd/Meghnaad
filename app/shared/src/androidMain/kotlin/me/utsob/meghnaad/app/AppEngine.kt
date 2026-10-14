package me.utsob.meghnaad.app

import android.content.Context

/** The one engine of the app, shared by the screen, the reminder alarm (and later the widgets and the background sync). */
object AppEngine {
    private var vault: AndroidVault? = null
    private var engine: LocalEngine? = null

    @Synchronized
    fun vault(context: Context): AndroidVault = vault ?: AndroidVault(context.applicationContext).also { vault = it }

    @Synchronized
    fun get(context: Context): LocalEngine = engine ?: LocalEngine(context.applicationContext.filesDir, vault(context)).also { engine = it }
}
