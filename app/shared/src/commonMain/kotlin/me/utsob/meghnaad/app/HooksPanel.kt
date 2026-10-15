package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.isCtrlPressed
import androidx.compose.ui.input.key.isMetaPressed
import androidx.compose.ui.input.key.isShiftPressed
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.onPreviewKeyEvent
import androidx.compose.ui.input.key.type
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.withStyle
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** What the engine says about a script: it loads and defines these hooks, or it is wrong at this line. */
sealed interface HookCheck {
    data class Ok(val defined: List<String>) : HookCheck
    data class Bad(val line: Int?, val message: String) : HookCheck
}

private val SECRET_NAME = Regex("^[A-Za-z][A-Za-z0-9_]{0,63}$")

/** The hooks tab's edits and secrets: kept while another tab is shown (HooksPanel.svelte). The script is read when the tab is first shown. */
class HooksState(private val store: Store) {
    var saved by mutableStateOf("")
    var text by mutableStateOf("")
    var loading by mutableStateOf(false)
    var loaded by mutableStateOf(false)
    var loadFailed by mutableStateOf<String?>(null)
    var result by mutableStateOf<HookCheck?>(null)
    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var done by mutableStateOf(false)

    // Secrets: kept from the app, sealed in the bucket. Only their names are ever shown.
    val secrets = mutableStateListOf<String>()
    var secretName by mutableStateOf("")
    var secretValue by mutableStateOf("")
    var secretBusy by mutableStateOf(false)
    var secretError by mutableStateOf<String?>(null)
    val secretNameOk get() = SECRET_NAME.matches(secretName)

    val edited get() = text != saved
    val blank get() = text.isBlank()
    val canSave get() = edited && !busy && (blank || result is HookCheck.Ok)

    suspend fun load() {
        if (loaded || loading) return
        loading = true
        try {
            saved = engineJson.parseToJsonElement(store.request("GET", "/api/hooks")).jsonObject["source"]?.jsonPrimitive?.content ?: ""
            text = saved
            loaded = true
            loadSecrets()
        } catch (e: Exception) {
            loadFailed = e.message ?: e.toString()
        } finally {
            loading = false
        }
    }

    private suspend fun loadSecrets() {
        try {
            val names = engineJson.parseToJsonElement(store.request("GET", "/api/secrets")).jsonObject["names"]?.jsonArray ?: return
            secrets.clear()
            secrets.addAll(names.map { it.jsonPrimitive.content })
        } catch (_: Exception) {
            // The editor works without them; setting one says why it could not.
        }
    }

    /** Ask the engine (its Lua is the one that will run the script) whether this one is all right. */
    suspend fun check(asked: String) {
        try {
            val o = engineJson.parseToJsonElement(store.request("POST", "/api/hooks/check", asked)).jsonObject
            if (asked != text) return
            result = if (o["ok"]?.jsonPrimitive?.boolean == true) {
                HookCheck.Ok(o["defined"]?.jsonArray?.map { it.jsonPrimitive.content } ?: emptyList())
            } else {
                HookCheck.Bad(o["line"]?.jsonPrimitive?.let { p -> if (p.content == "null") null else p.int }, o["message"]?.jsonPrimitive?.content ?: "")
            }
        } catch (_: Exception) {
            // Offline: nothing to say about the script; saving says why it could not.
        }
    }

    suspend fun save() {
        if (!canSave) return
        busy = true
        error = null
        try {
            val names = engineJson.parseToJsonElement(store.request("PUT", "/api/hooks", text)).jsonObject["defined"]?.jsonArray?.map { it.jsonPrimitive.content } ?: emptyList()
            saved = text
            done = true
            store.notify(if (names.isNotEmpty()) "Hooks saved: ${names.joinToString(", ")}." else if (blank) "Hooks removed." else "Saved; no hook is defined.")
        } catch (e: Exception) {
            error = e.message ?: "The hooks could not be saved."
        } finally {
            busy = false
        }
    }

    suspend fun keepSecret() {
        if (!secretNameOk || secretValue.isEmpty() || secretBusy) return
        secretBusy = true
        secretError = null
        try {
            store.request("PUT", "/api/secrets/$secretName", secretValue)
            store.notify("Secret $secretName saved. Read it in a hook with secret(\"$secretName\").")
            secretName = ""
            secretValue = ""
            loadSecrets()
        } catch (e: Exception) {
            secretError = e.message ?: "The secret could not be saved."
        } finally {
            secretBusy = false
        }
    }

