package me.utsob.meghnaad.app

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.runBlocking
import java.io.File
import java.nio.file.Files
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Boards against the real engine: the settings they live in, the home page, and the numbers worked out from real tasks. */
class BoardsStoreTest {
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

    private val clock = object : Clock {
        override fun utcOffsetSeconds() = 0

        override fun offsetAt(epochSeconds: Long) = 0

        override fun nowSeconds() = System.currentTimeMillis() / 1000.0
    }

    private fun withStore(block: suspend (Store, Tw) -> Unit) = runBlocking {
        if (!library()) return@runBlocking
        val engine = LocalEngine(Files.createTempDirectory("meghnaad-boards").toFile(), vault)
        val scope = CoroutineScope(SupervisorJob() + Dispatchers.Unconfined)
        val tw = Tw(engine, clock)
        val store = Store(tw, engine, vault, SyncController(engine, scope, clock), scope, clock)
        store.loadConfig()
        try {
            block(store, tw)
        } finally {
            engine.close()
            scope.cancel()
        }
    }

    private val week = """{"id":"week","name":"Week","sections":[
        {"id":"open","title":"Open","type":"number","x":0,"y":0,"w":3,"h":2,"filter":"status:pending"},
        {"id":"by","title":"By project","type":"bar","x":3,"y":0,"w":9,"h":4,"by":{"field":"project"},"filter":"status:pending"}]}"""

    @Test
    fun boardsAreReadFromTheSettingsAndTheHomePageOpensOne() = withStore { store, _ ->
        store.request("PUT", "/api/settings", """{"boardsVersion":1,"boards":[$week],"homepage":{"board":"week"}}""")
        store.loadAppSettings()
        assertEquals(listOf("week"), store.boards.map { it.id })
        assertEquals(Homepage(board = "week"), store.homepage)

        // Not forced, only from the front page.
        store.view = View.Tags
        store.goHome()
        assertEquals(View.Tags, store.view)
        store.view = View.Tasks
        store.goHome()
        assertEquals(View.Boards to "week", store.view to store.boardId)

        // Forced, from anywhere; and a home page that is gone is next.
        store.view = View.Summary
        store.goHome(force = true)
        assertEquals(View.Boards, store.view)
        store.saveHomepage(Homepage(report = "nope"))
        store.view = View.Summary
        store.goHome(force = true)
        assertEquals(View.Tasks to "next", store.view to store.report)

        // Going home leaves a board draft and the menu.
        store.saveHomepage(Homepage(board = "week"))
        store.boardEditor = BoardEditor(store, store.boards.first())
        store.menuOpen = true
        store.goHome(force = true)
        assertEquals(Triple(null, false, View.Boards), Triple(store.boardEditor, store.menuOpen, store.view))
    }

    @Test
    fun aBoardWhoseSlugChangesTakesTheHomePageWithIt() = withStore { store, _ ->
        store.request("PUT", "/api/settings", """{"boards":[$week],"homepage":{"board":"week"},"other":1}""")
        store.loadAppSettings()
        val renamed = store.boards.map { it.copy(id = "weekly") }
        store.saveBoards(renamed, Homepage(board = "weekly"), setHomepage = true)
        assertEquals(listOf("weekly"), store.boards.map { it.id })
        assertEquals(Homepage(board = "weekly"), store.homepage)
        // What the app does not know about is kept.
        assertTrue("\"other\"" in store.appSettings!!.raw.toString())
        // Deleting the last board leaves none, and the home page is taken out with it when asked.
        store.saveBoards(emptyList(), null, setHomepage = true)
        assertEquals(emptyList(), store.boards)
        assertNull(store.homepage)
    }

