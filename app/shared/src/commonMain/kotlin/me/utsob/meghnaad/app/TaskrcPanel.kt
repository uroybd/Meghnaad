package me.utsob.meghnaad.app

import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.compose.foundation.layout.safeDrawingPadding
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.IconButton
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Surface
import androidx.compose.material3.VerticalDivider
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.graphics.Color
import androidx.compose.runtime.remember
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay

/** The path of a file of the taskrc folder in the settings routes: the main taskrc has its own. */
private fun rcRoute(path: String) =
    if (path == RC_ROOT) "/api/config/taskrc" else "/api/config/files/" + path.split('/').joinToString("/") { percentEncode(it) }

/** One file's box: what is typed, and what is saved (to detect edits and to revert to). */
class RcDraft(text: String, saved: String) {
    var text by mutableStateOf(text)
    var saved by mutableStateOf(saved)
    val edited get() = text != saved
}

/** The taskrc tab: a folder of files, one shown at a time. Edits are kept while another file or tab is shown (TaskrcPanel.svelte). */
class TaskrcState(private val store: Store) {
    /** The files of the folder, as the engine lists them (null until it has, or if it can't). */
    var folder by mutableStateOf<RcFiles?>(null)
    /** The file shown. The link names it; the main taskrc is the default. */
    var current by mutableStateOf(RC_ROOT)
    val drafts = mutableStateMapOf<String, RcDraft>()
    var loading by mutableStateOf(true)
    var busy by mutableStateOf(false)
    var result by mutableStateOf<TaskrcResponse?>(null)
    var skipped by mutableStateOf<List<String>>(emptyList())
    var error by mutableStateOf<String?>(null)
    var confirmRestore by mutableStateOf(false)
    var confirmDelete by mutableStateOf(false)
    var link by mutableStateOf("")
    /** A new file or folder being named, in the folder [here]. */
    var naming by mutableStateOf<RcNew?>(null)
    var newName by mutableStateOf("")
    /** What is wrong with the name, checked once typing stops. */
    var nameProblem by mutableStateOf<String?>(null)
    var replace by mutableStateOf(false)
    var includeIn by mutableStateOf(RC_ROOT)
    var note by mutableStateOf<String?>(null)

    /** What the last download came to (null before one). */
    var kept by mutableStateOf<String?>(null)

    // Add holidays (HolidaysForm.svelte): where from, a country or a link and the years, then what was found, then the file it makes.
    private fun currentYear() = civil(store.clockNow(), store.utcOffset).year

    val holidataPlaces by lazy { holidataPlaces() }
    /** The bottom sheet open: Import, or Add holidays (drawers on the web). */
    var sheet by mutableStateOf<RcSheet?>(null)

    /** The list of files folded away; null until chosen, which is folded on a phone and shown on a wide screen. */
    var folded by mutableStateOf<Boolean?>(null)

    /** The editor over the whole screen: a bar with the file's name, the files as a drawer, and room to type. */
    var full by mutableStateOf(false)
    var closed by mutableStateOf<Set<String>>(emptySet())
    /** Folders made here with no file in them yet: they live only on the screen, since a folder is kept through its files. */
    var empty by mutableStateOf<List<String>>(emptyList())
    /** The folder new files and uploads go in: the last folder tapped, else the shown file's. */
    var here by mutableStateOf("")
    /** What saving the files as typed would find, worked out by the engine once typing stops (null: not yet). */
    var live by mutableStateOf<List<RcProblem>?>(null)
    var hSource by mutableStateOf(HolidaySource.Holidata)
    var hHolidata by mutableStateOf("")
    var hNager by mutableStateOf("")
    var nagerPlaces by mutableStateOf<List<HolidayPlace>?>(null)
    var hLink by mutableStateOf("")
    var hName by mutableStateOf("")
    var hFirst by mutableStateOf((currentYear() - 1).toString())
    var hLast by mutableStateOf((currentYear() + 2).toString())
    var hFetched by mutableStateOf<HolidayYears?>(null)

    /** A calendar's text and the link it came from. */
    var hCalendar by mutableStateOf<Pair<String, String>?>(null)
    var hChosen by mutableStateOf<List<String>>(emptyList())

    /** The names of a calendar to leave out; null is the calendar's observances. */
    var hSkip by mutableStateOf<List<String>?>(null)
    var hInclude by mutableStateOf(true)
    var hShow by mutableStateOf(true)
    var hLoading by mutableStateOf(false)
    var hError by mutableStateOf<String?>(null)

    private var started = false

    val draft get() = drafts[current]
    var text: String
        get() = draft?.text ?: ""
        set(value) {
            draft?.text = value
        }

    /** The file shown has edits. */
    val fileEdited get() = draft?.edited == true

    /** Some file has edits (the tab shows a dot). */
    val edited get() = drafts.values.any { it.edited }
    val paths get() = folder?.files?.map { it.path } ?: listOf(RC_ROOT)
    private val listed get() = if (RC_ROOT in paths) paths else listOf(RC_ROOT) + paths
    val rows get() = rcTree(listed, empty.filter { f -> listed.none { it.startsWith("$f/") } }, closed)
    val taken get() = listed + empty + rows.filter { it.folder }.map { it.path }
    val info get() = folder?.files?.firstOrNull { it.path == current }
    val others get() = paths.filter { it != current }
    val problems get() = (live ?: folder?.problems ?: emptyList()).filter { it.file == current }
    fun unused(path: String) = path != RC_ROOT && folder != null && folder?.files?.firstOrNull { it.path == path }?.used != true

