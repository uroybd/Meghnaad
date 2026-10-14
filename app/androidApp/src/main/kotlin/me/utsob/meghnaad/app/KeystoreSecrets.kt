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
 * The key that seals the hooks' secrets, encrypted with a key that lives in the Android Keystore and never leaves it, so a
 * copy of the app's files is not enough to read it. (The connection to the bucket comes with the sync screens.)
 */
class KeystoreSecrets(context: Context) : SecretStore {
    private val prefs = context.applicationContext.getSharedPreferences("vault", Context.MODE_PRIVATE)
    private var cached: ByteArray? = null

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

    @Synchronized
    override fun engineKey(): ByteArray {
        cached?.let { return it }
        val key = prefs.getString(ENGINE_KEY, null)?.let(::open)
            ?: ByteArray(32).also {
                SecureRandom().nextBytes(it)
                prefs.edit().putString(ENGINE_KEY, seal(it)).apply()
            }
        cached = key
        return key
    }

    private companion object {
        const val ALIAS = "meghnaad-next-vault"
        const val ENGINE_KEY = "engine_key"
        const val IV_BYTES = 12
    }
}
