package me.utsob.meghnaad.app

import androidx.compose.runtime.remember
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
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
    var newName by mutableStateOf("")
    var replace by mutableStateOf(false)
    var includeIn by mutableStateOf(RC_ROOT)
    var note by mutableStateOf<String?>(null)

    // Add holidays (HolidaysForm.svelte): a country and the years, then what Holidata has, then the file it makes.
    private fun currentYear() = civil(store.clockNow(), store.utcOffset).year

    val locales by lazy { holidayLocales() }
    var holidaysOpen by mutableStateOf(false)
    var hLocale by mutableStateOf("")
    var hFirst by mutableStateOf((currentYear() - 1).toString())
    var hLast by mutableStateOf((currentYear() + 2).toString())
    var hFetched by mutableStateOf<HolidayYears?>(null)
    var hChosen by mutableStateOf<List<String>>(emptyList())
    var hInclude by mutableStateOf(true)
    var hCalendar by mutableStateOf(true)
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
    val coloursOff get() = colourOff(text)
    val paths get() = folder?.files?.map { it.path } ?: listOf(RC_ROOT)
    val rows get() = listing(if (RC_ROOT in paths) paths else listOf(RC_ROOT) + paths)
    val info get() = folder?.files?.firstOrNull { it.path == current }
    val others get() = paths.filter { it != current }
    val problems get() = folder?.problems?.filter { it.file == current } ?: emptyList()
    val isRoot get() = current == RC_ROOT

    /** What the years' files make, with the regions chosen. */
    val hBuilt get() = hFetched?.takeIf { hLocale.isNotEmpty() }?.let { buildHolidays(hLocale, it.bodies, hChosen) }
    val hPath get() = holidayPath(hLocale)
    val hIncluded get() = folder?.files?.firstOrNull { it.path == RC_ROOT }?.includes?.any { it.found == hPath } == true
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
        store.settingsFile = path.takeIf { it != RC_ROOT }
        confirmDelete = false
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

    /** A theme: its colour lines replace the box's colour lines (Apply keeps them). */
    fun pickTheme(id: String) {
        error = null
        try {
            text = applyTheme(text, id, if (id == "meghnaad") "" else loadTheme(id))
            result = null
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        }
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

    suspend fun addFile() = run {
        val path = newName.trim().removePrefix("./")
        if (path.isEmpty()) return@run
        if (path in paths) {
            open(path)
            newName = ""
            return@run
        }
        applied(store.request("PUT", rcRoute(path)), path)
        newName = ""
        refreshList()
        open(path)
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

    /** The country or the years changed: what was fetched is no longer what was asked for. */
    fun holidaysAsked() {
        hFetched = null
        hError = null
    }

    suspend fun findHolidays(get: suspend (String) -> HttpAnswer = ::httpGet) {
        val first = hFirst.trim().toIntOrNull()
        val last = hLast.trim().toIntOrNull()
        if (hLocale.isEmpty() || hLoading) return
        if (first == null || last == null) {
            hError = "The years are numbers, like ${currentYear()}."
            return
        }
        hLoading = true
        hError = null
        try {
            hFetched = fetchHolidays(hLocale, minOf(first, last), maxOf(first, last), get)
        } catch (e: Exception) {
            hFetched = null
            hError = e.message ?: e.toString()
        } finally {
            hLoading = false
        }
    }

    /** Fetch the holidays of the file shown again, for the years now, with the regions it was made with. */
    suspend fun updateHolidays(get: suspend (String) -> HttpAnswer = ::httpGet) {
        val (locale, regions) = made ?: return
        holidaysOpen = true
        hLocale = locale
        hChosen = regions
        hFetched = null
        findHolidays(get)
    }

    fun toggleRegion(name: String, on: Boolean) {
        hChosen = if (on) hChosen + name else hChosen - name
    }

    /**
     * Save the holiday file, and the lines of the main taskrc that use it. If the main taskrc has edits that are not applied, the lines go
     * into its box instead of over them.
     */
    suspend fun addHolidays() = run {
        val built = hBuilt ?: return@run
        val path = hPath
        note = null
        applied(store.request("PUT", rcRoute(path), built.text), path)
        val saved = result
        val wantInclude = hInclude && !hIncluded
        val wantCalendar = hCalendar && !calendarOn
        if (wantInclude || wantCalendar) {
            val root = drafts[RC_ROOT]
            val before = root?.text ?: store.request("GET", rcRoute(RC_ROOT))
            var next = before
            if (wantInclude) next = withInclude(next, RC_ROOT, path)
            if (wantCalendar) next = withSetting(next, "calendar.holidays", "sparse")
            if (next != before && root != null && root.edited) {
                root.text = next
                note = "The lines for the holidays are in the taskrc box; Apply there keeps them."
            } else if (next != before) {
                applied(store.request("PUT", rcRoute(RC_ROOT), next), RC_ROOT)
            }
        }
        result = saved
        hFetched = null
        store.settingsChanged()
        refreshList()
        open(path, force = true)
    }

    /** Add the files picked, or make them the whole folder: one step, one version to go back to. */
    suspend fun upload(picked: List<PickedFile>) {
        val up = uploadFiles(picked.map { Picked(it.name, it.relative, it.text) })
        skipped = up.skipped
        if (up.files.isEmpty()) {
            error = "There was no taskrc file to add."
            return
        }
        run {
            val body = kotlinx.serialization.json.buildJsonObject {
                put("mode", kotlinx.serialization.json.JsonPrimitive(if (replace) "replace" else "merge"))
                put("files", kotlinx.serialization.json.buildJsonArray {
                    for ((path, text) in up.files) add(kotlinx.serialization.json.buildJsonObject {
                        put("path", kotlinx.serialization.json.JsonPrimitive(path))
                        put("text", kotlinx.serialization.json.JsonPrimitive(expandIncludes(text) { loadTheme(it) }))
                    })
                })
            }
            val r = engineJson.decodeFromString(TaskrcResponse.serializer(), store.request("POST", "/api/config/files/import", body.toString()))
            drafts.clear()
            refreshList()
            if (folder?.files?.none { it.path == current } == true) current = RC_ROOT
            open(current, force = true)
            result = r
            store.settingsChanged()
        }
    }
}

@Composable
fun TaskrcPanel(store: Store, st: TaskrcState) {
    val colors = MaterialTheme.colorScheme
    LaunchedEffect(Unit) { st.load() }
    // The link changed (a link, the back button): show that file.
    LaunchedEffect(store.settingsFile) { if (!st.loading && (store.settingsFile ?: RC_ROOT) != st.current) st.open(store.settingsFile ?: RC_ROOT) }
    // The saved settings changed somewhere else (the console's `config`, a sync): follow them, unless there are edits here.
    LaunchedEffect(store.config) { st.follow() }

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("taskrc", fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
        Dim(
            "UDAs, custom reports, contexts, holidays and journal settings live in your ~/.taskrc, not in your synced data, so this app keeps a copy: " +
                "a folder of files, as written. include works as in Taskwarrior, with the path written from the file that has it. " +
                "They are saved with your tasks and stay until you change them. A line this app can't use is commented out, not lost. " +
                "Sync settings and anything that looks like a credential are removed; they are never stored, logged or shown.",
            size = 13,
        )
        store.config?.configError?.let {
            Text("The saved settings couldn't be read ($it). Tasks still work with the defaults. Importing below replaces them; the unreadable copy is kept aside.", color = colors.error, fontSize = 13.sp)
        }

        // The files.
        Column(Modifier.fillMaxWidth()) {
            for (r in st.rows) {
                if (r.path.isEmpty()) {
                    Row(Modifier.padding(start = (r.depth * 14 + 6).dp, top = 4.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                        LucideIcon("folder", size = 14.dp)
                        Dim(r.name, size = 13)
                    }
                } else {
                    val on = r.path == st.current
                    val d = st.drafts[r.path]
                    Row(
                        Modifier.fillMaxWidth()
                            .background(if (on) colors.surfaceVariant else colors.background)
                            .clickable { store.go { st.open(r.path) } }
                            .padding(start = (r.depth * 14 + 6).dp, top = 8.dp, bottom = 8.dp, end = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(5.dp),
                    ) {
                        LucideIcon("file-text", size = 14.dp)
                        Text(r.name, Modifier.weight(1f), fontSize = 14.sp, fontWeight = if (on) FontWeight.SemiBold else FontWeight.Normal)
                        // Not applied yet.
                        if (d != null && d.edited) Box(Modifier.size(7.dp).background(colors.primary, CircleShape))
                        if (st.folder != null && st.folder?.files?.firstOrNull { it.path == r.path }?.used == false) Dim("unused", size = 11)
                    }
                }
            }
        }
        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedTextField(
                st.newName, { st.newName = it }, Modifier.weight(1f), singleLine = true,
                placeholder = { Text("holidays/en-US.rc", fontSize = 12.sp, maxLines = 1) },
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
            )
            OutlinedButton(enabled = !st.busy && st.newName.isNotBlank(), onClick = { store.go { st.addFile() } }) { Text("Add") }
        }
        if (store.files != null) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                OutlinedButton(enabled = !st.busy, onClick = { store.go { st.upload(files?.pickMany() ?: emptyList()) } }) { Text("Add files") }
                OutlinedButton(enabled = !st.busy, onClick = { store.go { st.upload(files?.pickFolder() ?: emptyList()) } }) { Text("Import folder") }
            }
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(checked = st.replace, onCheckedChange = { st.replace = it })
                Text("Replace what is here (needs a taskrc)", fontSize = 13.sp)
            }
        }

        HolidaysSection(store, st)

        // The file shown.
        Text(st.current, fontSize = 14.sp, fontFamily = FontFamily.Monospace, fontWeight = FontWeight.SemiBold)
        st.info?.takeIf { it.includes.isNotEmpty() }?.let { i ->
            Dim("Includes " + i.includes.joinToString(", ") { it.target + if (it.found == null) " (not found)" else "" } + ".", size = 12)
        }
        for (p in st.problems) Text("Line ${p.line}: ${p.message}", color = extras.warn, fontSize = 13.sp)

        if (st.isRoot) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Dim("Colour theme")
                Choice("choose…", THEMES.map { it.label }, hints = THEMES.map { it.note }) { st.pickTheme(THEMES[it].id) }
            }
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                Switch(checked = !st.coloursOff, onCheckedChange = { st.text = withColour(st.text, it) })
                Text("Colours", fontSize = 14.sp)
            }
            Dim("A theme replaces the colour lines below; Apply to keep it. The Meghnaad theme is what applies when there are none.", size = 12)

            if (store.files != null) {
                OutlinedButton(onClick = { store.go { files?.pick()?.text?.let { st.text = it; st.result = null } } }) { Text("Choose a file") }
            }
            Dim("Or import from a link")
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    st.link, { st.link = it }, Modifier.weight(1f), singleLine = true,
                    placeholder = { Text("https://raw.githubusercontent.com/you/dotfiles/main/.taskrc", fontSize = 12.sp, maxLines = 1) },
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                )
                OutlinedButton(enabled = !st.busy && !st.loading && st.link.isNotBlank(), onClick = { store.go { st.fromLink() } }) { Text("Import") }
            }
            Dim("Fetched and saved straight away (Restore previous undoes it). GitHub and GitLab file pages work too. A public link; keep it free of passwords. Files it includes are not fetched: add them above.", size = 12)
        }

        OutlinedTextField(
            st.text, { st.text = it }, Modifier.fillMaxWidth(), enabled = !st.loading && st.draft != null, minLines = 12,
            textStyle = androidx.compose.ui.text.TextStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp),
            placeholder = { Text(if (st.loading) "Loading…" else if (st.isRoot) "Nothing saved yet. Paste your taskrc here." else "An empty file.", fontSize = 13.sp) },
            keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
        )

        if (!st.isRoot && st.others.isNotEmpty()) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Dim("Include this file in")
                val at = st.others.indexOf(st.includeIn).takeIf { it >= 0 } ?: 0
                Choice(st.others[at], st.others) { st.includeIn = st.others[it] }
                OutlinedButton(onClick = { store.go { st.include() } }) { Text("Include") }
            }
            Dim("Adds ${includeLine(st.others.getOrElse(st.others.indexOf(st.includeIn).coerceAtLeast(0)) { RC_ROOT }, st.current)} to that file; Apply keeps it.", size = 12)
        }

        st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        st.note?.let { Dim(it, size = 13) }
        if (st.skipped.isNotEmpty()) Text("Left out: ${st.skipped.joinToString(", ")}", color = extras.warn, fontSize = 12.sp, fontFamily = FontFamily.Monospace)
        st.result?.let { r ->
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
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
        Row(Modifier.fillMaxWidth().padding(bottom = 24.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp, Alignment.End)) {
            if (store.config?.hasPrevious == true) {
                TextButton(onClick = { store.go { st.restore() } }, enabled = !st.busy) {
                    LucideIcon("rotate-ccw", size = 14.dp)
                    Text("  " + if (st.confirmRestore) "Tap again to restore" else "Restore previous", fontSize = 13.sp)
                }
            }
            if (!st.isRoot) {
                TextButton(onClick = { store.go { st.removeFile() } }, enabled = !st.busy) {
                    LucideIcon("trash-2", size = 14.dp)
                    Text("  " + if (st.confirmDelete) "Tap again to delete" else "Delete file", fontSize = 13.sp, color = colors.error)
                }
            }
            if (st.made != null) {
                TextButton(onClick = { store.go { st.updateHolidays() } }, enabled = !st.busy && !st.hLoading) {
                    LucideIcon("calendar-days", size = 14.dp)
                    Text("  Update holidays", fontSize = 13.sp)
                }
            }
            if (st.fileEdited) TextButton(onClick = { st.draft?.let { it.text = it.saved } }) { Text("Revert edits") }
            Button(onClick = { store.go { st.save() } }, enabled = !st.busy && !st.loading && st.fileEdited) { Text("Apply") }
        }
    }
}