    @Test
    fun theHomePageSettingFollowsWhatIsSavedUnlessItWasEdited() = withStore { store, _ ->
        store.request("PUT", "/api/settings", """{"boards":[$week]}""")
        store.loadAppSettings()
        val general = GeneralState(store)
        assertNull(general.choice)
        assertEquals(false, general.edited)
        general.choice = Homepage(board = "week")
        assertTrue(general.edited)
        general.apply()
        assertEquals(Homepage(board = "week"), store.homepage)
        assertEquals(false, general.edited)
        assertEquals(false, general.missing)
        // Saved somewhere else, and not touched here: it follows.
        store.saveHomepage(null)
        general.follow()
        assertNull(general.choice)
    }

    @Test
    fun theNumbersOfABoardAreWorkedOutFromTheTasksByTheEngine() = withStore { store, tw ->
        tw.run(line = "add A project:Home +x")
        tw.run(line = "add B project:Home")
        tw.run(line = "add C project:Work")
        // By what it says, not its number: tasks added in the same second have no fixed order.
        tw.run(line = "description:B done")
        store.request("PUT", "/api/settings", """{"boards":[$week]}""")
        store.loadAppSettings()

        val data = store.loadBoardData(store.boards.single())
        val open = data.getValue("open") as SectionData.Number
        assertEquals(2.0, open.answer.total)
        assertEquals("( status:pending )", open.sources.single().filter)
        assertEquals(2, parseExportedTasks(open.sources.single().text).size)

        val by = data.getValue("by") as SectionData.Chart
        assertEquals(listOf("Home", "Work"), by.chart.labels.map { it.label })
        assertEquals(listOf(1.0, 1.0), by.chart.datasets.single().data)
    }

    @Test
    fun aCardThatCannotBeWorkedOutSaysWhyAndTheOthersStillAre() = withStore { store, tw ->
        tw.run(line = "add A project:Home")
        store.request(
            "PUT", "/api/settings",
            """{"boards":[{"id":"b","name":"B","sections":[
                {"id":"ok","type":"number","x":0,"y":0,"w":3,"h":2},
                {"id":"bad","type":"number","x":3,"y":0,"w":3,"h":2,"measure":{"fn":"sum","field":"project"}},
                {"id":"t","type":"table","x":0,"y":2,"w":12,"h":3,"report":"list","filter":"due.before:nonsense"}]}]}""",
        )
        store.loadAppSettings()
        val data = store.loadBoardData(store.boards.single())
        assertTrue(data.getValue("ok") is SectionData.Number)
        assertTrue((data.getValue("bad") as SectionData.Failed).message.contains("not a number"))
        assertTrue(data.getValue("t") is SectionData.Failed)
    }

    @Test
    fun aLinkOpensTheReportTheBoardTheSettingsTabOrThePageItNames() = withStore { store, _ ->
        store.request("PUT", "/api/settings", """{"boards":[$week],"homepage":{"board":"week"}}""")
        store.loadAppSettings()

        store.openPlace(AppPlace.Report("list"))
        assertEquals(View.Tasks to "list", store.view to store.report)
        assertEquals(AppPlace.Report("list"), store.place)
        store.openPlace(AppPlace.Boards("week"))
        assertEquals(View.Boards to "week", store.view to store.boardId)
        assertEquals("meghnaad://board/week", placeLink(store.place))
        store.openPlace(AppPlace.Boards())
        assertEquals(View.Boards to null, store.view to store.boardId)
        store.openPlace(AppPlace.SettingsAt(SettingsTab.Urgency))
        assertEquals(View.Settings to SettingsTab.Urgency, store.view to store.settingsTab)
        store.openPlace(AppPlace.Page(View.Calendar))
        assertEquals(View.Calendar, store.view)

        // A report or a board that is not there is told, and the home page opens.
        store.openPlace(AppPlace.Report("nope"))
        assertTrue(store.toast!!.text.contains("no report called nope"))
        assertEquals(View.Boards to "week", store.view to store.boardId)
        store.view = View.Tags
        store.openPlace(AppPlace.Boards("gone"))
        assertTrue(store.toast!!.text.contains("no board called gone"))
        assertEquals(View.Boards, store.view)
    }

