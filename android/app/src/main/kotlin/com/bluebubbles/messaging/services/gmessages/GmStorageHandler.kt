package com.bluebubbles.messaging.services.gmessages

import android.content.Context
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/**
 * Flutter method channel handler for Google Messages secure storage operations.
 */
class GmStorageHandler {

    /**
     * Handle method calls from Flutter.
     * 
     * Supported methods:
     * - gm-save-session: Save encrypted session data
     * - gm-load-session: Load and decrypt session data
     * - gm-clear-session: Clear stored session
     * - gm-has-session: Check if session exists
     */
    fun handle(call: MethodCall, result: MethodChannel.Result, context: Context) {
        val storage = GmSecureStorage(context)
        when (call.method) {
            "gm-save-session" -> {
                val data = call.argument<ByteArray>("data")
                if (data == null) {
                    result.error("INVALID_ARGUMENT", "Missing 'data' argument", null)
                    return
                }
                
                val success = storage.saveSession(data)
                if (success) {
                    result.success(true)
                } else {
                    result.error("STORAGE_ERROR", "Failed to save session", null)
                }
            }
            
            "gm-load-session" -> {
                val data = storage.loadSession()
                if (data != null) {
                    result.success(data)
                } else {
                    result.success(null)
                }
            }
            
            "gm-clear-session" -> {
                storage.clearSession()
                result.success(true)
            }
            
            "gm-has-session" -> {
                result.success(storage.hasSession())
            }
            
            "gm-delete-key" -> {
                storage.deleteKey()
                result.success(true)
            }
            
            else -> {
                result.notImplemented()
            }
        }
    }

    companion object {
        /**
         * Check if this handler can handle the given method.
         */
        fun canHandle(method: String): Boolean {
            return method.startsWith("gm-")
        }
    }
}
