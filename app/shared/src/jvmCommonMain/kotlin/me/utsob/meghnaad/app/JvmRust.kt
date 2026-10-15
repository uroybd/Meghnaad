package me.utsob.meghnaad.app

import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import me.utsob.meghnaad.NativeEngine

private object JvmRust : RustHelpers {
    override fun splitWords(line: String): List<String> =
        engineJson.decodeFromString(ListSerializer(String.serializer()), NativeEngine.nativeSplitWords(line))

    override fun formatDate(pattern: String, epoch: Long, tz: Int, weekStartsMonday: Boolean): String =
        NativeEngine.nativeFormatDate(pattern, epoch, tz, weekStartsMonday)

    override fun vague(seconds: Long): String = NativeEngine.nativeVague(seconds)

    override fun call(name: String, args: String): String = NativeEngine.nativeHelper(name, args)
}

actual fun rustHelpers(): RustHelpers = JvmRust
