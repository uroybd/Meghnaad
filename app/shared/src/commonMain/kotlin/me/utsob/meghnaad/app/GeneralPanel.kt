package me.utsob.meghnaad.app

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch
import kotlinx.serialization.json.JsonObject

/**
 * The page the app opens on, kept in `settings.json` (GeneralPanel.svelte): any report or board, `next` when nothing is
 * chosen. And `settings.json` itself, whole, to read, edit or paste over (spec 0009).
 */
class GeneralState(private val store: Store) {
    // Under This device (spec 0027): the home page this device uses, and whether it is its own.
    private val device get() = store.settingsDevice
    val own get() = store.deviceRaw?.containsKey("homepage") == true
    val everyones get() = store.appSettings?.homepage
    private val saved get() = if (device) store.homepage else everyones

    /** The choice: a report, a board, or nothing (the default, `next`). */
    var choice by mutableStateOf<Homepage?>(saved)

    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var done by mutableStateOf(false)

    /** What the choice was last made from: when the saved one changes and the choice was not touched, it follows. */
    private var base = saved

    fun follow() {
        val now = saved
        if (now == base) return
        val untouched = choice == base
        base = now
        if (untouched) choice = now
    }

    val edited get() = choice != saved

    /** The file as saved. */
    val file: String get() = serializeSettings((if (device) store.deviceRaw else store.appSettings?.raw) ?: JsonObject(emptyMap()))

    /** The file's name: `settings.json`, or this device's under This device. */
    val fileName: String get() = if (device) "devices/${store.deviceId ?: ""}.json" else "settings.json"

    /** The file in the box. */
    var text by mutableStateOf(file)
    private var fileBase = text
    val textEdited get() = text != file
    val lint: Lint get() = lintSettings(text) { resolveKeymap(it).problems }
    var fileBusy by mutableStateOf(false)
    var fileError by mutableStateOf<String?>(null)
    var fileDone by mutableStateOf(false)

    /** Saved somewhere else: the box follows, unless it has edits. */
    fun followFile() {
        val now = file
        if (now == fileBase) return
        val untouched = text == fileBase
        fileBase = now
        if (untouched) text = now
    }

    suspend fun applyFile() {
        fileBusy = true
        fileError = null
        fileDone = false
        try {
            // Saved as written: what is pasted is the file.
            if (device) store.saveDeviceText(text) else store.saveSettingsFile(text)
            fileBase = file
            text = fileBase
            fileDone = true
        } catch (e: Exception) {
            fileError = e.message ?: e.toString()
        } finally {
            fileBusy = false
        }
    }

    /** The saved home page names a report or board that is not there (any more), so the app opens on `next`. */
    val missing: Boolean
        get() = saved?.let { h -> if (h.report != null) store.reports.none { it.name == h.report } else store.boards.none { it.id == h.board } } ?: false

    suspend fun apply() {
        busy = true
        error = null
        done = false
        try {
            if (device) {
                val id = ensureDeviceId(store) ?: return
                store.saveDevice(withDeviceHomepage(store.deviceRaw ?: JsonObject(emptyMap()), choice), id)
            } else {
                store.saveHomepage(choice)
            }
            // What is saved now is what the choice was made from.
            base = saved
            done = true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }

    /** Under This device: the home page everyone's again. */
    suspend fun putBack() {
        busy = true
        error = null
        try {
            store.saveDevice(withDeviceHomepage(store.deviceRaw ?: JsonObject(emptyMap()), null, own = false))
            base = saved
            choice = saved
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }

    /** This device's ID as typed, to change (which renames its file, or, for an ID another device has, uses that one's). */
    var idText by mutableStateOf(store.deviceId ?: "")
    private var idBase = store.deviceId
    var idError by mutableStateOf<String?>(null)

    fun followId() {
        if (store.deviceId == idBase) return
        idBase = store.deviceId
        idText = store.deviceId ?: ""
    }

    suspend fun rename() {
        val next = idText.trim()
        idError = null
        try {
            if (next in store.listDevices()) store.useDevice(next) else store.saveDevice(store.deviceRaw ?: JsonObject(emptyMap()), next)
        } catch (e: Exception) {
            idError = e.message ?: e.toString()
        }
    }

    suspend fun forget() {
        val id = store.deviceId ?: return
        try {
            store.deleteDevice(id)
        } catch (e: Exception) {
            idError = e.message ?: e.toString()
        }
    }
}

@Composable
private fun Choice(label: String, selected: Boolean, onClick: () -> Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min = 44.dp).clickable(onClick = onClick), verticalAlignment = Alignment.CenterVertically) {
        RadioButton(selected, onClick = null, modifier = Modifier.padding(horizontal = 8.dp))
        Text(label, fontSize = 15.sp, fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal)
    }
}

