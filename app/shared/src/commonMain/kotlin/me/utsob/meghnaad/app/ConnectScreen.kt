package me.utsob.meghnaad.app

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
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

/** The bucket's address and keys. [hint] says what it is for. */
@Composable
private fun BucketFields(
    endpoint: String, onEndpoint: (String) -> Unit,
    bucket: String, onBucket: (String) -> Unit,
    region: String, onRegion: (String) -> Unit,
    accessKey: String, onAccessKey: (String) -> Unit,
    secretKey: String, onSecretKey: (String) -> Unit,
) {
    val plain = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None)
    OutlinedTextField(
        value = endpoint,
        onValueChange = onEndpoint,
        label = { Text("S3 endpoint") },
        placeholder = { Text("https://<account>.r2.cloudflarestorage.com") },
        singleLine = true,
        keyboardOptions = plain.copy(keyboardType = KeyboardType.Uri),
        modifier = Modifier.fillMaxWidth(),
    )
    OutlinedTextField(
        value = bucket,
        onValueChange = onBucket,
        label = { Text("Bucket") },
        singleLine = true,
        keyboardOptions = plain,
        modifier = Modifier.fillMaxWidth(),
    )
    OutlinedTextField(
        value = region,
        onValueChange = onRegion,
        label = { Text("Region") },
        supportingText = { Text("“auto” for R2; the bucket's region (such as eu-west-1) for AWS.") },
        singleLine = true,
        keyboardOptions = plain,
        modifier = Modifier.fillMaxWidth(),
    )
    OutlinedTextField(
        value = accessKey,
        onValueChange = onAccessKey,
        label = { Text("Access key ID") },
        supportingText = { Text("“Access Key ID” in R2 and AWS, “keyID” in Backblaze.") },
        singleLine = true,
        keyboardOptions = plain,
        modifier = Modifier.fillMaxWidth(),
    )
    SecretField(secretKey, onSecretKey, "Secret access key", Modifier.fillMaxWidth())
}

