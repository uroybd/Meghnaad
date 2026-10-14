package me.utsob.meghnaad.app

import android.content.Context

/** [Prefs] in the app's private preferences file. */
class AndroidPrefs(context: Context) : Prefs {
    private val prefs = context.applicationContext.getSharedPreferences("prefs", Context.MODE_PRIVATE)

    override fun get(key: String): String? = prefs.getString(key, null)

    override fun put(key: String, value: String?) {
        prefs.edit().apply { if (value == null) remove(key) else putString(key, value) }.apply()
    }
}
