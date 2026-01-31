package com.bluebubbles.messaging.services.gmessages

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Secure storage for Google Messages session data using Android Keystore.
 * Encrypts cookie/session data at rest using AES-GCM with a key stored in the
 * hardware-backed keystore.
 */
class GmSecureStorage(private val context: Context) {
    
    companion object {
        private const val TAG = "GmSecureStorage"
        private const val KEY_ALIAS = "gm_session_key"
        private const val ANDROID_KEYSTORE = "AndroidKeyStore"
        private const val PREFS_NAME = "gm_secure_prefs"
        private const val PREFS_KEY_SESSION = "encrypted_session"
        private const val AES_GCM_NO_PADDING = "AES/GCM/NoPadding"
        private const val GCM_TAG_LENGTH = 128
        private const val GCM_IV_LENGTH = 12
    }

    private val keyStore: KeyStore = KeyStore.getInstance(ANDROID_KEYSTORE).apply {
        load(null)
    }

    private val prefs by lazy {
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    }

    /**
     * Initialize the secure storage key if it doesn't exist.
     */
    fun initialize() {
        if (!keyStore.containsAlias(KEY_ALIAS)) {
            createKey()
        }
    }

    /**
     * Create a new AES-GCM key in the Android Keystore.
     */
    private fun createKey() {
        try {
            val keyGenerator = KeyGenerator.getInstance(
                KeyProperties.KEY_ALGORITHM_AES,
                ANDROID_KEYSTORE
            )
            
            val spec = KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            ).apply {
                setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                setKeySize(256)
                // Don't require user authentication for this key - the session
                // needs to be accessible for background sync
                setUserAuthenticationRequired(false)
            }.build()
            
            keyGenerator.init(spec)
            keyGenerator.generateKey()
            
            Log.i(TAG, "Created GM session encryption key")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to create encryption key", e)
            throw e
        }
    }

    /**
     * Get the secret key from the keystore.
     */
    private fun getKey(): SecretKey {
        val entry = keyStore.getEntry(KEY_ALIAS, null) as? KeyStore.SecretKeyEntry
            ?: throw IllegalStateException("Key not found in keystore")
        return entry.secretKey
    }

    /**
     * Encrypt and save session data.
     * Does NOT log the data for security.
     */
    fun saveSession(data: ByteArray): Boolean {
        return try {
            initialize()
            
            val cipher = Cipher.getInstance(AES_GCM_NO_PADDING)
            cipher.init(Cipher.ENCRYPT_MODE, getKey())
            
            val iv = cipher.iv
            val encryptedData = cipher.doFinal(data)
            
            // Combine IV + encrypted data
            val combined = ByteArray(iv.size + encryptedData.size)
            System.arraycopy(iv, 0, combined, 0, iv.size)
            System.arraycopy(encryptedData, 0, combined, iv.size, encryptedData.size)
            
            // Store as Base64
            val encoded = Base64.encodeToString(combined, Base64.NO_WRAP)
            prefs.edit().putString(PREFS_KEY_SESSION, encoded).apply()
            
            Log.i(TAG, "Saved encrypted GM session (${data.size} bytes)")
            true
        } catch (e: Exception) {
            Log.e(TAG, "Failed to save session", e)
            false
        }
    }

    /**
     * Load and decrypt session data.
     * Returns null if no session exists or decryption fails.
     */
    fun loadSession(): ByteArray? {
        return try {
            initialize()
            
            val encoded = prefs.getString(PREFS_KEY_SESSION, null)
                ?: return null
            
            val combined = Base64.decode(encoded, Base64.NO_WRAP)
            if (combined.size <= GCM_IV_LENGTH) {
                Log.w(TAG, "Invalid encrypted session data")
                return null
            }
            
            // Extract IV and encrypted data
            val iv = combined.copyOfRange(0, GCM_IV_LENGTH)
            val encryptedData = combined.copyOfRange(GCM_IV_LENGTH, combined.size)
            
            val cipher = Cipher.getInstance(AES_GCM_NO_PADDING)
            val spec = GCMParameterSpec(GCM_TAG_LENGTH, iv)
            cipher.init(Cipher.DECRYPT_MODE, getKey(), spec)
            
            val decrypted = cipher.doFinal(encryptedData)
            Log.i(TAG, "Loaded encrypted GM session (${decrypted.size} bytes)")
            decrypted
        } catch (e: Exception) {
            Log.e(TAG, "Failed to load session", e)
            null
        }
    }

    /**
     * Clear the stored session.
     */
    fun clearSession() {
        try {
            prefs.edit().remove(PREFS_KEY_SESSION).apply()
            Log.i(TAG, "Cleared GM session")
        } catch (e: Exception) {
            Log.e(TAG, "Failed to clear session", e)
        }
    }

    /**
     * Check if a session is stored.
     */
    fun hasSession(): Boolean {
        return prefs.getString(PREFS_KEY_SESSION, null) != null
    }

    /**
     * Delete the encryption key (full reset).
     */
    fun deleteKey() {
        try {
            if (keyStore.containsAlias(KEY_ALIAS)) {
                keyStore.deleteEntry(KEY_ALIAS)
                Log.i(TAG, "Deleted GM encryption key")
            }
            clearSession()
        } catch (e: Exception) {
            Log.e(TAG, "Failed to delete key", e)
        }
    }
}