/** The fields of a git connection: the repository, its branch, the forge's API and the token. */
@Composable
fun GitFields(
    repo: String, onRepo: (String) -> Unit,
    branch: String, onBranch: (String) -> Unit,
    api: String, onApi: (String) -> Unit,
    token: String, onToken: (String) -> Unit,
    tokenLabel: String = "Token",
) {
    val plain = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None)
    OutlinedTextField(
        value = repo,
        onValueChange = onRepo,
        label = { Text("Repository") },
        placeholder = { Text("owner/name") },
        supportingText = { Text("Keep it private: the app's settings and hooks are kept in it too.") },
        singleLine = true,
        keyboardOptions = plain,
        modifier = Modifier.fillMaxWidth(),
    )
    OutlinedTextField(
        value = branch,
        onValueChange = onBranch,
        label = { Text("Branch") },
        supportingText = { Text("sync.git.branch in your taskrc.") },
        singleLine = true,
        keyboardOptions = plain,
        modifier = Modifier.fillMaxWidth(),
    )
    OutlinedTextField(
        value = api,
        onValueChange = onApi,
        label = { Text("API address") },
        supportingText = { Text("https://api.github.com for GitHub; https://<your host>/api/v1 for Gitea and Forgejo.") },
        singleLine = true,
        keyboardOptions = plain.copy(keyboardType = KeyboardType.Uri),
        modifier = Modifier.fillMaxWidth(),
    )
    SecretField(token, onToken, tokenLabel, Modifier.fillMaxWidth())
    Text(
        "A token that may read and write the repository's contents: on GitHub a fine-grained token for that repository " +
            "with Contents read and write; on Gitea and Forgejo one with the repository scope.",
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

/** The first screen's and Settings' form for a device that is not connected: an S3 bucket, a sync server, or a git repository. */
@Composable
fun ConnectForm(connect: suspend (Connection) -> String?, modifier: Modifier = Modifier) {
    var where by remember { mutableStateOf("bucket") }
    val server = where == "server"
    val git = where == "git"
    var repo by remember { mutableStateOf("") }
    var branch by remember { mutableStateOf(Connection.GIT_BRANCH) }
    var api by remember { mutableStateOf(Connection.GITHUB_API) }
    var token by remember { mutableStateOf("") }
    var url by remember { mutableStateOf("") }
    var clientId by remember { mutableStateOf("") }
    var withBucket by remember { mutableStateOf(false) }
    var endpoint by remember { mutableStateOf("") }
    var bucket by remember { mutableStateOf("") }
    var region by remember { mutableStateOf("auto") }
    var accessKey by remember { mutableStateOf("") }
    var secretKey by remember { mutableStateOf("") }
    var secret by remember { mutableStateOf("") }
    var working by remember { mutableStateOf(false) }
    var problem by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    val typed = if (git) {
        Connection.typedGit(api, repo, branch, token, secret)
    } else if (server) {
        Connection.typedServer(
            url, clientId, secret,
            if (withBucket) endpoint else "", if (withBucket) bucket else "", region,
            if (withBucket) accessKey else "", if (withBucket) secretKey else "",
        )
    } else {
        Connection.typed(endpoint, bucket, region, accessKey, secretKey, secret)
    }
    val plain = KeyboardOptions(autoCorrectEnabled = false, capitalization = KeyboardCapitalization.None)

    Column(modifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text("Where do your tasks sync?", fontWeight = FontWeight.SemiBold)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            for ((key, label) in listOf("bucket" to "S3 bucket", "server" to "Sync server", "git" to "Git")) {
                if (where == key) Button(onClick = {}, modifier = Modifier.weight(1f)) { Text(label, maxLines = 1) }
                else OutlinedButton(onClick = { where = key }, modifier = Modifier.weight(1f)) { Text(label, maxLines = 1) }
            }
        }
        if (git) {
            GitFields(repo, { repo = it }, branch, { branch = it }, api, { api = it }, token, { token = it })
        } else if (server) {
            OutlinedTextField(
                value = url,
                onValueChange = { url = it },
                label = { Text("Server address") },
                placeholder = { Text("https://tasks.example.com") },
                supportingText = { Text("sync.server.url in your taskrc.") },
                singleLine = true,
                keyboardOptions = plain.copy(keyboardType = KeyboardType.Uri),
                modifier = Modifier.fillMaxWidth(),
            )
            OutlinedTextField(
                value = clientId,
                onValueChange = { clientId = it },
                label = { Text("Client ID") },
                supportingText = { Text("sync.server.client_id: the same on every device that shares these tasks. Starting new? Generate one.") },
                singleLine = true,
                keyboardOptions = plain,
                suffix = {
                    Text(
                        "Generate", Modifier.clickable { clientId = newClientId() }.padding(horizontal = 4.dp),
                        color = MaterialTheme.colorScheme.primary, fontWeight = FontWeight.Medium, fontSize = 14.sp,
                    )
                },
                modifier = Modifier.fillMaxWidth(),
            )
        } else {
            BucketFields(endpoint, { endpoint = it }, bucket, { bucket = it }, region, { region = it }, accessKey, { accessKey = it }, secretKey, { secretKey = it })
        }
        SecretField(secret, { secret = it }, "Encryption secret", Modifier.fillMaxWidth())
        Text(
            "The same value as sync.encryption_secret in your taskrc.",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (server) {
            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                Checkbox(withBucket, { withBucket = it })
                Text("Also keep settings and hooks in a bucket (optional)")
            }
            Text(
                "A bucket shares this app's settings and Lua hooks with the web app. The tasks themselves go to the server only.",
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (withBucket) {
                BucketFields(endpoint, { endpoint = it }, bucket, { bucket = it }, region, { region = it }, accessKey, { accessKey = it }, secretKey, { secretKey = it })
            }
        }
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
                "task CLI syncs with (Cloudflare R2, AWS S3, MinIO, anything that speaks S3), a TaskChampion sync server, or a " +
                "branch of a git repository on GitHub, Gitea or Forgejo. " +
                "The keys stay on this phone, in its secure storage.",
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
