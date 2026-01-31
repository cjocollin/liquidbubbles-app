package com.bluebubbles.messaging.services.gmessages

import android.content.Context
import android.os.Build
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
 * GMKeystoreHelper manages the AES-256 encryption key for Google Messages session data.
 * 
 * This uses Android Keystore to generate and store a hardware-backed symmetric key
 * that is used to encrypt/decrypt the Google Messages session cookies and state.
 * 
 * The encryption scheme is AES-256-GCM which provides both confidentiality and integrity.
 */
class GMKeystoreHelper(private val context: Context) {
    
    companion object {
        private const val TAG = "GMKeystoreHelper"
        private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        private const val GM_KEY_ALIAS = "gmessages:session:aes256"
        private const val GCM_IV_LENGTH = 12
        private const val GCM_TAG_LENGTH = 128
    }
    
    private val keyStore: KeyStore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply {
        load(null)
    }
    
    /**
     * Checks if the GM encryption key exists in the Keystore.
     */
    fun hasKey(): Boolean {
        return try {
            keyStore.containsAlias(GM_KEY_ALIAS)
        } catch (e: Exception) {
            Log.e(TAG, "Error checking key existence: ${e.message}")
            false
        }
    }
    
    /**
     * Creates a new AES-256 key for GM session encryption if one doesn't exist.
     * Returns true if key was created or already exists, false on error.
     */
    fun ensureKeyExists(): Boolean {
        if (hasKey()) {
            Log.d(TAG, "GM encryption key already exists")
            return true
        }
        
        return try {
            createKey()
            true
        } catch (e: Exception) {
            Log.e(TAG, "Failed to create GM encryption key: ${e.message}")
            false
        }
    }
    
    /**
     * Creates a new AES-256-GCM key in the Android Keystore.
     */
    private fun createKey() {
        val keyGenerator = KeyGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_AES,
            KEYSTORE_PROVIDER
        )
        
        val builder = KeyGenParameterSpec.Builder(
            GM_KEY_ALIAS,
            KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
        ).apply {
            setKeySize(256)
            setBlockModes(KeyProperties.BLOCK_MODE_GCM)
            setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
            // GCM generates random IVs, so we don't require user authentication
            // The key is still hardware-backed when available
            setRandomizedEncryptionRequired(true)
        }
        
        keyGenerator.init(builder.build())
        keyGenerator.generateKey()
        
        Log.i(TAG, "Created new GM encryption key")
    }
    
    /**
     * Gets the raw key bytes for use in Rust encryption.
     * Note: This exports the key material, which is necessary for Rust-side encryption.
     * The key is still protected by Keystore when not in use.
     * 
     * Returns the 32-byte AES key as a Base64 string, or null on error.
     */
    fun getKeyBytes(): ByteArray? {
        return try {
            if (!ensureKeyExists()) {
                return null
            }
            
            val entry = keyStore.getEntry(GM_KEY_ALIAS, null) as? KeyStore.SecretKeyEntry
            if (entry == null) {
                Log.e(TAG, "Key entry not found")
                return null
            }
            
            // Note: On some devices, this may return null if the key is not extractable
            // In that case, we'll need to do encryption on the Android side
            val key = entry.secretKey
            
            // For Keystore keys, we can't directly export the key material.
            // Instead, we'll perform encryption/decryption on the Android side
            // and pass the encrypted data to Rust.
            // 
            // Return a derived key identifier that Rust can use to request
            // Android-side encryption operations.
            null
        } catch (e: Exception) {
            Log.e(TAG, "Failed to get key bytes: ${e.message}")
            null
        }
    }
    
    /**
     * Encrypts data using the GM AES-256-GCM key.
     * Returns the ciphertext with prepended IV (12 bytes IV + ciphertext + 16 bytes tag).
     */
    fun encrypt(plaintext: ByteArray): ByteArray? {
        return try {
            if (!ensureKeyExists()) {
                return null
            }
            
            val key = getSecretKey() ?: return null
            
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.ENCRYPT_MODE, key)
            
            val iv = cipher.iv
            val ciphertext = cipher.doFinal(plaintext)
            
            // Prepend IV to ciphertext
            iv + ciphertext
        } catch (e: Exception) {
            Log.e(TAG, "Encryption failed: ${e.message}")
            null
        }
    }
    
    /**
     * Decrypts data using the GM AES-256-GCM key.
     * Expects data format: 12 bytes IV + ciphertext + tag
     */
    fun decrypt(ciphertext: ByteArray): ByteArray? {
        return try {
            if (ciphertext.size < GCM_IV_LENGTH) {
                Log.e(TAG, "Ciphertext too short")
                return null
            }
            
            val key = getSecretKey() ?: return null
            
            val iv = ciphertext.copyOfRange(0, GCM_IV_LENGTH)
            val encryptedData = ciphertext.copyOfRange(GCM_IV_LENGTH, ciphertext.size)
            
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            val spec = GCMParameterSpec(GCM_TAG_LENGTH, iv)
            cipher.init(Cipher.DECRYPT_MODE, key, spec)
            
            cipher.doFinal(encryptedData)
        } catch (e: Exception) {
            Log.e(TAG, "Decryption failed: ${e.message}")
            null
        }
    }
    
    /**
     * Encrypts a string and returns Base64-encoded result.
     */
    fun encryptString(plaintext: String): String? {
        val encrypted = encrypt(plaintext.toByteArray(Charsets.UTF_8)) ?: return null
        return Base64.encodeToString(encrypted, Base64.NO_WRAP)
    }
    
    /**
     * Decrypts a Base64-encoded ciphertext and returns the plaintext string.
     */
    fun decryptString(base64Ciphertext: String): String? {
        val ciphertext = try {
            Base64.decode(base64Ciphertext, Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.e(TAG, "Invalid Base64 input: ${e.message}")
            return null
        }
        
        val decrypted = decrypt(ciphertext) ?: return null
        return String(decrypted, Charsets.UTF_8)
    }
    
    /**
     * Deletes the GM encryption key from Keystore.
     * This should be called when the user logs out of Google Messages.
     */
    fun deleteKey(): Boolean {
        return try {
            if (hasKey()) {
                keyStore.deleteEntry(GM_KEY_ALIAS)
                Log.i(TAG, "Deleted GM encryption key")
            }
            true
        } catch (e: Exception) {
            Log.e(TAG, "Failed to delete key: ${e.message}")
            false
        }
    }
    
    /**
     * Gets the SecretKey from the Keystore.
     */
    private fun getSecretKey(): SecretKey? {
        return try {
            val entry = keyStore.getEntry(GM_KEY_ALIAS, null) as? KeyStore.SecretKeyEntry
            entry?.secretKey
        } catch (e: Exception) {
            Log.e(TAG, "Failed to get secret key: ${e.message}")
            null
        }
    }
}