    suspend fun removeSecret(name: String) {
        secretError = null
        try {
            store.request("DELETE", "/api/secrets/$name")
            loadSecrets()
        } catch (e: Exception) {
            secretError = e.message ?: "The secret could not be removed."
        }
    }
}

@Composable
fun HooksPanel(store: Store, st: HooksState) {
    val colors = MaterialTheme.colorScheme
    LaunchedEffect(Unit) { st.load() }
    // A moment after typing, ask whether the script is all right.
    LaunchedEffect(st.text, st.loaded) {
        if (st.loaded && st.text.isNotBlank()) {
            delay(400)
            st.check(st.text)
        } else st.result = null
    }

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("Hooks", fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
        Dim(
            "A hook is a Lua function that runs at the same four moments Taskwarrior runs its scripts: on_launch before a command, on_add for each new task, " +
                "on_modify for each task a command changes, and on_exit after it. They run on this phone, so a script can hold secrets that never leave it. " +
                "Your desktop's hooks (~/.task/hooks) are separate. rc.hooks:off turns them off for a command.",
        )
        st.loadFailed?.let { Text("The hooks could not be read: $it", color = colors.error, fontSize = 13.sp) }
        if (st.loading) Dim("Loading…", size = 14)
        if (st.loaded) {
            // The text with its cursor, so Tab can indent where it is; what is typed here and what Discard or an example puts there are the same text.
            var tfv by remember { mutableStateOf(TextFieldValue(st.text, TextRange(st.text.length))) }
            if (tfv.text != st.text) tfv = TextFieldValue(st.text, TextRange(st.text.length))
            val x = extras
            val palette = remember(x, colors) { LuaPalette(colors.primary, x.ok, x.warn, colors.onSurfaceVariant, colors.error) }
            val problemLine = (st.result as? HookCheck.Bad)?.line
            val colouring = remember(palette, problemLine) { LuaTransformation(palette, problemLine) }
            OutlinedTextField(
                tfv,
                { v -> tfv = v; if (v.text != st.text) { st.text = v.text; st.done = false; st.error = null } },
                Modifier.fillMaxWidth().heightIn(min = 220.dp, max = 420.dp).onPreviewKeyEvent { e ->
                    if (e.type != KeyEventType.KeyDown) return@onPreviewKeyEvent false
                    when {
                        // Ctrl+S or Cmd+S saves, as in the web's editor.
                        e.key == Key.S && (e.isCtrlPressed || e.isMetaPressed) -> {
                            if (st.canSave) store.go { st.save() }
                            true
                        }
                        // Tab indents (two spaces) instead of leaving the field.
                        e.key == Key.Tab && !e.isShiftPressed -> {
                            val sel = tfv.selection
                            val text = tfv.text.replaceRange(sel.min, sel.max, "  ")
                            tfv = TextFieldValue(text, TextRange(sel.min + 2))
                            st.text = text; st.done = false; st.error = null
                            true
                        }
                        else -> false
                    }
                },
                textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp, lineHeight = 17.sp),
                isError = st.result is HookCheck.Bad,
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
                visualTransformation = colouring,
            )
            when (val r = st.result) {
                is HookCheck.Bad -> Text("${if (r.line != null) "Line ${r.line}: " else ""}${r.message}", color = colors.error, fontSize = 13.sp)
                is HookCheck.Ok -> Text(if (r.defined.isNotEmpty()) "Defines ${r.defined.joinToString(", ")}." else "Loads, but defines none of the four hooks.", color = extras.ok, fontSize = 13.sp)
                null -> if (st.blank) Dim("No hooks. Start from an example, or write your own.", size = 13)
            }
            st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Button(onClick = { store.go { st.save() } }, enabled = st.canSave) { Text(if (st.busy) "Saving…" else if (st.blank && st.edited) "Remove the hooks" else "Save") }
                TextButton(onClick = { st.text = st.saved }, enabled = st.edited && !st.busy) { Text("Discard changes") }
                if (st.blank) TextButton(onClick = { st.text = HOOKS_EXAMPLE }) { Text("Insert an example") }
                if (st.done && !st.edited) Text("Saved.", color = extras.ok, fontSize = 13.sp)
            }

            Text("Secrets", fontSize = 16.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 12.dp))
            Dim(
                "A secret is a value a script reads with secret(\"name\") instead of having it in the script: a token for http.post(…), say. It is kept in your " +
                    "bucket sealed with the same key as your tasks, can be replaced but never shown again, and is hidden from anything a hook prints or returns.",
            )
            for (name in st.secrets.toList()) {
                Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
                    Text(name, fontFamily = FontFamily.Monospace, fontSize = 14.sp)
                    TextButton(onClick = { store.go { st.removeSecret(name) } }) { Text("Remove") }
                }
            }
            OutlinedTextField(
                st.secretName, { st.secretName = it }, Modifier.fillMaxWidth(), singleLine = true,
                placeholder = { Text("name (jira)", fontSize = 13.sp) },
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
            )
            SecretField(st.secretValue, { st.secretValue = it }, "value", Modifier.fillMaxWidth())
            OutlinedButton(onClick = { store.go { st.keepSecret() } }, enabled = st.secretNameOk && st.secretValue.isNotEmpty() && !st.secretBusy) {
                Text(if (st.secretName in st.secrets) "Replace" else "Keep")
            }
            st.secretError?.let { Text(it, color = colors.error, fontSize = 13.sp) }
            var help by remember { mutableStateOf(false) }
            Row(Modifier.fillMaxWidth().clickable { help = !help }.padding(vertical = 8.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                LucideIcon(if (help) "chevron-down" else "chevron-right", size = 14.dp)
                Text("What a hook gets", fontWeight = FontWeight.SemiBold, fontSize = 14.sp)
            }
            if (help) for (item in HOOK_HELP) {
                Text(richText(item, SpanStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp, background = colors.surfaceVariant)), fontSize = 13.sp, lineHeight = 19.sp, color = colors.onSurfaceVariant)
            }
            Row(Modifier.padding(bottom = 24.dp)) {}
        }
    }
}

