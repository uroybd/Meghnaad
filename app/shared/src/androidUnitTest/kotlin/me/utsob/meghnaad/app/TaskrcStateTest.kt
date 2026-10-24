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

    private val newYear = """{"date":"2026-01-01","description":"New Year's Day","region":""}
{"date":"2026-04-20","description":"Patriots' Day","region":"MA"}
"""

    @Test
    fun holidaysFromHolidataAreSavedIncludedAndShown() = runBlocking {
        if (!library()) return@runBlocking
        val store = store()
        store.loadConfig()
        val st = TaskrcState(store)
        st.load()
        st.text = "bulk=3\n"
        st.save()

        val asked = mutableListOf<String>()
        val get: suspend (String) -> HttpAnswer = { url ->
            asked += url
            if ("/2026.json" in url) HttpAnswer(200, newYear) else HttpAnswer(404, "")
        }
        st.hLocale = "en-US"
        st.hFirst = "2026"
        st.hLast = "2027"
        st.findHolidays(get)
        assertEquals(null, st.hError, st.hError)
        assertEquals(listOf("https://holidata.net/en-US/2026.json", "https://holidata.net/en-US/2027.json"), asked)
        assertEquals(listOf(2027), st.hFetched?.missing)
        // National holidays only until a region is asked for.
        assertEquals(1, st.hBuilt?.count)
        assertEquals(listOf("MA"), st.hBuilt?.regions)

        st.addHolidays()
        assertEquals(null, st.error, st.error)
        assertEquals("holidays/en-US.rc", st.current)
        assertTrue("holiday.en-US1.date=20260101" in st.text, st.text)
        // The main taskrc includes it and the calendar shows holidays, and the settings are in force.
        st.open(RC_ROOT, force = true)
        assertEquals("bulk=3\ninclude holidays/en-US.rc\ncalendar.holidays=sparse\n", st.text)
        assertEquals("New Year's Day", store.config?.config?.settings?.get("holiday.en-US1.name"))
        assertEquals(emptyList(), st.folder?.problems)

        // The file knows what it was made for, so it can be fetched again.
        st.open("holidays/en-US.rc", force = true)
        assertEquals("en-US", st.made?.first)
        st.hChosen = emptyList()
        st.updateHolidays(get)
        assertEquals("en-US", st.hLocale)
        assertEquals(1, st.hBuilt?.count)
    }

    @Test
    fun holidataIsNotReachedForAnythingButALocale() = runBlocking {
        if (!library()) return@runBlocking
        val failure = runCatching { fetchHolidays("../x", 2026, 2026) { error("must not be asked") } }.exceptionOrNull()
        assertTrue(failure?.message?.contains("not a year Holidata can have") == true, failure?.message)
        val offline = runCatching { fetchHolidays("en-US", 2026, 2026) { throw java.io.IOException("down") } }.exceptionOrNull()
        assertTrue(offline?.message?.contains("Couldn't reach holidata.net") == true, offline?.message)
        assertEquals("a=1\n", withSetting("", "a", "1"))
        assertEquals("x=2\nb=1\n", withSetting("x=1\nb=1\n", "x", "2"))
    }
}
