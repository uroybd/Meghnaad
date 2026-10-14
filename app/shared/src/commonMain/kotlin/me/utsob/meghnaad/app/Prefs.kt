package me.utsob.meghnaad.app

/**
 * Small per-device choices: the last commands, whether hidden reports are shown, the reminder options. The web keeps the same
 * in the browser (`localStorage`); nothing secret goes here (the [Vault] has that).
 */
interface Prefs {
    fun get(key: String): String?

    /** Keep [value], or forget the key when it is null. */
    fun put(key: String, value: String?)
}

/** Kept for as long as the app runs: for tests and the preview. */
class MemoryPrefs : Prefs {
    private val values = mutableMapOf<String, String>()

    override fun get(key: String) = values[key]

    override fun put(key: String, value: String?) {
        if (value == null) values.remove(key) else values[key] = value
    }
}
