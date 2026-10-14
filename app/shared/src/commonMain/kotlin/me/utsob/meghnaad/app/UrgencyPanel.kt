package me.utsob.meghnaad.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** A number as the engine writes it: whole numbers without ".0". */
private fun num(n: Double?) = n?.let { if (it == it.toLong().toDouble()) it.toLong().toString() else it.toString() } ?: ""

/** The urgency tab's edits: kept while another tab is shown (UrgencyPanel.svelte). */
class UrgencyState(private val store: Store) {
    val defaults get() = store.config?.urgencyDefaults ?: emptyMap()
    val savedOverrides get() = store.config?.config?.urgency ?: emptyMap()
    val savedInherit get() = store.config?.urgencyInherit ?: false

    var overrides by mutableStateOf(savedOverrides)
    var inherit by mutableStateOf(savedInherit)
    var busy by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var done by mutableStateOf(false)

    val edited get() = overrides != savedOverrides || inherit != savedInherit
    val blocked get() = store.config?.configError != null
    val keys get() = sortUrgencyKeys((defaults.keys + overrides.keys).toSet())

    fun set(key: String, raw: String) {
        val v = raw.trim().toDoubleOrNull() ?: return
        if (!v.isFinite()) return
        overrides = withUrgencyValue(overrides, defaults, key, v)
        done = false
    }

    fun reset(key: String) {
        overrides = overrides - key
        done = false
    }

    fun revert() {
        overrides = savedOverrides
        inherit = savedInherit
        done = false
    }

    suspend fun apply() {
        busy = true
        error = null
        try {
            val body = buildJsonObject {
                put("urgency", JsonObject(overrides.mapValues { JsonPrimitive(it.value) }))
                put("inherit", inherit)
            }
            store.request("PUT", "/api/config/urgency", body.toString())
            store.loadConfig()
            // Scores, and so the order of tasks, may have changed.
            store.live?.let { store.refresh(it) }
            done = true
        } catch (e: Exception) {
            error = e.message ?: e.toString()
        } finally {
            busy = false
        }
    }
}

