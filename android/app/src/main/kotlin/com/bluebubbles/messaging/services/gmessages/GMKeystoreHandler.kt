package com.bluebubbles.messaging.services.gmessages

import android.content.Context
import android.util.Base64
import android.util.Log
import com.bluebubbles.messaging.models.MethodCallHandlerImpl
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/**
 * GMKeystoreHandler exposes Google Messages Keystore operations to Flutter/Dart.
 * 
 * Available methods:
 * - gm-keystore-ensure: Ensures the GM encryption key exists
 * - gm-keystore-encrypt: Encrypts data using the GM key
 * - gm-keystore-decrypt: Decrypts data using the GM key
 * - gm-keystore-delete: Deletes the GM encryption key
 * - gm-keystore-has-key: Checks if GM encryption key exists
 */
class GMKeystoreHandler : MethodCallHandlerImpl() {
    
    companion object {
        const val tag = "gm-keystore"
        private const val LOG_TAG = "GMKeystoreHandler"
    }
    
    override fun handleMethodCall(
        call: MethodCall,
        result: MethodChannel.Result,
        context: Context
    ) {
        val helper = GMKeystoreHelper(context)
        
        when (call.method) {
            "gm-keystore-ensure" -> handleEnsureKey(helper, result)
            "gm-keystore-encrypt" -> handleEncrypt(call, helper, result)
            "gm-keystore-decrypt" -> handleDecrypt(call, helper, result)
            "gm-keystore-delete" -> handleDeleteKey(helper, result)
            "gm-keystore-has-key" -> handleHasKey(helper, result)
            else -> {
                Log.w(LOG_TAG, "Unknown method: ${call.method}")
                result.notImplemented()
            }
        }
    }
    
    private fun handleEnsureKey(helper: GMKeystoreHelper, result: MethodChannel.Result) {
        try {
            val success = helper.ensureKeyExists()
            result.success(success)
        } catch (e: Exception) {
            Log.e(LOG_TAG, "Error ensuring key: ${e.message}")
            result.error("KEYSTORE_ERROR", "Failed to ensure key exists: ${e.message}", null)
        }
    }
    
    private fun handleEncrypt(
        call: MethodCall,
        helper: GMKeystoreHelper,
        result: MethodChannel.Result
    ) {
        try {
            val data = call.argument<String>("data")
            if (data == null) {
                result.error("INVALID_ARGUMENT", "Missing 'data' argument", null)
                return
            }
            
            val encrypted = helper.encryptString(data)
            if (encrypted == null) {
                result.error("ENCRYPTION_ERROR", "Failed to encrypt data", null)
                return
            }
            
            result.success(encrypted)
        } catch (e: Exception) {
            Log.e(LOG_TAG, "Error encrypting: ${e.message}")
            result.error("ENCRYPTION_ERROR", "Encryption failed: ${e.message}", null)
        }
    }
    
    private fun handleDecrypt(
        call: MethodCall,
        helper: GMKeystoreHelper,
        result: MethodChannel.Result
    ) {
        try {
            val data = call.argument<String>("data")
            if (data == null) {
                result.error("INVALID_ARGUMENT", "Missing 'data' argument", null)
                return
            }
            
            val decrypted = helper.decryptString(data)
            if (decrypted == null) {
                result.error("DECRYPTION_ERROR", "Failed to decrypt data", null)
                return
            }
            
            result.success(decrypted)
        } catch (e: Exception) {
            Log.e(LOG_TAG, "Error decrypting: ${e.message}")
            result.error("DECRYPTION_ERROR", "Decryption failed: ${e.message}", null)
        }
    }
    
    private fun handleDeleteKey(helper: GMKeystoreHelper, result: MethodChannel.Result) {
        try {
            val success = helper.deleteKey()
            result.success(success)
        } catch (e: Exception) {
            Log.e(LOG_TAG, "Error deleting key: ${e.message}")
            result.error("KEYSTORE_ERROR", "Failed to delete key: ${e.message}", null)
        }
    }
    
    private fun handleHasKey(helper: GMKeystoreHelper, result: MethodChannel.Result) {
        try {
            val hasKey = helper.hasKey()
            result.success(hasKey)
        } catch (e: Exception) {
            Log.e(LOG_TAG, "Error checking key: ${e.message}")
            result.error("KEYSTORE_ERROR", "Failed to check key existence: ${e.message}", null)
        }
    }
}
