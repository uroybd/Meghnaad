package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.launch
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

// Looking for a newer version of the app. The app is not in a store: each release publishes its APK on GitHub (the Release workflow), so
// the latest release says what the newest version is and where its APK is. Nothing is installed here: the APK is handed to the
// phone's browser, which downloads it and offers to install it over this app (an update keeps the tasks and the connection when it
// is signed with the same key, see docs/android.md).

/** The repository the app's releases are published to (`owner/name`), set by the build that made the app; empty in a build that was not made by a release, which does not check. */
var UPDATE_REPO: String = ""

/** The newest release: its version, its APK (none when it has not been attached yet) and its page. */
class Release(val version: String, val apkUrl: String?, val pageUrl: String)

/** The release a GitHub `releases/latest` answer describes, or null when it is not one. Only `https` addresses are kept. */
fun parseRelease(text: String): Release? {
    val o = runCatching { Json.parseToJsonElement(text).jsonObject }.getOrNull() ?: return null
    val tag = o["tag_name"]?.jsonPrimitive?.contentOrNull?.trim()?.removePrefix("v") ?: return null
    if (tag.isEmpty()) return null
    val page = o["html_url"]?.jsonPrimitive?.contentOrNull?.takeIf { it.startsWith("https://") } ?: return null
    val apks = (o["assets"] as? JsonArray).orEmpty().mapNotNull { a ->
        val asset = a as? JsonObject ?: return@mapNotNull null
        val name = asset["name"]?.jsonPrimitive?.contentOrNull ?: return@mapNotNull null
        val url = asset["browser_download_url"]?.jsonPrimitive?.contentOrNull?.takeIf { it.startsWith("https://") } ?: return@mapNotNull null
        if (name.endsWith(".apk")) name to url else null
    }
    // A debug-signed APK is the fallback of a build with no key; a signed one is the one to update with.
    val apk = (apks.firstOrNull { "-debug" !in it.first } ?: apks.firstOrNull())?.second
    return Release(tag, apk, page)
}

private val VERSION = Regex("^v?(\\d+)\\.(\\d+)\\.(\\d+)(.*)$")

/** Whether [latest] is a later version than [current] (`0.6.36` after `0.6.35`; a release after its own pre-release). */
fun isNewer(latest: String, current: String): Boolean {
    val a = VERSION.matchEntire(latest.trim())?.groupValues ?: return false
    val b = VERSION.matchEntire(current.trim())?.groupValues ?: return false
    for (i in 1..3) {
        val x = a[i].toLong()
        val y = b[i].toLong()
        if (x != y) return x > y
    }
    return a[4].isEmpty() && b[4].isNotEmpty()
}

sealed interface UpdateStatus {
    data object Idle : UpdateStatus

    data object Checking : UpdateStatus

    data class UpToDate(val version: String) : UpdateStatus

    data class Available(val release: Release) : UpdateStatus

    data class Failed(val message: String) : UpdateStatus
}

private const val CHECKED_KEY = "update-checked"
private const val TOLD_KEY = "update-told"
private const val DAY = 24 * 3600.0

/** What GitHub's answer for the latest release says to someone running [current]. */
fun judge(answer: HttpAnswer, current: String): UpdateStatus = when {
    answer.status == 404 -> UpdateStatus.Failed("No release has been published yet.")
    answer.status != 200 -> UpdateStatus.Failed("GitHub answered ${answer.status}.")
    else -> parseRelease(answer.body)?.let { if (isNewer(it.version, current)) UpdateStatus.Available(it) else UpdateStatus.UpToDate(current) }
        ?: UpdateStatus.Failed("The answer was not a release.")
}

/** What the app knows of newer versions: asked for when the app opens (at most once a day) and when the person asks. */
class UpdateState(private val prefs: Prefs, private val clock: Clock, private val get: suspend (String, Int) -> HttpAnswer = ::httpGet) {
    var status by mutableStateOf<UpdateStatus>(UpdateStatus.Idle)
        private set

    /** Whether this build can check at all (a build that was not made by a release does not know where the releases are). */
    val canCheck get() = UPDATE_REPO.isNotEmpty() && APP_VERSION.isNotEmpty()

    /** Ask for the latest release. A failure is kept as the status; it never throws. */
    suspend fun check() {
        if (!canCheck) return
        status = UpdateStatus.Checking
        status = try {
            val answer = get("https://api.github.com/repos/$UPDATE_REPO/releases/latest", 500_000)
            prefs.put(CHECKED_KEY, clock.nowSeconds().toLong().toString())
            judge(answer, APP_VERSION)
        } catch (e: Exception) {
            UpdateStatus.Failed(e.message ?: e.toString())
        }
    }

    /**
     * The check the app makes when it opens: once a day, and a version that was already told about is not told again.
     * Returns the release to tell the person about, if there is a new one.
     */
    suspend fun checkWhenDue(): Release? {
        if (!canCheck) return null
        val last = prefs.get(CHECKED_KEY)?.toLongOrNull() ?: 0L
        if (clock.nowSeconds() - last < DAY) return null
        check()
        val found = (status as? UpdateStatus.Available)?.release ?: return null
        if (prefs.get(TOLD_KEY) == found.version) return null
        prefs.put(TOLD_KEY, found.version)
        return found
    }
}

/** Settings: the version, whether a newer one is out, and the way to download it. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun UpdateRow(updates: UpdateState) {
    if (!updates.canCheck) return
    val scope = rememberCoroutineScope()
    val open = LocalUriHandler.current
    val status = updates.status
    Column(Modifier.padding(top = 16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text("Updates", style = MaterialTheme.typography.titleSmall)
        when (status) {
            UpdateStatus.Idle -> Dim("Meghnaad $APP_VERSION. See whether a newer version is out.")
            UpdateStatus.Checking -> Dim("Looking for a newer version…")
            is UpdateStatus.UpToDate -> Dim("Meghnaad ${status.version} is the latest version.")
            is UpdateStatus.Available -> Text(
                "Version ${status.release.version} is out (this is $APP_VERSION).${if (status.release.apkUrl == null) " Its APK is not on the release yet: try again in a few minutes." else ""}",
                color = MaterialTheme.colorScheme.primary,
            )
            is UpdateStatus.Failed -> Text(status.message, color = MaterialTheme.colorScheme.error)
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            val available = (status as? UpdateStatus.Available)?.release
            if (available?.apkUrl != null) Button({ open.openUri(available.apkUrl) }) { Text("Download the APK") }
            OutlinedButton({ scope.launch { updates.check() } }, enabled = status != UpdateStatus.Checking) { Text("Check for updates") }
            if (available != null) OutlinedButton({ open.openUri(available.pageUrl) }) { Text("What’s new") }
        }
        if (available(status)) Dim("The phone’s browser downloads it and asks to install it over this app; your tasks and connection stay.", size = 12)
    }
}

private fun available(s: UpdateStatus) = s is UpdateStatus.Available && s.release.apkUrl != null