    /** Every file's text as it is in its box (or as saved), for the check. */
    val typed get() = listed.associateWith { drafts[it]?.text ?: "" }

    /** What saving the files as typed would find: a line this app can't use, an include with no file, a credential. */
    fun check(files: Map<String, String> = typed) {
        live = try {
            val arg = kotlinx.serialization.json.JsonObject(files.mapValues { kotlinx.serialization.json.JsonPrimitive(it.value) })
            engineJson.decodeFromJsonElement(kotlinx.serialization.builtins.ListSerializer(RcProblem.serializer()), callHelper("taskrc_check", arg))
        } catch (_: Exception) {
            null
        }
    }

    /** A folder's row opens or closes it, and makes it the one new files and uploads go in. */
    fun toggle(folder: String) {
        closed = if (folder in closed) closed - folder else closed + folder
        here = folder
    }

    fun startNew(kind: RcNew) {
        naming = kind
        newName = ""
        nameProblem = null
    }

    fun nameTyped() {
        nameProblem = newName.takeIf { it.isNotBlank() }?.let { newNameProblem(here, it, taken) }
    }
    val isRoot get() = current == RC_ROOT

    val hPlace get() = if (hSource == HolidaySource.Nager) hNager else hHolidata

    /** What the source's answers make, with the regions and names chosen. */
    val hBuilt: HolidaysBuilt?
        get() {
            val lo = minOf(hFirst.trim().toIntOrNull() ?: return null, hLast.trim().toIntOrNull() ?: return null)
            val hi = maxOf(hFirst.trim().toInt(), hLast.trim().toInt())
            return when (hSource) {
                HolidaySource.Ics -> hCalendar?.let { (text, link) -> buildCalendar(hName.trim(), text, lo, hi, hSkip, link) }
                HolidaySource.Nager -> hFetched?.takeIf { hNager.isNotEmpty() }?.let { buildNager(hNager, it.bodies, hChosen) }
                HolidaySource.Holidata -> hFetched?.takeIf { hHolidata.isNotEmpty() }?.let { buildHolidata(hHolidata, it.bodies, hChosen) }
            }
        }
    val hReady get() = if (hSource == HolidaySource.Ics) hLink.isNotBlank() else hPlace.isNotEmpty()
    fun included(path: String) = folder?.files?.firstOrNull { it.path == RC_ROOT }?.includes?.any { it.found == path } == true
    val calendarOn get() = (store.config?.config?.settings?.get("calendar.holidays") ?: "none") != "none"

    /** The file shown was made by Add holidays and has no edits: it can be fetched again. */
    val made get() = draft?.takeIf { !it.edited }?.let { holidaySettings(it.saved) }

    private suspend fun refreshList() {
        try {
            folder = engineJson.decodeFromString(RcFiles.serializer(), store.request("GET", "/api/config/files"))
        } catch (_: Exception) {
            // The list is a convenience: the file shown still works.
        }
    }

    /** Open a file: its box starts from what is saved, so an edit changes it rather than replacing it with a blank page. */
    suspend fun open(path: String, force: Boolean = false) {
        current = path
        here = if ('/' in path) path.substringBeforeLast('/') else ""
        store.settingsFile = path.takeIf { it != RC_ROOT }
        confirmDelete = false
        kept = null
        if (drafts[path] != null && !force) return
        loading = true
        try {
            val body = store.request("GET", rcRoute(path))
            drafts[path] = RcDraft(body, body)
            error = null
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            loading = false
        }
    }

    /** The list and the file the link names, once. */
    suspend fun load() {
        if (!started) {
            started = true
            refreshList()
        }
        val want = store.settingsFile ?: RC_ROOT
        if (want != current || drafts[want] == null) open(want)
    }

    /** The saved settings changed somewhere else (the console's `config`, a sync): follow them, unless there are edits here. */
    suspend fun follow() {
        if (loading || edited || busy) return
        drafts.clear()
        open(current, force = true)
        refreshList()
    }

    // The engine returns what it actually saved: show that, not what was pasted, so a credential in a pasted file is gone from the
    // screen as well as from storage.
    private fun applied(body: String, path: String = current) {
        val r = engineJson.decodeFromString(TaskrcResponse.serializer(), body)
        result = r
        drafts[path] = RcDraft(r.text, r.text)
    }

    // A download holds what is saved, never the edits in a box that are not applied (nothing unsafe is in what is saved).
    suspend fun downloadFile() = run {
        val host = store.files ?: return@run
        val ok = host.save(current.substringAfterLast('/'), "text/plain", store.request("GET", rcRoute(current)))
        kept = if (ok) "Saved." else "Not saved."
    }

    /** The whole folder in one zip that keeps the folders: unzipped, it is a folder Import folder takes back. */
    suspend fun downloadAll() = run {
        val host = store.files ?: return@run
        refreshList()
        val files = (folder?.files?.map { it.path } ?: listOf(RC_ROOT)).map { it to store.request("GET", rcRoute(it)) }
        val zip = RcZip.zip(files, store.clockNow() + store.utcOffset)
        kept = if (host.saveBytes("taskrc.zip", "application/zip", zip)) "Saved." else "Not saved."
    }

