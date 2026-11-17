package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** A TaskChampion sync server: where it is and who this phone is to it (a UUID the CLI and the other devices share). */
@Serializable
data class SyncServerAddress(val url: String, val clientId: String)

/**
 * A branch of a git repository on GitHub, Gitea or Forgejo, reached through the forge's API: the one the task CLI syncs to
 * with `sync.git.*`. [api] is `https://api.github.com` for GitHub, `https://<host>/api/v1` for Gitea and Forgejo; [token]
 * may read and write the repository.
 */
@Serializable
data class GitAddress(val api: String, val repo: String, val branch: String, val token: String)

/**
 * Where the tasks sync to and how to open them. The secrets in it are only ever given to the engine, never drawn.
 *
 * With a [server] the tasks sync to that TaskChampion sync server. The bucket fields are then optional, and only for
 * keeping the settings and the hooks in step with the web app's (a bucket needs all of them or none). With [git] they sync
 * to a branch of a git repository, which keeps those files too, beside the tasks. Without either the tasks sync to the
 * bucket, and the same bucket keeps those files.
 */
@Serializable
data class Connection(
    val endpoint: String = "",
    val bucket: String = "",
    val region: String = "auto",
    val accessKey: String = "",
    val secretKey: String = "",
    val encryptionSecret: String,
    val server: SyncServerAddress? = null,
    val git: GitAddress? = null,
) {
    /** Whether the bucket is filled in. */
    val hasBucket: Boolean get() = listOf(endpoint, bucket, accessKey, secretKey).all { it.isNotBlank() }

    private fun bucketJson() = buildJsonObject {
        put("endpoint", endpoint)
        put("bucket", bucket)
        put("region", region)
        put("access_key", accessKey)
        put("secret_key", secretKey)
    }

    /**
     * What the engine's `sync` takes, with the files to keep in step with the bucket's (none without a bucket) and, once
     * the person has said what to do with the tasks when this is not where they were synced before, [switch]
     * (`replace` or `merge`).
     */
    fun toSyncJson(files: List<String> = emptyList(), switch: String? = null): String = buildJsonObject {
        put("files", JsonArray(if (hasBucket || git != null) files.map(::JsonPrimitive) else emptyList()))
        if (git != null) {
            put("tasks", buildJsonObject {
                put("kind", "git"); put("api", git.api); put("repo", git.repo); put("branch", git.branch); put("token", git.token)
            })
        } else if (server != null) {
            put("tasks", buildJsonObject { put("kind", "server"); put("url", server.url); put("client_id", server.clientId) })
            if (hasBucket) put("files_bucket", bucketJson())
        } else {
            put("tasks", JsonObject(bucketJson() + ("kind" to JsonPrimitive("bucket"))))
        }
        put("encryption_secret", encryptionSecret)
        switch?.let { put("switch", it) }
    }.toString()

    /** What is missing, or null. */
    fun problem(): String? {
        val partial = listOf(endpoint, bucket, accessKey, secretKey).any { it.isNotBlank() } && !hasBucket
        if (git != null) {
            return when {
                listOf(git.repo, git.token, encryptionSecret).any { it.isBlank() } ->
                    "The repository, a token and the encryption secret are all needed."
                !REPO_SHAPE.matches(git.repo) -> "The repository is written owner/name, such as alice/tasks."
                !git.api.startsWith("https://") -> "The API address starts with https://."
                !BRANCH_SHAPE.matches(git.branch) || ".." in git.branch -> "That is not a branch name; taskchampion is the usual one."
                else -> null
            }
        }
        return when {
            server == null && listOf(endpoint, bucket, accessKey, secretKey, encryptionSecret).any { it.isBlank() } ->
                "Every field but the region is needed."
            server == null -> null
            server.url.isBlank() || encryptionSecret.isBlank() || server.clientId.isBlank() ->
                "The server's address, the client ID and the encryption secret are all needed."
            !UUID_SHAPE.matches(server.clientId) ->
                "The client ID is a UUID, such as 3f2b8d1e-9c4a-4b7e-8f21-5a6d0c9e1b34 (the same on every device that shares these tasks)."
            partial -> "To keep settings and hooks in a bucket, fill in the endpoint, the bucket and both keys; or leave them all empty."
            else -> null
        }
    }

    companion object {
        private val REPO_SHAPE = Regex("^[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$")
        private val BRANCH_SHAPE = Regex("^[A-Za-z0-9_][A-Za-z0-9._/-]*[A-Za-z0-9_-]$|^[A-Za-z0-9_]$")

        /** GitHub's API, the default forge. */
        const val GITHUB_API = "https://api.github.com"

        /** The branch the task CLI's manual suggests for `sync.git.branch`. */
        const val GIT_BRANCH = "taskchampion"

        private val UUID_SHAPE = Regex("^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$")

        /** As typed in a form: trimmed, no slash at the end of the endpoint, `auto` for no region. */
        fun typed(endpoint: String, bucket: String, region: String, accessKey: String, secretKey: String, secret: String) =
            Connection(
                endpoint.trim().trimEnd('/'),
                bucket.trim(),
                region.trim().ifBlank { "auto" },
                accessKey.trim(),
                secretKey.trim(),
                secret.trim(),
            )

        /** A git repository as typed in a form: GitHub's API and the `taskchampion` branch when left blank. */
        fun typedGit(api: String, repo: String, branch: String, token: String, secret: String) = Connection(
            encryptionSecret = secret.trim(),
            git = GitAddress(
                api.trim().trimEnd('/').ifBlank { GITHUB_API },
                repo.trim().trim('/'),
                branch.trim().ifBlank { GIT_BRANCH },
                token.trim(),
            ),
        )

        /** A sync server as typed in a form, with the optional bucket for the shared files (all of it blank for none). */
        fun typedServer(
            url: String, clientId: String, secret: String,
            endpoint: String = "", bucket: String = "", region: String = "", accessKey: String = "", secretKey: String = "",
        ) = typed(endpoint, bucket, region, accessKey, secretKey, secret)
            .copy(server = SyncServerAddress(url.trim().trimEnd('/'), clientId.trim().lowercase()))
    }
}