@Composable
fun UrgencyPanel(store: Store, st: UrgencyState) {
    val colors = MaterialTheme.colorScheme
    // The saved values changed somewhere else (the taskrc tab, the console): follow them, unless there are edits here.
    var before by remember { mutableStateOf(st.savedOverrides to st.savedInherit) }
    LaunchedEffect(st.savedOverrides, st.savedInherit) {
        val now = st.savedOverrides to st.savedInherit
        val untouched = st.overrides == before.first && st.inherit == before.second
        val changed = now != before
        before = now
        if (changed && untouched) st.revert()
    }
    val defaults = st.defaults

    Column(Modifier.fillMaxWidth().padding(top = 8.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        Text("Urgency", fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
        Dim(
            "A task's urgency is the sum of these terms, as in Taskwarrior's urgency.* settings. Changes are saved with your other imported settings, " +
                "so they appear in the taskrc tab and Restore previous covers them.",
        )
        Dim("• Coefficients add up: a task in Home.Kitchen gets the Home and the Home.Kitchen coefficient.")
        Dim("• A project coefficient applies to that project and its sub-projects, never to a different project that merely starts with the same letters (Work does not reach Workshop).")
        Dim("• A tag coefficient also works for virtual tags such as OVERDUE.")
        if (st.blocked) Text("The saved settings couldn't be read, so urgency can't be saved until that is fixed in the taskrc tab.", color = colors.error, fontSize = 13.sp)

        Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Column(Modifier.weight(1f)) {
                Text("Blocking tasks inherit urgency", fontWeight = FontWeight.Medium)
                Text(INHERIT_KEY, fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = colors.onSurfaceVariant)
                Dim("A task that blocks others is at least as urgent as the most urgent of them, and a little more, so it sorts first. Off in Taskwarrior unless you turn it on.", size = 12)
            }
            Switch(st.inherit, { st.inherit = it; st.done = false })
        }

        for (key in st.keys) {
            val d = describeUrgency(key)
            val overridden = key in st.overrides
            val custom = key !in defaults
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Column(Modifier.weight(1f)) {
                    Text(d.label, fontWeight = if (overridden) FontWeight.SemiBold else FontWeight.Normal)
                    if (d.hint.isNotEmpty()) Dim(d.hint, size = 12)
                    Text(key, fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = colors.onSurfaceVariant)
                    if (!custom) Dim("Default ${num(defaults[key])}", size = 12)
                }
                // Rekeyed when the value changes from outside (a reset), so the field shows it.
                var text by remember(key, st.overrides[key]) { mutableStateOf(num(effectiveUrgency(key, st.overrides, defaults))) }
                OutlinedTextField(
                    text, { text = it; st.set(key, it) }, Modifier.width(92.dp), singleLine = true,
                    textStyle = TextStyle(fontSize = 14.sp),
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                )
                when {
                    custom -> IconAction("x", "Remove ${d.label}", { st.reset(key) }, size = 15.dp)
                    overridden -> IconAction("rotate-ccw", "Back to default for ${d.label}", { st.reset(key) }, size = 15.dp)
                    else -> Row(Modifier.width(40.dp)) {}
                }
            }
        }

        // Adding a setting.
        var addIndex by remember { mutableStateOf(0) }
        var addName by remember { mutableStateOf("") }
        var addValue by remember { mutableStateOf("1") }
        val adding = ADDABLE[addIndex]
        val addKey = if (adding.kind == UrgencyKind.Uda) {
            if (validUda(addName)) buildUrgencyKey(UrgencyKind.Uda, addName) else null
        } else if (validUrgencyName(addName)) buildUrgencyKey(adding.kind, addName) else null
        val value = addValue.trim().toDoubleOrNull()?.takeIf { it.isFinite() }
        // UDA = value only makes sense with a value; UDA (any value) only without one.
        val shapeOk = when (adding.label) {
            "UDA = value" -> '.' in addName
            "UDA (any value)" -> '.' !in addName
            else -> true
        }
        Text("Add a setting", fontWeight = FontWeight.Medium, modifier = Modifier.padding(top = 6.dp))
        Choice(adding.label, ADDABLE.map { it.label }) { addIndex = it }
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
            OutlinedTextField(
                addName, { addName = it }, Modifier.weight(1f), singleLine = true,
                placeholder = { Text(adding.placeholder, fontSize = 13.sp) },
                keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None),
            )
            OutlinedTextField(
                addValue, { addValue = it }, Modifier.width(92.dp), singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
            )
        }
        if (addName.isNotEmpty() && (addKey == null || !shapeOk)) {
            Text(
                when (adding.label) {
                    "UDA = value" -> "Write the UDA, a dot and its value: estimate.huge."
                    "UDA (any value)" -> "Write just the UDA name: estimate."
                    else -> "Names can't contain spaces, = or #."
                },
                color = extras.warn, fontSize = 13.sp,
            )
        }
        OutlinedButton(
            onClick = {
                if (addKey != null && value != null) {
                    st.overrides = withUrgencyValue(st.overrides, defaults, addKey, value)
                    addName = ""
                    st.done = false
                }
            },
            enabled = addKey != null && value != null && shapeOk,
        ) { LucideIcon("plus", size = 14.dp); Text("  Add") }

        st.error?.let { Text(it, color = colors.error, fontSize = 13.sp) }
        if (st.done && !st.edited) Text("Saved. Task scores are updated.", color = extras.ok, fontSize = 13.sp)
        Row(Modifier.fillMaxWidth().padding(bottom = 24.dp), horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End), verticalAlignment = Alignment.CenterVertically) {
            if (st.edited) TextButton(onClick = st::revert) { Text("Revert edits") }
            Button(onClick = { store.go { st.apply() } }, enabled = !st.busy && !st.blocked && st.edited) { Text("Apply") }
        }
    }
}
