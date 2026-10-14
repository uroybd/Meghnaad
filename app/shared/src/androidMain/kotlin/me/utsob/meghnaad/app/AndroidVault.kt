package me.utsob.meghnaad.app

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import java.security.KeyStore
import java.security.SecureRandom
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * The connection to the bucket and the key that seal the hooks' secrets, encrypted with a key that lives in the Android
 * Keystore and never leaves it, so a copy of the app's files is not enough to read them. The Keystore is slow to answer
 * (seconds on an emulator, and the first time in a process): each thing is opened once and kept for as long as the app runs.
 */
class AndroidVault(context: Context) : Vault {
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

    private var engineKey: ByteArray? = null
    private var connection: Connection? = null
    private var connectionRead = false

    @Synchronized
    override fun engineKey(): ByteArray {
        engineKey?.let { return it }
        val key = prefs.getString(ENGINE_KEY, null)?.let(::open)
            ?: ByteArray(32).also {
                SecureRandom().nextBytes(it)
                prefs.edit().putString(ENGINE_KEY, seal(it)).apply()
            }
        engineKey = key
        return key
    }

    @Synchronized
    override fun connection(): Connection? {
        if (!connectionRead) {
            connection = prefs.getString(CONNECTION, null)?.let(::open)?.let {
                runCatching { engineJson.decodeFromString(Connection.serializer(), String(it)) }.getOrNull()
            }
            connectionRead = true
        }
        return connection
    }

    @Synchronized
    override fun saveConnection(connection: Connection) {
        val text = engineJson.encodeToString(Connection.serializer(), connection)
        prefs.edit().putString(CONNECTION, seal(text.toByteArray())).apply()
        this.connection = connection
        connectionRead = true
    }

    @Synchronized
    override fun clearConnection() {
        prefs.edit().remove(CONNECTION).apply()
        connection = null
        connectionRead = true
    }

    override var localOnly: Boolean
        @Synchronized get() = prefs.getBoolean(LOCAL_ONLY, false)
        @Synchronized set(value) = prefs.edit().putBoolean(LOCAL_ONLY, value).apply()

    override var syncOptions: SyncOptions
        @Synchronized get() = SyncOptions(prefs.getBoolean(SYNC_SETTINGS, false), prefs.getBoolean(SYNC_HOOKS, false))
        @Synchronized set(value) = prefs.edit().putBoolean(SYNC_SETTINGS, value.settings).putBoolean(SYNC_HOOKS, value.hooks).apply()

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