/** Text with `code` in monospace and **bold** in bold: what the help below is written in. */
private fun richText(text: String, code: SpanStyle): AnnotatedString = buildAnnotatedString {
    var i = 0
    while (i < text.length) {
        when {
            text[i] == '`' -> {
                val e = text.indexOf('`', i + 1).let { if (it < 0) text.length else it }
                withStyle(code) { append(text.substring(i + 1, e)) }
                i = minOf(e + 1, text.length)
            }
            text.startsWith("**", i) -> {
                val e = text.indexOf("**", i + 2).let { if (it < 0) text.length else it }
                withStyle(SpanStyle(fontWeight = FontWeight.Bold)) { append(text.substring(i + 2, e)) }
                i = minOf(e + 2, text.length)
            }
            else -> {
                append(text[i])
                i++
            }
        }
    }
}

/** What a hook gets (the web's "What a hook gets", HooksPanel.svelte). */
private val HOOK_HELP = listOf(
    """**A task** (`task`, `old`, `new`): `description`, `project`, `priority`, `status`, `uuid`, `blocked`, `blocking`, `recur`, `annotations` (a list of `{entry, text}`), and your UDAs as plain fields (`task.estimate = "big"`; set one to `nil` to remove it). `status`, `uuid`, annotations and recurrence can be read, not changed. Change the task you are given, or return one.""",
    """**Tags** are a set: `task.tags:has("work")`, `:add("a", "b")`, `:remove("x")`, `:has_any(…)`, `:has_all(…)`, `:list()`, `#task.tags`, `for t in task.tags:each() do … end`. So is `task.depends` (task ids as text).""",
    """**Dates** (`due`, `wait`, `scheduled`, `start`, `entry`, `modified`, `finished` for `end`, `expires` for `until`) are values with arithmetic: `task.due = tomorrow()`, `task.wait = task.due - days(2)`, `date("friday")`, `date("2026-12-25")`, `date("eom")`, `d1 < d2`, `d2 - d1` (a duration), `d.year .month .day .hour .min .sec .weekday`, `d:format("Y-M-D")`, `d:start_of("week")`, `now()`, `today()`. **Durations**: `days(n)`, `hours(n)`, `minutes(n)`, `weeks(n)`, `duration("3d")`, `d:in_days()`.""",
    """**The network**, to hosts the script declares at the top (`allow_hosts{"acme.example.com"}`), over https: `local res, err = http.post(url, { headers = { Authorization = "Bearer " .. secret("jira") }, json = { … } })` (also `get`, `put`, `patch`, `delete`). `res` is `{status, ok, headers, body, json}`, or `nil` and a reason if the network failed, so a script can carry on. A few requests a call, 10 seconds at most each.""",
    """**Helpers**: `task:in_project("Work")` (also true for `Work.Mail`), `:is_pending()`, `:is_waiting()`, `:is_overdue()`. `say(…)` and `warn(…)` print under the result; `refuse("why")` (or any error) stops the command. There are no files, and a script that runs too long is stopped.""",
)
