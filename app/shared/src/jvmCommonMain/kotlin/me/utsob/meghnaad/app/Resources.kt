package me.utsob.meghnaad.app

private object ResourceAnchor

actual fun readResource(path: String): String? =
    ResourceAnchor::class.java.getResourceAsStream(path)?.use { it.readBytes().toString(Charsets.UTF_8) }
