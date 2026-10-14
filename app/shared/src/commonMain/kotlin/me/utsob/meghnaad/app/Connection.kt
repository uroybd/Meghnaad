package me.utsob.meghnaad.app

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put

/** Where the bucket is and how to open it. The secrets in it are only ever given to the engine, never drawn. */
@Serializable
data class Connection(
    val endpoint: String,
    val bucket: String,
    val region: String = "auto",
    val accessKey: String,
    val secretKey: String,
    val encryptionSecret: String,
) {
    /** What the engine's `sync` takes, with the files to keep in step with the bucket's. */
    fun toSyncJson(files: List<String> = emptyList()): String = buildJsonObject {
        put("files", JsonArray(files.map(::JsonPrimitive)))
        put("endpoint", endpoint)
        put("bucket", bucket)
        put("region", region)
        put("access_key", accessKey)
        put("secret_key", secretKey)
        put("encryption_secret", encryptionSecret)
    }.toString()

    /** What is missing, or null: every field but the region is needed. */
    fun problem(): String? =
        if (listOf(endpoint, bucket, accessKey, secretKey, encryptionSecret).any { it.isBlank() }) {
            "Every field but the region is needed."
        } else {
            null
        }

    companion object {
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
    }
}

/** Which of the shared files the phone keeps in step with the bucket's (the secrets of hooks never are). */
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