/** Add holidays: a country and the years, what Holidata has for them, and the file that makes. */
@Composable
private fun HolidaysSection(store: Store, st: TaskrcState) {
    val colors = MaterialTheme.colorScheme
    Row(
        Modifier.fillMaxWidth().clickable { st.holidaysOpen = !st.holidaysOpen }.padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        LucideIcon("calendar-days", size = 14.dp)
        Text("Add holidays", fontSize = 14.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
        LucideIcon(if (st.holidaysOpen) "chevron-down" else "chevron-right", size = 14.dp)
    }
    if (!st.holidaysOpen) return
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Dim(
            "Public holidays for a country, from Holidata (holidata.net), the data behind Taskwarrior's own holiday files. " +
                "They are saved as a file here and shown in the calendar.",
            size = 12,
        )
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dim("Country")
            val names = st.locales.map { it.label }
            Choice(st.locales.firstOrNull { it.locale == st.hLocale }?.label ?: "choose…", names) {
                st.hLocale = st.locales[it].locale
                st.holidaysAsked()
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Dim("From")
            OutlinedTextField(
                st.hFirst, { st.hFirst = it; st.holidaysAsked() }, Modifier.width(96.dp), singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            )
            Dim("to")
            OutlinedTextField(
                st.hLast, { st.hLast = it; st.holidaysAsked() }, Modifier.width(96.dp), singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            )
        }
        OutlinedButton(enabled = st.hLocale.isNotEmpty() && !st.hLoading && !st.busy, onClick = { store.go { st.findHolidays() } }) {
            Text(if (st.hLoading) "Fetching…" else "Find holidays")
        }
        st.hError?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        val built = st.hBuilt
        val fetched = st.hFetched
        if (built != null && fetched != null) {
            Text(
                "${built.count} holiday${if (built.count == 1) "" else "s"}" +
                    (if (fetched.missing.isNotEmpty()) " (Holidata has none for ${fetched.missing.joinToString(", ")})" else "") + ".",
                fontSize = 13.sp,
            )
            if (built.regions.isNotEmpty()) {
                Dim("Also the holidays of", size = 12)
                for (r in built.regions) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Checkbox(checked = r in st.hChosen, onCheckedChange = { st.toggleRegion(r, it) })
                        Text(r, fontSize = 13.sp)
                    }
                }
            }
            if (!st.hIncluded) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = st.hInclude, onCheckedChange = { st.hInclude = it })
                    Text("Include it in taskrc", fontSize = 13.sp)
                }
            }
            if (!st.calendarOn) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Checkbox(checked = st.hCalendar, onCheckedChange = { st.hCalendar = it })
                    Text("Show holidays in the calendar (calendar.holidays=sparse)", fontSize = 13.sp)
                }
            }
            Button(enabled = !st.busy && built.count > 0, onClick = { store.go { st.addHolidays() } }) { Text("Save ${st.hPath}") }
            Dim("Replaces that file if it is there; Restore previous undoes it.", size = 12)
        }
    }
}
