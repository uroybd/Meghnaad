package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
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
            OutlinedTextField(
                st.text, { st.text = it; st.done = false; st.error = null }, Modifier.fillMaxWidth().heightIn(min = 220.dp, max = 420.dp),
                textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp, lineHeight = 17.sp),
                isError = st.result is HookCheck.Bad,
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
            )
            when (val r = st.result) {
                is HookCheck.Bad -> Text("${if (r.line != null) "Line ${r.line}: " else ""}${r.message}", color = colors.error, fontSize = 13.sp)
                is HookCheck.Ok -> Text(if (r.defined.isNotEmpty()) "Defines ${r.defined.joinToString(", ")}." else "Loads, but defines none of the four hooks.", color = extras.ok, fontSize = 13.sp)
                null -> if (st.blank) Dim("No hooks. Start from an example, or write your own.", size = 13)
            }
            st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
                Button(onClick = { store.go { st.save() } }, enabled = st.canSave) { Text(if (st.busy) "Saving…" else if (st.blank && st.edited) "Remove the hooks" else "Save") }
                TextButton(onClick = { st.text = st.saved }, enabled = st.edited && !st.busy) { Text("Discard") }
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
            Dim("What a hook gets: a task (task, old, new) with description, project, priority, status, uuid, tags as a set (task.tags:has(\"work\"), :add, :remove), dates as values with arithmetic (task.due = tomorrow()), helpers (task:in_project(\"Work\"), :is_overdue()), say(…) and warn(…) to print, refuse(\"why\") to stop the command, and http.get/post/… to hosts declared with allow_hosts{\"acme.example.com\"}.", size = 12)
            Row(Modifier.padding(bottom = 24.dp)) {}
        }
    }
}
