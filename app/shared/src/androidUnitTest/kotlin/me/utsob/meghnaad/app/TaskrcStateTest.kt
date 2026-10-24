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

    private fun store(files: FileHost? = null): Store {
        val clock = object : Clock {
            override fun utcOffsetSeconds() = 0

            override fun offsetAt(epochSeconds: Long) = 0

            override fun nowSeconds() = System.currentTimeMillis() / 1000.0
        }
        val engine = LocalEngine(Files.createTempDirectory("meghnaad-taskrc").toFile(), vault)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        return Store(Tw(engine, clock), engine, vault, SyncController(engine, scope, clock), scope, clock, files = files)
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

    @Test
    fun theZipIsOneAnyReaderOpens() {
        val files = listOf("taskrc" to "bulk=3\ninclude holidays/en-US.rc\n", "holidays/en-US.rc" to "holiday.en-US1.name=Fête\n", "empty.rc" to "")
        // 2026-10-24 12:30:10 UTC
        val zip = RcZip.zip(files, 1_792_845_010L)
        val read = java.util.zip.ZipInputStream(zip.inputStream())
        val got = generateSequence { read.nextEntry }.map { it.name to read.readBytes().decodeToString() }.toList()
        assertEquals(files, got)
        assertEquals(java.util.zip.CRC32().also { it.update("123456789".encodeToByteArray()) }.value.toInt(), RcZip.crc32("123456789".encodeToByteArray()))
        assertEquals(22, RcZip.zip(emptyList(), 0L).size)
    }

    @Test
    fun theFolderAndAFileAreHandedToTheFilesHostAsSaved() = runBlocking {
        if (!library()) return@runBlocking
        val kept = mutableListOf<Triple<String, String, ByteArray>>()
        val host = object : FileHost {
            override suspend fun pick(): PickedFile? = null

            override suspend fun save(name: String, mime: String, text: String): Boolean {
                kept += Triple(name, mime, text.encodeToByteArray())
                return true
            }

            override suspend fun saveBytes(name: String, mime: String, bytes: ByteArray): Boolean {
                kept += Triple(name, mime, bytes)
                return true
            }
        }
        val store = store(host)
        store.loadConfig()
        val st = TaskrcState(store)
        st.load()
        st.upload(listOf(
            PickedFile("taskrc", 0, "bulk=4\ninclude holidays/en.rc\n", "dots/taskrc"),
            PickedFile("en.rc", 0, "holiday.towel.name=Towel day\nholiday.towel.date=20260525\n", "dots/holidays/en.rc"),
        ))
        // What is typed and not applied is not in a download.
        st.open("holidays/en.rc")
        st.text = "holiday.towel.name=Edited\n"
        st.downloadFile()
        assertEquals(null, st.error, st.error)
        assertEquals(Triple("en.rc", "text/plain", "holiday.towel.name=Towel day\nholiday.towel.date=20260525\n"), kept[0].let { Triple(it.first, it.second, it.third.decodeToString()) })
        assertEquals("Saved.", st.kept)

        st.downloadAll()
        assertEquals(null, st.error, st.error)
        val (name, mime, zip) = kept[1]
        assertEquals("taskrc.zip" to "application/zip", name to mime)
        val read = java.util.zip.ZipInputStream(zip.inputStream())
        val got = generateSequence { read.nextEntry }.associate { it.name to read.readBytes().decodeToString() }
        assertEquals(setOf("taskrc", "holidays/en.rc"), got.keys)
        assertEquals("bulk=4\ninclude holidays/en.rc\n", got["taskrc"])
        assertEquals("holiday.towel.name=Towel day\nholiday.towel.date=20260525\n", got["holidays/en.rc"])
    }

    private val newYear = """{"date":"2026-01-01","description":"New Year's Day","region":""}
{"date":"2026-04-20","description":"Patriots' Day","region":"MA"}
"""

    private val calendar = listOf(
        "BEGIN:VCALENDAR", "X-WR-CALNAME:Holidays in Bangladesh",
        "BEGIN:VEVENT", "DTSTART;VALUE=DATE:20260326", "DTEND;VALUE=DATE:20260327", "SUMMARY:Independence Day", "DESCRIPTION:Public holiday", "END:VEVENT",
        "BEGIN:VEVENT", "DTSTART;VALUE=DATE:20260214", "DTEND;VALUE=DATE:20260215", "SUMMARY:Valentine's Day", "DESCRIPTION:Observance", "END:VEVENT",
        "END:VCALENDAR",
    ).joinToString("\r\n")

    private suspend fun newState(): Pair<Store, TaskrcState> {
        val store = store()
        store.loadConfig()
        val st = TaskrcState(store)
        st.load()
        st.text = "bulk=3\n"
        st.save()
        return store to st
    }

    @Test
    fun holidaysFromHolidataAreSavedIncludedAndShown() = runBlocking {
        if (!library()) return@runBlocking
        val (store, st) = newState()

        val asked = mutableListOf<String>()
        val get: HolidayGet = { url, _ ->
            asked += url
            if ("/2026.json" in url) HttpAnswer(200, newYear) else HttpAnswer(404, "")
        }
        st.hHolidata = "en-US"
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
        assertEquals("bulk=3\ninclude holidays/en-US.rc\ncalendar.holidays=full\n", st.text)
        assertEquals("New Year's Day", store.config?.config?.settings?.get("holiday.en-US1.name"))
        assertEquals(emptyList(), st.folder?.problems)

        // The file knows what it was made for, so it can be fetched again.
        st.open("holidays/en-US.rc", force = true)
        assertEquals("en-US", st.made?.key)
        st.hChosen = emptyList()
        st.updateHolidays(get)
        assertEquals("en-US", st.hHolidata)
        assertEquals(1, st.hBuilt?.count)
    }

    @Test
    fun aCalendarsHolidaysAreSavedWithoutItsObservances() = runBlocking {
        if (!library()) return@runBlocking
        val (store, st) = newState()
        val get: HolidayGet = { url, _ ->
            assertEquals("https://calendar.example.org/bd.ics", url)
            HttpAnswer(200, calendar)
        }
        st.hSource = HolidaySource.Ics
        st.hLink = "webcal://calendar.example.org/bd.ics"
        st.hFirst = "2026"
        st.hLast = "2026"
        st.findHolidays(get)
        assertEquals(null, st.hError, st.hError)
        assertEquals(listOf("Independence Day"), st.hBuilt?.entries?.map { it.name })
        assertEquals(listOf(true, false), st.hBuilt?.names?.map { !it.skip }) // Independence Day, Valentine's Day

        st.toggleName("Valentine's Day", keep = true)
        assertEquals(2, st.hBuilt?.count)
        st.toggleName("Valentine's Day", keep = false)
        st.addHolidays()
        assertEquals(null, st.error, st.error)
        assertEquals("holidays/holidays-in-bangladesh.rc", st.current)
        assertEquals("Independence Day", store.config?.config?.settings?.get("holiday.ics-holidays-in-bangladesh1.name"))
        assertEquals("holidays/holidays-in-bangladesh.rc", st.made?.let { "holidays/${it.key}.rc" })
        assertEquals("https://calendar.example.org/bd.ics", st.made?.link)
    }

    @Test
    fun nagerDateCountriesAreListedAndTheOnePickedIsFetched() = runBlocking {
        if (!library()) return@runBlocking
        val (_, st) = newState()
        val get: HolidayGet = { url, _ ->
            HttpAnswer(
                200,
                if (url.endsWith("AvailableCountries")) """[{"countryCode":"BD","name":"Bangladesh"}]"""
                else """[{"date":"2026-02-21","name":"Language Day","global":true,"counties":null,"types":["Public"]}]""",
            )
        }
        st.hSource = HolidaySource.Nager
        st.loadNagerPlaces(get)
        assertEquals(listOf("BD"), st.nagerPlaces?.map { it.id })
        st.hNager = "BD"
        st.hFirst = "2026"
        st.hLast = "2026"
        st.findHolidays(get)
        assertEquals(listOf("Language Day"), st.hBuilt?.entries?.map { it.name })
        assertEquals("holidays/nager-BD.rc", st.hBuilt?.path)
    }

    @Test
    fun linksAndLocalesAreCheckedBeforeAnythingIsFetched() = runBlocking {
        if (!library()) return@runBlocking
        val none: HolidayGet = { _, _ -> error("must not be asked") }
        val bad = runCatching { fetchHolidata("../x", 2026, 2026, none) }.exceptionOrNull()
        assertTrue(bad?.message?.contains("not a year that can be asked for") == true, bad?.message)
        val offline = runCatching { fetchHolidata("en-US", 2026, 2026) { _, _ -> throw java.io.IOException("down") } }.exceptionOrNull()
        assertTrue(offline?.message?.contains("Couldn't reach holidata.net") == true, offline?.message)
        val plain = runCatching { fetchCalendar("http://example.org/a.ics", none) }.exceptionOrNull()
        assertTrue(plain?.message?.contains("https://") == true, plain?.message)
        val private = runCatching { fetchCalendar("https://127.0.0.1/a.ics", none) }.exceptionOrNull()
        assertTrue(private != null)
        // A redirect is followed, and checked like the first link.
        var hops = 0
        val text = fetchCalendar("https://example.org/a") { url, _ ->
            hops++
            if (url == "https://example.org/a") HttpAnswer(302, "", "/b.ics") else HttpAnswer(200, calendar)
        }
        assertEquals(2, hops)
        assertTrue(text.startsWith("BEGIN:VCALENDAR"))
        val away = runCatching { fetchCalendar("https://example.org/a") { _, _ -> HttpAnswer(302, "", "http://example.org/x") } }.exceptionOrNull()
        assertTrue(away != null)
        assertEquals("a=1\n", withSetting("", "a", "1"))
        assertEquals("x=2\nb=1\n", withSetting("x=1\nb=1\n", "x", "2"))
    }
}
