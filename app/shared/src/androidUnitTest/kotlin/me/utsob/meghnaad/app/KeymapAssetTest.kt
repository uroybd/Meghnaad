package me.utsob.meghnaad.app

import java.io.File
import kotlin.test.Test
import kotlin.test.assertEquals

// The default keys are the web's asset, copied by scripts/gen-lucide.mjs: the copy must not drift from it.
class KeymapAssetTest {
    @Test
    fun theResourceIsTheAssetTheWebServes() {
        val web = generateSequence(File("").absoluteFile) { it.parentFile }
            .map { File(it, "web/public/keymaps/default.json") }
            .firstOrNull { it.exists() } ?: return
        assertEquals(web.readText(), readResource("/keymaps/default.json"), "run node scripts/gen-lucide.mjs")
    }
}