    private suspend fun run(block: suspend () -> Unit) {
        busy = true
        error = null
        try {
            block()
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }

    suspend fun save() = run {
        // `include dark-256.theme`, as a desktop taskrc has it, becomes that theme's lines.
        val body = expandIncludes(text) { loadTheme(it) }
        applied(store.request("PUT", rcRoute(current), body))
        skipped = emptyList()
        store.settingsChanged()
        refreshList()
    }

    /** The engine fetches the link and saves what it keeps, as a paste does. */
    suspend fun fromLink() = run {
        val r = store.request("POST", "/api/config/taskrc/url", link.trim())
        link = ""
        sheet = null
        drafts.clear()
        open(RC_ROOT, force = true)
        applied(r, RC_ROOT)
        store.settingsChanged()
        refreshList()
    }

    suspend fun restore() {
        if (!confirmRestore) {
            confirmRestore = true
            delay(4000)
            confirmRestore = false
            return
        }
        confirmRestore = false
        run {
            val r = engineJson.decodeFromString(TaskrcResponse.serializer(), store.request("POST", "/api/config/taskrc/restore"))
            drafts.clear()
            refreshList()
            if (folder?.files?.none { it.path == current } == true) current = RC_ROOT
            open(current, force = true)
            result = r
            store.settingsChanged()
        }
    }

    /** A new file is saved empty at once; a new folder is only on the screen until a file goes in it. */
    suspend fun addNew() {
        val kind = naming ?: return
        nameProblem = newNameProblem(here, newName, taken)
        if (nameProblem != null) return
        val n = newName.trim().removePrefix("./")
        val path = if (here.isEmpty()) n else "$here/$n"
        naming = null
        newName = ""
        if (kind == RcNew.Folder) {
            empty = empty + path
            closed = closed - path
            here = path
            return
        }
        run {
            applied(store.request("PUT", rcRoute(path)), path)
            empty = empty.filter { !path.startsWith("$it/") }
            refreshList()
            open(path)
        }
    }

    suspend fun removeFile() {
        if (isRoot) return
        if (!confirmDelete) {
            confirmDelete = true
            delay(4000)
            confirmDelete = false
            return
        }
        confirmDelete = false
        run {
            val gone = current
            result = engineJson.decodeFromString(TaskrcResponse.serializer(), store.request("DELETE", rcRoute(gone)))
            drafts.remove(gone)
            open(RC_ROOT)
            store.settingsChanged()
            refreshList()
        }
    }

    /** `include <this file>` goes into another file's box (Apply keeps it), which is then shown. */
    suspend fun include() {
        val file = current
        val target = includeIn
        if (drafts[target] == null) open(target)
        val d = drafts[target] ?: return
        d.text = withInclude(d.text, target, file)
        open(target)
    }

    /** The source, the country or the link changed: what was found is no longer what was asked for. */
    fun holidaysAsked() {
        hFetched = null
        hCalendar = null
        hError = null
    }

    /** Nager.Date's countries, asked for once when it is first chosen. */
    suspend fun loadNagerPlaces(get: HolidayGet = ::httpGet) {
        if (nagerPlaces != null) return
        try {
            nagerPlaces = nagerPlaces(get)
        } catch (e: Exception) {
            hError = e.message ?: e.toString()
        }
    }

    suspend fun findHolidays(get: HolidayGet = ::httpGet) {
        if (!hReady || hLoading) return
        val first = hFirst.trim().toIntOrNull()
        val last = hLast.trim().toIntOrNull()
        if (first == null || last == null) {
            hError = "The years are numbers, like ${currentYear()}."
            return
        }
        hLoading = true
        hError = null
        try {
            when (hSource) {
                HolidaySource.Ics -> hCalendar = fetchCalendar(hLink, get).let { it to calendarLink(hLink) }
                HolidaySource.Nager -> hFetched = fetchNager(hNager, minOf(first, last), maxOf(first, last), get)
                HolidaySource.Holidata -> hFetched = fetchHolidata(hHolidata, minOf(first, last), maxOf(first, last), get)
            }
        } catch (e: Exception) {
            hFetched = null
            hCalendar = null
            hError = e.message ?: e.toString()
        } finally {
            hLoading = false
        }
    }

    /** Fetch the holidays of the file shown again, for the years now, with what it was made for. */
    suspend fun updateHolidays(get: HolidayGet = ::httpGet) {
        val was = made ?: return
        val source = HolidaySource.entries.firstOrNull { it.id == was.source } ?: return
        sheet = RcSheet.Holidays
        hSource = source
        hChosen = was.regions
        hSkip = if (source == HolidaySource.Ics) was.skip else null
        when (source) {
            HolidaySource.Ics -> { hName = was.key; hLink = was.link ?: "" }
            HolidaySource.Nager -> hNager = was.key
            HolidaySource.Holidata -> hHolidata = was.key
        }
        holidaysAsked()
        if (source != HolidaySource.Ics || was.link != null) findHolidays(get)
    }

    fun toggleRegion(name: String, on: Boolean) {
        hChosen = if (on) hChosen + name else hChosen - name
    }

    fun toggleName(name: String, keep: Boolean) {
        val left = hBuilt?.names?.filter { it.skip }?.map { it.name } ?: emptyList()
        hSkip = if (keep) left - name else left + name
    }

    /**
     * Save the holiday file, and the lines of the main taskrc that use it. If the main taskrc has edits that are not applied, the lines go
     * into its box instead of over them.
     */
    suspend fun addHolidays() {
        val built = hBuilt ?: return
        saveHolidays(built.path, built.text, hInclude && !included(built.path), hShow && !calendarOn)
        holidaysAsked()
    }

    /** A holiday file picked: it goes in holidays/, and the main taskrc includes it and turns the calendar on. */
    suspend fun holidayFile(picked: PickedFile?) {
        val f = picked ?: return
        val text = f.text
        if (text == null) {
            error = "${f.name} is too large."
            return
        }
        val path = "holidays/" + f.name.trimStart('.').replace('/', '-').replace('\\', '-').ifEmpty { "holidays.rc" }
        saveHolidays(path, text, !included(path), !calendarOn)
    }

    private suspend fun saveHolidays(path: String, text: String, wantInclude: Boolean, wantCalendar: Boolean) = run {
        note = null
        applied(store.request("PUT", rcRoute(path), text), path)
        val saved = result
        if (wantInclude || wantCalendar) {
            val root = drafts[RC_ROOT]
            val before = root?.text ?: store.request("GET", rcRoute(RC_ROOT))
            var next = before
            if (wantInclude) next = withInclude(next, RC_ROOT, path)
            if (wantCalendar) next = withSetting(next, "calendar.holidays", "full")
            if (next != before && root != null && root.edited) {
                root.text = next
                note = "The lines for the holidays are in the taskrc box; Apply there keeps them."
            } else if (next != before) {
                applied(store.request("PUT", rcRoute(RC_ROOT), next), RC_ROOT)
            }
        }
        result = saved
        sheet = null
        store.settingsChanged()
        refreshList()
        open(path, force = true)
    }

    /**
     * Add the files picked (into the folder [here]) or a whole folder (as it is; with [replace], it is the folder now): one step, one
     * version to go back to.
     */
    suspend fun upload(picked: List<PickedFile>, whole: Boolean = true) {
        val got = uploadFiles(picked.map { Picked(it.name, it.relative, it.text) })
        val up = if (whole || here.isEmpty()) got else Upload(got.files.map { (p, t) -> "$here/$p" to t }, got.skipped)
        skipped = up.skipped
        if (up.files.isEmpty()) {
            error = "There was no taskrc file to add."
            return
        }
        run {
            val body = kotlinx.serialization.json.buildJsonObject {
                put("mode", kotlinx.serialization.json.JsonPrimitive(if (whole && replace) "replace" else "merge"))
                put("files", kotlinx.serialization.json.buildJsonArray {
                    for ((path, text) in up.files) add(kotlinx.serialization.json.buildJsonObject {
                        put("path", kotlinx.serialization.json.JsonPrimitive(path))
                        put("text", kotlinx.serialization.json.JsonPrimitive(expandIncludes(text) { loadTheme(it) }))
                    })
                })
            }
            val r = engineJson.decodeFromString(TaskrcResponse.serializer(), store.request("POST", "/api/config/files/import", body.toString()))
            drafts.clear()
            sheet = null
            empty = empty.filter { f -> up.files.none { it.first.startsWith("$f/") } }
            refreshList()
            if (folder?.files?.none { it.path == current } == true) current = RC_ROOT
            open(current, force = true)
            result = r
            store.settingsChanged()
        }
    }
}

enum class RcNew { File, Folder }

enum class RcSheet { Import, Holidays }

@OptIn(ExperimentalLayoutApi::class, ExperimentalMaterial3Api::class)
@Composable
fun TaskrcPanel(store: Store, st: TaskrcState) {
    val colors = MaterialTheme.colorScheme
    LaunchedEffect(Unit) { st.load() }
    // The link changed (a link, the back button): show that file.
    LaunchedEffect(store.settingsFile) { if (!st.loading && (store.settingsFile ?: RC_ROOT) != st.current) st.open(store.settingsFile ?: RC_ROOT) }
    // The saved settings changed somewhere else (the console's `config`, Task colours, a sync): follow them, unless there are edits here.
    LaunchedEffect(store.config) { st.follow() }
    // The check runs once typing stops, so typing stays quick.
    val typed = st.typed
    LaunchedEffect(typed) {
        delay(300)
        st.check(typed)
    }
    // The name of a new file or folder is checked once typing stops.
    LaunchedEffect(st.newName) {
        delay(300)
        st.nameTyped()
    }
    var about by remember { mutableStateOf(false) }

    Column(Modifier.fillMaxSize().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("taskrc", fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
                Dim("A copy of your taskrc, saved with your tasks.", Modifier.weight(1f), 13)
                IconButton(onClick = { about = !about }) { LucideIcon("info", size = 16.dp) }
            }
            if (about) {
                Dim(
                    "UDAs, custom reports, contexts, holidays and journal settings live in your ~/.taskrc, not in your synced data, so this app keeps a copy: " +
                        "a folder of files, as written. include works as in Taskwarrior, with the path written from the file that has it. " +
                        "They are saved with your tasks and stay until you change them. A line this app can't use is commented out, not lost. " +
                        "Sync settings and anything that looks like a credential are removed; they are never stored, logged or shown. The colours are set in Appearance.",
                    size = 13,
                )
            }
            store.config?.configError?.let {
                Text("The saved settings couldn't be read ($it). Tasks still work with the defaults. Importing replaces them; the unreadable copy is kept aside.", color = colors.error, fontSize = 13.sp)
            }
            if (!st.full) {
                Box(Modifier.weight(1f).fillMaxWidth().border(1.dp, colors.outlineVariant, RoundedCornerShape(8.dp))) { RcWorkspace(store, st) }
            }
        }
    if (st.full) {
        // Over everything, the console and the keyboard's room included: the file's name in a bar on top, the files in a
        // drawer from the side, and the rest of the screen for the text.
        Dialog(onDismissRequest = { st.full = false }, properties = DialogProperties(usePlatformDefaultWidth = false, dismissOnClickOutside = false)) {
            Surface(Modifier.fillMaxSize(), color = colors.surface) {
                Box(Modifier.fillMaxSize().safeDrawingPadding()) { RcWorkspace(store, st) }
            }
        }
    }

    when (st.sheet) {
        RcSheet.Holidays -> ModalBottomSheet(onDismissRequest = { st.sheet = null }) {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Add holidays", fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                HolidaysSection(store, st)
            }
        }
        RcSheet.Import -> ModalBottomSheet(onDismissRequest = { st.sheet = null }) {
            Column(Modifier.fillMaxWidth().verticalScroll(rememberScrollState()).padding(horizontal = 16.dp).padding(bottom = 32.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("Import", fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    LucideIcon("link", size = 14.dp)
                    Text("From a link", fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                }
                OutlinedTextField(
                    st.link, { st.link = it }, Modifier.fillMaxWidth(), singleLine = true,
                    placeholder = { Text("https://raw.githubusercontent.com/you/dotfiles/main/.taskrc", fontSize = 12.sp, maxLines = 1) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                )
                OutlinedButton(enabled = !st.busy && st.link.isNotBlank(), onClick = { store.go { st.fromLink() } }) { Text("Import") }
                Dim("Fetched and saved as the main taskrc straight away (Restore previous undoes it). GitHub and GitLab file pages work too. A public link; keep it free of passwords. Files it includes are not fetched: add them with + New.", size = 12)
                if (store.files != null) {
                    HorizontalDivider(Modifier.padding(vertical = 8.dp))
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        LucideIcon("folder-up", size = 14.dp)
                        Text("A folder, as it is", fontSize = 14.sp, fontWeight = FontWeight.SemiBold)
                    }
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(checked = st.replace, onCheckedChange = { st.replace = it })
                        Text("Replace what is here (needs a taskrc)", fontSize = 13.sp)
                    }
                    OutlinedButton(enabled = !st.busy, onClick = { store.go { st.upload(files?.pickFolder() ?: emptyList()) } }) { Text("Choose a folder") }
                    Dim("Without Replace, the files are added to what is here, and same names are overwritten.", size = 12)
                }
            }
        }
        null -> {}
    }
}

/** The files and the editor. A phone: the list of files is folded away, and opens over the editor. A wide screen: it sits beside it. */
@Composable
private fun RcWorkspace(store: Store, st: TaskrcState) {
    val colors = MaterialTheme.colorScheme
    BoxWithConstraints(Modifier.fillMaxSize()) {
        val wide = maxWidth >= 760.dp
        val folded = st.folded ?: !wide
        Row(Modifier.fillMaxSize()) {
            if (wide && !folded) {
                RcSidebar(store, st, Modifier.width(240.dp).fillMaxHeight())
                VerticalDivider()
            }
            EditorPane(store, st, folded, Modifier.weight(1f).fillMaxHeight())
        }
        if (!wide && !folded) {
            Box(Modifier.fillMaxSize().background(Color.Black.copy(alpha = 0.35f)).clickable { st.folded = true })
            Surface(Modifier.fillMaxHeight().width(280.dp), shadowElevation = 8.dp, color = colors.surface) {
                RcSidebar(store, st, Modifier.fillMaxSize())
            }
        }
    }
}

/** The files as a tree, with + New on top (every way to add something) and the zip at the bottom (RcTree.svelte). */
@Composable
private fun RcSidebar(store: Store, st: TaskrcState, modifier: Modifier) {
    val colors = MaterialTheme.colorScheme
    var menu by remember { mutableStateOf(false) }
    Column(modifier) {
        Row(Modifier.fillMaxWidth().padding(8.dp), verticalAlignment = Alignment.CenterVertically) {
            Box {
                OutlinedButton(onClick = { menu = true }, enabled = !st.busy) {
                    LucideIcon("plus", size = 15.dp)
                    Text("  New", fontWeight = FontWeight.SemiBold)
                }
                DropdownMenu(expanded = menu, onDismissRequest = { menu = false }) {
                    @Composable
                    fun item(icon: String, label: String, onClick: () -> Unit) = DropdownMenuItem(
                        text = { Text(label) }, leadingIcon = { LucideIcon(icon, size = 16.dp) },
                        onClick = { menu = false; onClick() },
                    )
                    item("file-plus", "New file") { st.startNew(RcNew.File) }
                    item("folder-plus", "New folder") { st.startNew(RcNew.Folder) }
                    if (store.files != null) {
                        HorizontalDivider()
                        item("upload", "Upload files") { store.go { st.upload(files?.pickMany() ?: emptyList(), whole = false) } }
                        item("folder-up", "Upload a folder") { store.go { st.upload(files?.pickFolder() ?: emptyList()) } }
                    }
                    item("link", "Import from a link…") { st.sheet = RcSheet.Import }
                    HorizontalDivider()
                    item("calendar-plus", "Add holidays…") { st.sheet = RcSheet.Holidays }
                    if (store.files != null) item("upload", "Upload a holiday file") { store.go { st.holidayFile(files?.pick()) } }
                    Dim(
                        "New files and uploads go in " + st.here.ifEmpty { "the top folder" } + ".",
                        Modifier.padding(horizontal = 12.dp, vertical = 6.dp).widthIn(max = 240.dp), 12,
                    )
                }
            }
            Spacer(Modifier.weight(1f))
            IconButton(onClick = { st.folded = true }) { LucideIcon("panel-left-close", size = 16.dp) }
        }
        st.naming?.let { kind ->
            Column(Modifier.padding(horizontal = 8.dp).padding(bottom = 8.dp)) {
                OutlinedTextField(
                    st.newName, { st.newName = it }, Modifier.fillMaxWidth(), singleLine = true,
                    label = { Text((if (kind == RcNew.File) "New file" else "New folder") + st.here.takeIf { it.isNotEmpty() }?.let { " in $it" }.orEmpty()) },
                    placeholder = { Text(if (kind == RcNew.File) "en-US.rc" else "folder", fontSize = 12.sp) },
                    isError = st.nameProblem != null,
                    keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                )
                st.nameProblem?.let { Text(it, color = extras.warn, fontSize = 12.sp) }
                Row {
                    TextButton(onClick = { st.naming = null }) { Text("Cancel") }
                    TextButton(enabled = !st.busy && st.newName.isNotBlank(), onClick = { store.go { st.addNew() } }) { Text("Add") }
                }
            }
        }
        Column(Modifier.weight(1f).verticalScroll(rememberScrollState())) {
            for (r in st.rows) {
                if (r.folder) {
                    Row(
                        Modifier.fillMaxWidth().clickable { st.toggle(r.path) }
                            .padding(start = (r.depth * 14 + 6).dp, top = 8.dp, bottom = 8.dp, end = 8.dp)
                            .alpha(if (r.empty) 0.6f else 1f),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(5.dp),
                    ) {
                        LucideIcon(if (r.open) "chevron-down" else "chevron-right", size = 14.dp)
                        LucideIcon(if (r.open) "folder-open" else "folder", size = 14.dp)
                        Text(r.name, Modifier.weight(1f), fontSize = 14.sp, color = if (r.path == st.here) colors.onSurface else colors.onSurfaceVariant)
                        if (r.empty) Dim("empty, not saved", size = 11)
                    }
                } else {
                    val on = r.path == st.current
                    val d = st.drafts[r.path]
                    Row(
                        Modifier.fillMaxWidth()
                            .background(if (on) colors.surfaceVariant else Color.Transparent)
                            .clickable {
                                store.go { st.open(r.path) }
                                // On a phone the list is over the editor: the file chosen is what is wanted next.
                                if (st.folded != false) st.folded = null
                            }
                            .padding(start = (r.depth * 14 + 25).dp, top = 9.dp, bottom = 9.dp, end = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(5.dp),
                    ) {
                        LucideIcon("file-text", size = 14.dp)
                        Text(r.name, Modifier.weight(1f), fontSize = 14.sp, fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal)
                        // Not applied yet.
                        if (d != null && d.edited) Box(Modifier.size(7.dp).background(colors.primary, CircleShape))
                        if (st.unused(r.path)) Dim("unused", size = 11)
                    }
                }
            }
        }
        if (store.files != null) {
            HorizontalDivider()
            TextButton(onClick = { store.go { st.downloadAll() } }, enabled = !st.busy, modifier = Modifier.fillMaxWidth().padding(4.dp)) {
                LucideIcon("download", size = 14.dp)
                Text("  Download all (.zip)", Modifier.weight(1f), fontSize = 13.sp)
            }
        }
    }
}

/** The file shown: its name and ⋯, the editor taking the height, and under it what was found and the buttons, always in view. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun EditorPane(store: Store, st: TaskrcState, folded: Boolean, modifier: Modifier) {
    val colors = MaterialTheme.colorScheme
    var more by remember { mutableStateOf(false) }
    Column(modifier.padding(horizontal = 10.dp, vertical = 6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        // The file's bar: the files, its name, the editor over the whole screen or back, and the rest in ⋯.
        Row(
            Modifier.then(if (st.full) Modifier.clip(RoundedCornerShape(8.dp)).background(colors.surfaceContainer) else Modifier),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            if (folded) IconButton(onClick = { st.folded = false }) { LucideIcon("panel-left-open", size = 16.dp) }
            Text(st.current, Modifier.weight(1f).padding(start = if (folded) 0.dp else 8.dp), fontSize = 14.sp, fontFamily = FontFamily.Monospace, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
            IconButton(onClick = { st.full = !st.full }) { LucideIcon(if (st.full) "minimize-2" else "maximize-2", size = 16.dp) }
            Box {
                IconButton(onClick = { more = true }) { LucideIcon("ellipsis", size = 16.dp) }
                DropdownMenu(expanded = more, onDismissRequest = { more = false }) {
                    if (store.files != null) {
                        DropdownMenuItem(
                            text = { Text("Download") }, leadingIcon = { LucideIcon("download", size = 16.dp) },
                            enabled = !st.busy && !st.loading && st.draft != null,
                            onClick = { more = false; store.go { st.downloadFile() } },
                        )
                    }
                    if (st.made != null) {
                        DropdownMenuItem(
                            text = { Text("Update holidays") }, leadingIcon = { LucideIcon("calendar-days", size = 16.dp) },
                            enabled = !st.busy && !st.hLoading,
                            onClick = { more = false; store.go { st.updateHolidays() } },
                        )
                    }
                    if (!st.isRoot && st.others.isNotEmpty()) {
                        HorizontalDivider()
                        Column(Modifier.padding(horizontal = 12.dp, vertical = 6.dp).widthIn(max = 280.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            Dim("Include this file in", size = 12)
                            val at = st.others.indexOf(st.includeIn).takeIf { it >= 0 } ?: 0
                            Choice(st.others[at], st.others) { st.includeIn = st.others[it] }
                            Dim("Adds ${includeLine(st.others[at], st.current)} to that file; Apply keeps it.", size = 12)
                            OutlinedButton(onClick = { more = false; st.includeIn = st.others[at]; store.go { st.include() } }) { Text("Include") }
                        }
                        HorizontalDivider()
                    }
                    if (!st.isRoot) {
                        DropdownMenuItem(
                            text = { Text(if (st.confirmDelete) "Tap again to delete" else "Delete file", color = colors.error) },
                            leadingIcon = { LucideIcon("trash-2", size = 16.dp) },
                            enabled = !st.busy,
                            onClick = { store.go { st.removeFile() } },
                        )
                    }
                }
            }
        }
        st.info?.takeIf { it.includes.isNotEmpty() }?.let { i ->
            Dim("Includes " + i.includes.joinToString(", ") { it.target + if (it.found == null) " (not found)" else "" } + ".", size = 12)
        }

        Box(Modifier.weight(1f).fillMaxWidth().verticalScroll(rememberScrollState())) {
            RcTextField(
                st.text, { st.text = it }, enabled = !st.loading && st.draft != null,
                placeholder = if (st.loading) "Loading…" else if (st.isRoot) "Nothing saved yet. Paste your taskrc here." else "An empty file.",
                problems = st.problems.map { it.line }.toSet(),
                modifier = Modifier.fillMaxWidth(),
            )
        }

        Column(Modifier.heightIn(max = 160.dp).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            for (p in st.problems) Text("Line ${p.line}: ${p.message}", color = extras.warn, fontSize = 13.sp)
            st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
            st.note?.let { Dim(it, size = 13) }
            if (st.skipped.isNotEmpty()) Text("Left out: ${st.skipped.joinToString(", ")}", color = extras.warn, fontSize = 12.sp, fontFamily = FontFamily.Monospace)
            st.kept?.let { Dim(it, size = 12) }
            st.result?.let { r ->
                Text("Saved ${r.udas} UDA${if (r.udas == 1) "" else "s"}, ${r.reports} report${if (r.reports == 1) "" else "s"}, ${r.contexts} context${if (r.contexts == 1) "" else "s"}.", color = extras.ok, fontSize = 13.sp)
                if (r.blocked.isNotEmpty()) Text("Removed for safety (${r.blocked.size}): ${r.blocked.joinToString(", ")}", fontSize = 12.sp, fontFamily = FontFamily.Monospace)
                if (r.commented.isNotEmpty()) {
                    Dim("${r.commented.size} line${if (r.commented.size == 1) "" else "s"} commented out", size = 12)
                    for (c in r.commented) Dim((c.file?.let { "$it:" } ?: "line ") + "${c.line} ${c.message}", size = 12)
                }
                r.warnings.forEach { Text(it, color = extras.warn, fontSize = 13.sp) }
                if (r.ignored.isNotEmpty()) {
                    // Closed until asked for, as the web's: the list can be long.
                    var showIgnored by remember(r) { mutableStateOf(false) }
                    Row(Modifier.clickable { showIgnored = !showIgnored }.padding(vertical = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        LucideIcon(if (showIgnored) "chevron-down" else "chevron-right", size = 13.dp)
                        Dim("${r.ignored.size} other settings not used", size = 12)
                    }
                    if (showIgnored) Text(r.ignored.joinToString(", "), fontSize = 12.sp, fontFamily = FontFamily.Monospace, color = colors.onSurfaceVariant)
                }
            }
        }
        // Wraps on a phone: in one row the buttons before Apply took its width, leaving it there but not drawn.
        FlowRow(
            Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.End),
            itemVerticalAlignment = Alignment.CenterVertically,
        ) {
            if (st.fileEdited) TextButton(onClick = { st.draft?.let { it.text = it.saved } }) { Text("Revert edits") }
            if (store.config?.hasPrevious == true) {
                TextButton(onClick = { store.go { st.restore() } }, enabled = !st.busy) {
                    LucideIcon("rotate-ccw", size = 14.dp)
                    Text("  " + if (st.confirmRestore) "Tap again to restore" else "Restore previous", fontSize = 13.sp)
                }
            }
            Button(onClick = { store.go { st.save() } }, enabled = !st.busy && !st.loading && st.fileEdited) { Text("Apply") }
        }
    }
}

/** Add holidays, in its sheet: where from, a country or a link and the years, what was found, and the file that makes. */
@Composable
private fun HolidaysSection(store: Store, st: TaskrcState) {
    val colors = MaterialTheme.colorScheme
    LaunchedEffect(st.hSource) { if (st.hSource == HolidaySource.Nager) st.loadNagerPlaces() }
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Dim(
            "Public holidays for a country, saved as a file here and shown in the calendar. Holidata is the data behind Taskwarrior's own holiday " +
                "files. Nager.Date covers more countries, but for some it lists only the days that fall on the same date every year. A calendar " +
                "link (a public .ics feed, such as Google's holiday calendars) covers whatever the calendar does.",
            size = 12,
        )
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dim("From")
            Choice(st.hSource.label, HolidaySource.entries.map { it.label }) {
                st.hSource = HolidaySource.entries[it]
                st.holidaysAsked()
            }
        }
        when (st.hSource) {
            HolidaySource.Holidata -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Dim("Country")
                Choice(st.holidataPlaces.firstOrNull { it.id == st.hHolidata }?.label ?: "choose…", st.holidataPlaces.map { it.label }) {
                    st.hHolidata = st.holidataPlaces[it].id
                    st.holidaysAsked()
                }
            }
            HolidaySource.Nager -> Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Dim("Country")
                val places = st.nagerPlaces
                if (places == null) Dim("loading…") else Choice(places.firstOrNull { it.id == st.hNager }?.label ?: "choose…", places.map { it.label }) {
                    st.hNager = places[it].id
                    st.holidaysAsked()
                }
            }
            HolidaySource.Ics -> OutlinedTextField(
                st.hLink, { st.hLink = it; st.holidaysAsked() }, Modifier.fillMaxWidth(), singleLine = true,
                label = { Text("Link to the calendar") },
                placeholder = { Text("https://calendar.google.com/calendar/ical/…/basic.ics", fontSize = 12.sp, maxLines = 1) },
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dim("From")
            OutlinedTextField(
                st.hFirst, { st.hFirst = it; if (st.hSource != HolidaySource.Ics) st.holidaysAsked() }, Modifier.width(96.dp), singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            )
            Dim("to")
            OutlinedTextField(
                st.hLast, { st.hLast = it; if (st.hSource != HolidaySource.Ics) st.holidaysAsked() }, Modifier.width(96.dp), singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            )
        }
        if (st.hSource == HolidaySource.Ics) Dim("A public link, with no password in it. Nothing is saved until you choose Save.", size = 12)
        OutlinedButton(enabled = st.hReady && !st.hLoading && !st.busy, onClick = { store.go { st.findHolidays() } }) {
            Text(if (st.hLoading) "Fetching…" else "Find holidays")
        }
        st.hError?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        val built = st.hBuilt
        if (built != null && (st.hFetched != null || st.hCalendar != null)) {
            val parts = mutableListOf("${built.count} holiday${if (built.count == 1) "" else "s"}")
            if (built.title.isNotEmpty()) parts += "in ${built.title}"
            st.hFetched?.missing?.takeIf { it.isNotEmpty() }?.let { parts += "(none for ${it.joinToString(", ")})" }
            if (built.skipped > 0) parts += "(${built.skipped} entr${if (built.skipped == 1) "y" else "ies"} could not be used)"
            Text(parts.joinToString(" ") + ".", fontSize = 13.sp)
            if (st.hSource == HolidaySource.Nager) {
                Dim("Look the list over: Nager.Date may not have the holidays that move with the moon (Eid, Diwali) for every country.", size = 12)
            }
            if (st.hSource == HolidaySource.Ics && built.names.isNotEmpty()) {
                Dim("Keep", size = 12)
                for (n in built.names) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(checked = !n.skip, onCheckedChange = { st.toggleName(n.name, it) })
                        Text(n.name + (if (n.count > 1) " ×${n.count}" else "") + (if (n.kind.isNotEmpty()) "  ${n.kind}" else ""), fontSize = 13.sp)
                    }
                }
            }
            if (built.regions.isNotEmpty()) {
                Dim("Also the holidays of", size = 12)
                for (r in built.regions) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(checked = r in st.hChosen, onCheckedChange = { st.toggleRegion(r, it) })
                        Text(r, fontSize = 13.sp)
                    }
                }
            }
            for (e in built.entries.take(60)) {
                Text("${e.date.take(4)}-${e.date.substring(4, 6)}-${e.date.substring(6)}" + (e.end?.let { " – ${it.take(4)}-${it.substring(4, 6)}-${it.substring(6)}" } ?: "") + "  ${e.name}", fontSize = 12.sp, fontFamily = FontFamily.Monospace)
            }
            if (built.entries.size > 60) Dim("and ${built.entries.size - 60} more", size = 12)
            if (st.hSource == HolidaySource.Ics) {
                OutlinedTextField(
                    st.hName, { st.hName = it }, Modifier.fillMaxWidth(), singleLine = true, label = { Text("Name of the file") },
                    placeholder = { Text(built.key, fontSize = 12.sp, maxLines = 1) },
                    keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                )
            }
            if (!st.included(built.path)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = st.hInclude, onCheckedChange = { st.hInclude = it })
                    Text("Include it in taskrc", fontSize = 13.sp)
                }
            }
            if (!st.calendarOn) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = st.hShow, onCheckedChange = { st.hShow = it })
                    Text("Show holidays in the calendar (calendar.holidays=full)", fontSize = 13.sp)
                }
            }
            Button(enabled = !st.busy && built.count > 0, onClick = { store.go { st.addHolidays() } }) { Text("Save ${built.path}") }
            Dim("Replaces that file if it is there; Restore previous undoes it.", size = 12)
        }
    }
}
