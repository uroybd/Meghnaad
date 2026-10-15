package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.foundation.focusable
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.focus.focusRequester
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

/** A text field for a secret: hidden until asked, never remembered by the keyboard. */
@Composable
fun SecretField(value: String, onChange: (String) -> Unit, label: String, modifier: Modifier = Modifier) {
    var shown by remember { mutableStateOf(false) }
    OutlinedTextField(
        value = value,
        onValueChange = onChange,
        label = { Text(label) },
        singleLine = true,
        visualTransformation = if (shown) VisualTransformation.None else PasswordVisualTransformation(),
        keyboardOptions = KeyboardOptions(autoCorrectEnabled = false, keyboardType = KeyboardType.Password),
        // A suffix, not a trailing icon: Material gives an icon a 48 dp slot, which would make this field taller than the others.
        suffix = {
            Text(
                if (shown) "Hide" else "Show", Modifier.clickable { shown = !shown }.padding(horizontal = 4.dp),
                color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Medium, fontSize = 14.sp,
            )
        },
        modifier = modifier,
    )
}

/** The bucket's address and keys: the form of the first screen, and of Settings when the device is not connected. */
@Composable
fun ConnectForm(connect: suspend (Connection) -> String?, modifier: Modifier = Modifier) {
    var endpoint by remember { mutableStateOf("") }
    var bucket by remember { mutableStateOf("") }
    var region by remember { mutableStateOf("auto") }
    var accessKey by remember { mutableStateOf("") }
    var secretKey by remember { mutableStateOf("") }
    var secret by remember { mutableStateOf("") }
    var working by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    val typed = Connection.typed(endpoint, bucket, region, accessKey, secretKey, secret)
    val plain = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None)

    Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        OutlinedTextField(
            value = endpoint,
            onValueChange = { endpoint = it },
            label = { Text("S3 endpoint") },
            placeholder = { Text("https://<account>.r2.cloudflarestorage.com") },
            singleLine = true,
            keyboardOptions = plain.copy(keyboardType = KeyboardType.Uri),
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = bucket,
            onValueChange = { bucket = it },
            label = { Text("Bucket") },
            singleLine = true,
            keyboardOptions = plain,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = region,
            onValueChange = { region = it },
            label = { Text("Region") },
            supportingText = { Text("“auto” for R2; the bucket's region (such as eu-west-1) for AWS.") },
            singleLine = true,
            keyboardOptions = plain,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = accessKey,
            onValueChange = { accessKey = it },
            label = { Text("Access key ID") },
            supportingText = { Text("“Access Key ID” in R2 and AWS, “keyID” in Backblaze.") },
            singleLine = true,
            keyboardOptions = plain,
            modifier = Modifier.fillMaxWidth(),
        )
        SecretField(secretKey, { secretKey = it }, "Secret access key", Modifier.fillMaxWidth())
        SecretField(secret, { secret = it }, "Encryption secret", Modifier.fillMaxWidth())
        Text(
            "The same value as sync.encryption_secret in your taskrc.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (problem.isNotEmpty()) Text(problem, color = MaterialTheme.colorScheme.error)
        Button(
            onClick = {
                working = true
                problem = ""
                scope.launch {
                    problem = connect(typed) ?: ""
                    if (problem.isNotEmpty()) working = false
                }
            },
            enabled = typed.problem() == null && !working,
            modifier = Modifier.fillMaxWidth(),
        ) { Text(if (working) "Connecting…" else "Connect") }
    }
}

/** The first screen, before the tasks: connect to a bucket, or go on without one. */
@Composable
fun ConnectScreen(store: Store) {
    // The page itself takes the focus, so the keyboard does not open over the form before a field is asked for.
    val focus = remember { androidx.compose.ui.focus.FocusRequester() }
    LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
    Column(
        Modifier.verticalScroll(rememberScrollState()).padding(horizontal = 20.dp, vertical = 24.dp)
            .focusRequester(focus).focusable(),
    ) {
        Text("Your tasks", style = MaterialTheme.typography.headlineMedium)
        Text(
            "Meghnaad keeps your tasks on this phone. To have them on your other devices too, connect the S3 bucket your " +
                "task CLI syncs with (Cloudflare R2, AWS S3, MinIO, anything that speaks S3). The keys stay on this phone, in " +
                "its secure storage.",
            modifier = Modifier.padding(vertical = 12.dp),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        ConnectForm(connect = store::connect)
        Text(
            "Or start without syncing: everything works, and you can connect later in Settings → Sync.",
            modifier = Modifier.padding(top = 28.dp, bottom = 8.dp),
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        OutlinedButton(onClick = store::useLocally, modifier = Modifier.fillMaxWidth()) { Text("Use on this phone only") }
    }
}