@Composable
fun GeneralPanel(store: Store, state: GeneralState) {
    state.follow()
    state.followFile()
    val scope = rememberCoroutineScope()
    state.followId()
    val device = store.settingsDevice
    if (device) DeviceIdSection(store, state)
    Column(
        Modifier.padding(vertical = 8.dp).then(if (device && state.own) Modifier.ownField() else Modifier),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text("Home page", fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
            if (device && state.own) Text("  this device's", color = MaterialTheme.colorScheme.primary, fontSize = 12.sp)
        }
        Dim(
            "The page the app opens on: any report, including your own, or any of your boards. Left alone it is next. " +
                "It is kept in this app's own settings.json, not in your taskrc.",
        )
        Choice("next (the default)", state.choice == null) { state.choice = null; state.done = false }
        store.offeredReports.filter { it.name != "next" }.forEach { r ->
            Choice(r.name, state.choice?.report == r.name) { state.choice = Homepage(report = r.name); state.done = false }
        }
        if (store.boards.isNotEmpty()) {
            Dim("Boards", Modifier.padding(top = 6.dp))
            store.boards.forEach { b ->
                Choice(b.name, state.choice?.board == b.id) { state.choice = Homepage(board = b.id); state.done = false }
            }
        }
        if (device && state.own) {
            TextButton({ scope.launch { state.putBack() } }, enabled = !state.busy) {
                Text("Use everyone's (${state.everyones?.let { it.report ?: it.board } ?: "next"})")
            }
        }
        if (!device && store.deviceRaw?.containsKey("homepage") == true) {
            val mine = parseHomepage(store.deviceRaw?.get("homepage"))
            Text(
                "This device uses its own: ${mine?.let { it.report ?: it.board } ?: "next"} (under This device)",
                color = MaterialTheme.colorScheme.primary,
                fontSize = 13.sp,
                modifier = Modifier.clickable { store.settingsDevice = true },
            )
        }
        if (state.missing) Text("The saved home page is not there any more, so the app opens on next.", color = MaterialTheme.colorScheme.error, fontSize = 13.sp)
        state.error?.let { Text(it, color = MaterialTheme.colorScheme.error, fontSize = 13.sp) }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            Button({ scope.launch { state.apply() } }, enabled = !state.busy && state.edited) { Text("Apply") }
            if (state.done) Dim("Saved")
        }
    }
}

/** A field this device has its own value for (spec 0027): framed, name and value together. */
@Composable
fun Modifier.ownField(): Modifier {
    val c = MaterialTheme.colorScheme.primary
    return this.border(1.dp, c, RoundedCornerShape(8.dp)).background(c.copy(alpha = 0.07f), RoundedCornerShape(8.dp)).padding(10.dp)
}

/** Under This device: its ID, to change, and its settings to delete; or a line saying it has none yet. */
@Composable
private fun DeviceIdSection(store: Store, state: GeneralState) {
    val scope = rememberCoroutineScope()
    Column(Modifier.padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (store.deviceId == null) {
            Dim("This device has no settings of its own yet: it uses everyone's. Saving something here makes them, and asks for a name.")
            return@Column
        }
        val problem = deviceIdProblem(state.idText.trim())
        OutlinedTextField(
            value = state.idText,
            onValueChange = { state.idText = it },
            label = { Text("Device ID") },
            singleLine = true,
            textStyle = MaterialTheme.typography.bodyMedium.copy(fontFamily = FontFamily.Monospace),
            isError = problem != null,
            modifier = Modifier.fillMaxWidth(),
        )
        if (problem != null && state.idText.isNotBlank()) Text(problem, color = MaterialTheme.colorScheme.error, fontSize = 13.sp)
        state.idError?.let { Text(it, color = MaterialTheme.colorScheme.error, fontSize = 13.sp) }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            OutlinedButton({ scope.launch { state.rename() } }, enabled = problem == null && state.idText.trim() != store.deviceId) { Text("Change") }
            TextButton({ scope.launch { state.forget() } }) { Text("Delete this device's settings") }
        }
    }
}

/** The whole settings.json, on the General tab under the update checker. */
@Composable
fun SettingsFilePanel(store: Store, state: GeneralState) {
    val scope = rememberCoroutineScope()
    val clipboard = LocalClipboardManager.current
    Column(Modifier.padding(vertical = 8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        if (store.settingsDevice && store.deviceId == null) return@Column
        Text(state.fileName, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 8.dp))
        if (store.settingsDevice) Dim(
            "This device's own settings, laid over everyone's: its deviceId, and its home page, keys and reminders where it has them.",
        ) else Dim(
            "This app's own settings: the home page, appearance, task commands, boards and keys, and the web app's and the " +
                "terminal app's own. Edit it here or paste a whole file over it; what this app doesn't use is kept as it is.",
        )
        OutlinedTextField(
            value = state.text,
            onValueChange = { state.text = it; state.fileDone = false },
            modifier = Modifier.fillMaxWidth().heightIn(min = 160.dp, max = 480.dp),
            textStyle = MaterialTheme.typography.bodySmall.copy(fontFamily = FontFamily.Monospace),
            isError = state.lint.errors.isNotEmpty(),
        )
        val lint = state.lint
        lint.errors.forEach { Text(it, color = MaterialTheme.colorScheme.error, fontSize = 13.sp) }
        lint.warnings.forEach { Dim("$it (kept)") }
        state.fileError?.let { Text(it, color = MaterialTheme.colorScheme.error, fontSize = 13.sp) }
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalAlignment = Alignment.CenterVertically) {
            Button({ scope.launch { state.applyFile() } }, enabled = !state.fileBusy && state.textEdited && lint.errors.isEmpty()) { Text("Apply") }
            OutlinedButton({ clipboard.setText(AnnotatedString(state.text)); store.notify("${state.fileName} copied") }) { Text("Copy") }
            TextButton({ state.text = state.file }, enabled = !state.fileBusy && state.textEdited) { Text("Reset") }
            if (state.fileDone) Dim("Saved")
        }
    }
}