/**
 * What the Sync tab may show of the connection: where it is, and a hint of the key's name. Never a secret. [serverUrl] is
 * set when the tasks sync to a sync server, and then the bucket is only the one that keeps the shared files (if there is one).
 */
data class ConnectionInfo(
    val endpoint: String,
    val bucket: String,
    val region: String,
    val accessKeyHint: String,
    val serverUrl: String? = null,
    val clientIdHint: String? = null,
    val hasBucket: Boolean = true,
    val gitRepo: String? = null,
    val gitBranch: String? = null,
    val gitApi: String? = null,
) {
    /** Whether the settings and hooks can be kept in step somewhere: a bucket, or the branch the tasks are on. */
    val sharesFiles: Boolean get() = hasBucket || gitRepo != null
}

private fun hint(key: String) = if (key.length > 8) "${key.take(4)}…${key.takeLast(4)}" else "•".repeat(key.length)

fun Connection.info(): ConnectionInfo = ConnectionInfo(
    endpoint, bucket, region, hint(accessKey),
    serverUrl = server?.url,
    clientIdHint = server?.clientId?.let(::hint),
    hasBucket = hasBucket,
    gitRepo = git?.repo,
    gitBranch = git?.branch,
    gitApi = git?.api,
)

/** A fresh client ID for a sync server: a random UUID, which the server learns of the first time it is used. */
@OptIn(kotlin.uuid.ExperimentalUuidApi::class)
fun newClientId(): String = kotlin.uuid.Uuid.random().toString()

/** Which of the shared files the phone keeps in step with the bucket's (or the git branch's) (the secrets of hooks never are). */
data class SyncOptions(val settings: Boolean = false, val hooks: Boolean = false) {
    /** The files in the bucket these two stand for. */
    fun files(): List<String> = buildList {
        if (settings) addAll(listOf("web/config.json", "web/settings.json"))
        if (hooks) add("web/hooks.lua")
    }
}

/**
 * What the app keeps secret, and the choices that go with it: the connection to the bucket and the key that seals the
 * hooks' secrets, kept so that a copy of the app's files is not enough to read them (the Android Keystore).
 */
interface Vault {
    /** The key that seals the hooks' secrets (32 bytes), made on first use. */
    fun engineKey(): ByteArray

    fun connection(): Connection?

    fun saveConnection(connection: Connection)

    fun clearConnection()

    /** The app was told to keep the tasks here only: not being connected is then a way of using it, not a problem. */
    var localOnly: Boolean

    var syncOptions: SyncOptions
}
