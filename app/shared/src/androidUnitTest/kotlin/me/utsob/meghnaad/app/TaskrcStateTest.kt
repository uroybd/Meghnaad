package me.utsob.meghnaad.app

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.runBlocking
import java.io.File
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

/** The taskrc tab against the real engine: a folder of files, kept as written, with include followed and credentials removed. */
class TaskrcStateTest {
    private val vault = object : Vault {
        override fun engineKey() = ByteArray(32) { 7 }

        override fun connection(): Connection? = null

        override fun saveConnection(connection: Connection) {}

        override fun clearConnection() {}

        override var localOnly = true
        override var syncOptions = SyncOptions()
    }

    private fun library(): Boolean {
        val dir = File(System.getProperty("tc.engine.dir") ?: return false)
        return listOf("libtc_android.dylib", "libtc_android.so", "tc_android.dll").any { File(dir, it).exists() }
    }

    private fun store(): Store {
        val clock = object : Clock {
            override fun utcOffsetSeconds() = 0

            override fun offsetAt(epochSeconds: Long) = 0

            override fun nowSeconds() = System.currentTimeMillis() / 1000.0
        }
        val engine = LocalEngine(Files.createTempDirectory("meghnaad-taskrc").toFile(), vault)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        return Store(Tw(engine, clock), engine, vault, SyncController(engine, scope, clock), scope, clock)
    }

    @Test
    fun aFolderIsKeptAsWrittenIncludeIsFollowedAndACredentialIsRemoved() = runBlocking {
        if (!library()) return@runBlocking
        val store = store()
        store.loadConfig()
        val st = TaskrcState(store)
        st.load()
        assertEquals(RC_ROOT, st.current)
        assertEquals("", st.text)

        // A folder: the main taskrc includes a file in a folder; one line is a credential, one cannot be used.
        st.upload(listOf(
            PickedFile("taskrc", 0, "# mine\nbulk=4   # four\ninclude holidays/en.rc\nsync.encryption_secret=hunter2\nnonsense=1\n", "dots/taskrc"),
            PickedFile("en.rc", 0, "holiday.towel.name=Towel day\nholiday.towel.date=20260525\n", "dots/holidays/en.rc"),
            PickedFile("HEAD", 0, "ref", "dots/.git/HEAD"),
        ))
        assertEquals(null, st.error, st.error)
        assertEquals(listOf("dots/.git/HEAD (hidden)"), st.skipped)
        // As written, comments and all, less the credential, with what cannot be used commented out.
        assertEquals("# mine\nbulk=4   # four\ninclude holidays/en.rc\n# [unsupported] nonsense=1\n", st.text)
        assertFalse("hunter2" in st.text)
        assertEquals(listOf("sync.encryption_secret"), st.result?.blocked)
        assertEquals(setOf(RC_ROOT, "holidays/en.rc"), st.paths.toSet())
        assertEquals(listOf("taskrc", "holidays", "en.rc"), st.rows.map { it.name })
        // The settings in the included file are in force.
        assertEquals("Towel day", store.config?.config?.settings?.get("holiday.towel.name"))

        // Open the included file, and include another one in the main taskrc from it.
        st.open("holidays/en.rc")
        assertEquals("holidays/en.rc", st.current)
        assertEquals("holidays/en.rc", store.settingsFile)
        assertEquals(RC_ROOT, st.others.single())
        st.newName = "holidays/de.rc"
        st.addFile()
        assertEquals("holidays/de.rc", st.current)
        st.includeIn = RC_ROOT
        st.include()
        assertEquals(RC_ROOT, st.current)
        assertTrue(st.text.endsWith("include holidays/de.rc\n"), st.text)
        assertTrue(st.fileEdited)
        st.save()
        assertEquals(null, st.error, st.error)
        assertFalse(st.fileEdited)
        assertEquals(emptyList(), st.folder?.problems)

        // A file that is deleted leaves an include that points nowhere, which is said, not a crash.
        st.open("holidays/de.rc")
        st.confirmDelete = true
        st.removeFile()
        assertEquals(RC_ROOT, st.current)
        assertEquals(1, st.folder?.problems?.size)
        assertNotNull(st.folder?.problems?.single()?.message?.takeIf { "no such file" in it })

        // And back to before the delete.
        st.confirmRestore = true
        st.restore()
        assertTrue("holidays/de.rc" in st.paths, st.paths.toString())
        assertEquals(emptyList(), st.folder?.problems)
    }
}