    @Test
    fun aLinkOpenedFromOutsideBeforeTheAppIsReadyWaitsAndOnceReadyGoesThere() = withStore { store, _ ->
        store.request("PUT", "/api/settings", """{"boards":[$week]}""")
        store.loadAppSettings()
        // Not started (`begin` was not called): the link is kept for the start. A link of no app is ignored.
        store.openFromLink("https://example.com")
        store.openFromLink("meghnaad://board/week")
        assertEquals(View.Tasks, store.view)
    }

    @Test
    fun aBoardIsMadeChangedRenamedAndDeletedOnThePhone() = withStore { store, _ ->
        val made = createBoard(store, "My Week")!!
        assertEquals(listOf("my-week"), store.boards.map { it.id })

        val editor = BoardEditor(store, made)
        editor.add("number")
        editor.add("bar")
        editor.add("table")
        assertEquals(listOf("number", "bar", "table"), editor.draft.sections.map { it.id })
        // A number takes a quarter of the row, a bar half of it beside the number, and a table the whole row under them.
        assertEquals(listOf(0 to 0, 3 to 0, 0 to 5), editor.draft.sections.map { it.x to it.y })
        assertTrue(editor.valid)

        // The engine refuses a measure with nothing to measure, and the draft cannot be saved until it is put right.
        val number = editor.draft.sections.first { it.id == "number" }
        editor.update(number.copy(measure = Measure("sum")))
        assertTrue(!editor.valid)
        assertTrue(editor.check.errors.single().contains("need a field"))
        assertEquals(false, editor.save())
        editor.update(number.copy(measure = Measure("sum", "urgency")))
        assertTrue(editor.valid)

        // Moving a section changes the reading order.
        editor.move("table", -1)
        assertEquals(listOf("number", "table", "bar"), stacked(editor.draft.sections).map { it.id })

        // A new address is a new slug; the home page moves with the board.
        store.saveHomepage(Homepage(board = "my-week"))
        editor.edit { it.copy(id = "weekly") }
        assertTrue(editor.save())
        assertEquals(listOf("weekly"), store.boards.map { it.id })
        assertEquals("weekly", store.boardId)
        assertEquals(Homepage(board = "weekly"), store.homepage)
        assertEquals(listOf("number", "table", "bar"), stacked(store.boards.single().sections).map { it.id })
        assertEquals(Measure("sum", "urgency"), store.boards.single().sections.first { it.id == "number" }.measure)

        // Another board cannot have the same address, and a bad one is said.
        val other = createBoard(store, "Other")!!
        val second = BoardEditor(store, other)
        second.edit { it.copy(id = "weekly") }
        assertTrue(second.slugError!!.contains("already a board"))
        assertTrue(!second.valid)
        second.edit { it.copy(id = "Not Allowed") }
        assertTrue(second.slugError!!.contains("lower case"))
        second.edit { it.copy(id = "other") }
        assertTrue(second.valid)

        // Deleting the board that is the home page takes the home page with it.
        assertTrue(BoardEditor(store, store.boards.first { it.id == "weekly" }).delete())
        assertEquals(listOf("other"), store.boards.map { it.id })
        assertNull(store.homepage)
        assertNull(store.boardId)
    }

    @Test
    fun theBackButtonLeavesTheEditorThenTheBoard() = withStore { store, _ ->
        store.request("PUT", "/api/settings", """{"boards":[$week]}""")
        store.loadAppSettings()
        store.openPlace(AppPlace.Boards("week"))
        store.boardEditor = BoardEditor(store, store.boards.single())
        assertTrue(store.handleBack())
        assertNull(store.boardEditor)
        assertEquals(View.Boards to "week", store.view to store.boardId)
        assertTrue(store.handleBack())
        assertEquals(View.Boards to null, store.view to store.boardId)
        assertTrue(store.handleBack())
        assertEquals(View.Tasks, store.view)
    }
}
