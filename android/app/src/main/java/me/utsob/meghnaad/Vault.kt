package me.utsob.meghnaad

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import org.json.JSONObject
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Where the bucket is and how to open it. The secrets in it are only ever given to the engine, never to the page. */
data class Connection(
    val endpoint: String,
    val bucket: String,
    val region: String,
    val accessKey: String,
    val secretKey: String,
    val encryptionSecret: String,
) {
    /** What the engine's `sync` takes, with the files to keep in step with the bucket's. */
    fun toSyncJson(files: List<String> = emptyList()): String = JSONObject()
        .put("files", org.json.JSONArray(files))
        .put("endpoint", endpoint)
        .put("bucket", bucket)
        .put("region", region)
        .put("access_key", accessKey)
        .put("secret_key", secretKey)
        .put("encryption_secret", encryptionSecret)
        .toString()

    fun toJson(): String = JSONObject()
        .put("endpoint", endpoint)
        .put("bucket", bucket)
        .put("region", region)
        .put("accessKey", accessKey)
        .put("secretKey", secretKey)
        .put("encryptionSecret", encryptionSecret)
        .toString()

    companion object {
        fun fromJson(text: String): Connection {
            val j = JSONObject(text)
            return Connection(
                j.getString("endpoint"),
                j.getString("bucket"),
                j.optString("region", "auto").ifBlank { "auto" },
                j.getString("accessKey"),
                j.getString("secretKey"),
                j.getString("encryptionSecret"),
            )
        }
    }
}

/**
 * What the phone keeps secret: the connection to the bucket, and the key that seals the hooks' secrets. They are
 * encrypted with a key that lives in the Android Keystore and never leaves it, so a copy of the app's files is not
 * enough to read them.
 */
class Vault(context: Context) {
    private val prefs = context.applicationContext.getSharedPreferences("vault", Context.MODE_PRIVATE)

    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    private fun seal(plain: ByteArray): String {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key()) }
        return Base64.encodeToString(cipher.iv + cipher.doFinal(plain), Base64.NO_WRAP)
    }

    private fun open(sealed: String): ByteArray? = try {
        val bytes = Base64.decode(sealed, Base64.NO_WRAP)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            .apply { init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, bytes, 0, IV_BYTES)) }
        cipher.doFinal(bytes, IV_BYTES, bytes.size - IV_BYTES)
    } catch (e: Exception) {
        null
    }

    // The Keystore is slow to answer, seconds on an emulator and the first time in a process: each thing is opened once
    // and kept in memory for as long as the app runs.
    private var engineKey: ByteArray? = null
    private var connection: Connection? = null
    private var connectionRead = false

    /** The key that seals the hooks' secrets, made on first use. */
    @Synchronized
    fun engineKey(): ByteArray {
        engineKey?.let { return it }
        val key = prefs.getString(ENGINE_KEY, null)?.let { s -> open(s) }
            ?: ByteArray(32).also {
                SecureRandom().nextBytes(it)
                prefs.edit().putString(ENGINE_KEY, seal(it)).apply()
            }
        engineKey = key
        return key
    }

    /** Whether the settings (the taskrc subset and the app's own) are kept in step with the bucket's, as the web app keeps them. */
    @Synchronized
    fun syncSettings(): Boolean = prefs.getBoolean(SYNC_SETTINGS, false)

    /** Whether the hooks script is kept in step with the bucket's. */
    @Synchronized
    fun syncHooks(): Boolean = prefs.getBoolean(SYNC_HOOKS, false)

    @Synchronized
    fun setSyncOptions(settings: Boolean, hooks: Boolean) {
        prefs.edit().putBoolean(SYNC_SETTINGS, settings).putBoolean(SYNC_HOOKS, hooks).apply()
    }

    /** The phone was told to keep the tasks here only: not being connected to a bucket is then not a problem. */
    @Synchronized
    fun localOnly(): Boolean = prefs.getBoolean(LOCAL_ONLY, false)

    @Synchronized
    fun setLocalOnly(value: Boolean) {
        prefs.edit().putBoolean(LOCAL_ONLY, value).apply()
    }

    @Synchronized
    fun connection(): Connection? {
        if (!connectionRead) {
            connection = prefs.getString(CONNECTION, null)?.let { s -> open(s)?.let { Connection.fromJson(String(it)) } }
            connectionRead = true
        }
        return connection
    }

    @Synchronized
    fun save(c: Connection) {
        prefs.edit().putString(CONNECTION, seal(c.toJson().toByteArray())).apply()
        connection = c
        connectionRead = true
    }

    @Synchronized
    fun clearConnection() {
        prefs.edit().remove(CONNECTION).apply()
        connection = null
        connectionRead = true
    }

    private companion object {
        const val ALIAS = "meghnaad-vault"
        const val ENGINE_KEY = "engine_key"
        const val CONNECTION = "connection"
        const val LOCAL_ONLY = "local_only"
        const val SYNC_SETTINGS = "sync_settings"
        const val SYNC_HOOKS = "sync_hooks"
        const val IV_BYTES = 12
    }
}
