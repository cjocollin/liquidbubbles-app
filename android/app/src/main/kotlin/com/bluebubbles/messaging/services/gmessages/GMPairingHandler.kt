package com.bluebubbles.messaging.services.gmessages

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.util.Log
import com.bluebubbles.messaging.MainActivity
import com.bluebubbles.messaging.models.MethodCallHandlerImpl
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/**
 * GMPairingHandler manages Google Messages WebView pairing operations.
 * 
 * Available methods:
 * - gm-start-pairing: Launches the WebView pairing activity
 * - gm-cancel-pairing: Cancels ongoing pairing
 * - gm-check-pairing-status: Returns current pairing status
 */
class GMPairingHandler : MethodCallHandlerImpl() {
    
    companion object {
        const val tag = "gm-pairing"
        private const val LOG_TAG = "GMPairingHandler"
        private const val REQUEST_CODE_PAIRING = 9001
        
        // Store the pending result for async callback
        var pendingResult: MethodChannel.Result? = null
    }
    
    override fun handleMethodCall(
        call: MethodCall,
        result: MethodChannel.Result,
        context: Context
    ) {
        when (call.method) {
            "gm-start-pairing" -> handleStartPairing(context, result)
            "gm-cancel-pairing" -> handleCancelPairing(result)
            "gm-check-pairing-status" -> handleCheckPairingStatus(context, result)
            else -> {
                Log.w(LOG_TAG, "Unknown method: ${call.method}")
                result.notImplemented()
            }
        }
    }
    
    private fun handleStartPairing(context: Context, result: MethodChannel.Result) {
        try {
            Log.i(LOG_TAG, "Starting Google Messages pairing")
            
            // Check if pairing is already in progress
            if (GMWebViewPairingActivity.currentActivity != null) {
                result.error("PAIRING_IN_PROGRESS", "Pairing is already in progress", null)
                return
            }
            
            // Store result for callback after activity completes
            pendingResult = result
            
            // Launch the pairing activity
            val intent = Intent(context, GMWebViewPairingActivity::class.java)
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            
            if (context is Activity) {
                context.startActivityForResult(intent, REQUEST_CODE_PAIRING)
            } else {
                // If launched from service context, use new task flag
                context.startActivity(intent)
                // Return immediately since we can't get activity result
                result.success(mapOf(
                    "status" to "launched",
                    "message" to "Pairing activity launched"
                ))
                pendingResult = null
            }
            
        } catch (e: Exception) {
            Log.e(LOG_TAG, "Error starting pairing: ${e.message}")
            result.error("PAIRING_ERROR", "Failed to start pairing: ${e.message}", null)
            pendingResult = null
        }
    }
    
    private fun handleCancelPairing(result: MethodChannel.Result) {
        try {
            val activity = GMWebViewPairingActivity.currentActivity
            if (activity != null) {
                Log.i(LOG_TAG, "Cancelling pairing")
                activity.finish()
                result.success(true)
            } else {
                result.success(false)
            }
        } catch (e: Exception) {
            Log.e(LOG_TAG, "Error cancelling pairing: ${e.message}")
            result.error("CANCEL_ERROR", "Failed to cancel pairing: ${e.message}", null)
        }
    }
    
    private fun handleCheckPairingStatus(context: Context, result: MethodChannel.Result) {
        try {
            val isPairingActive = GMWebViewPairingActivity.currentActivity != null
            val keystoreHelper = GMKeystoreHelper(context)
            val hasKey = keystoreHelper.hasKey()
            
            result.success(mapOf(
                "is_pairing_active" to isPairingActive,
                "has_encryption_key" to hasKey
            ))
        } catch (e: Exception) {
            Log.e(LOG_TAG, "Error checking status: ${e.message}")
            result.error("STATUS_ERROR", "Failed to check pairing status: ${e.message}", null)
        }
    }
    
    /**
     * Called from MainActivity.onActivityResult to handle pairing results.
     */
    fun handleActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        if (requestCode != REQUEST_CODE_PAIRING) return
        
        val result = pendingResult
        pendingResult = null
        
        if (result == null) {
            Log.w(LOG_TAG, "No pending result for pairing callback")
            return
        }
        
        try {
            when (resultCode) {
                GMWebViewPairingActivity.RESULT_PAIRED -> {
                    val encryptedCookies = data?.getStringExtra(GMWebViewPairingActivity.EXTRA_ENCRYPTED_COOKIES)
                    val userAgent = data?.getStringExtra(GMWebViewPairingActivity.EXTRA_USER_AGENT)
                    
                    result.success(mapOf(
                        "status" to "paired",
                        "encrypted_cookies" to (encryptedCookies ?: ""),
                        "user_agent" to (userAgent ?: "")
                    ))
                }
                GMWebViewPairingActivity.RESULT_CANCELLED -> {
                    result.success(mapOf(
                        "status" to "cancelled"
                    ))
                }
                GMWebViewPairingActivity.RESULT_ERROR -> {
                    val errorMessage = data?.getStringExtra(GMWebViewPairingActivity.EXTRA_ERROR_MESSAGE)
                    result.error("PAIRING_FAILED", errorMessage ?: "Unknown error", null)
                }
                else -> {
                    result.success(mapOf(
                        "status" to "unknown",
                        "result_code" to resultCode
                    ))
                }
            }
        } catch (e: Exception) {
            Log.e(LOG_TAG, "Error handling activity result: ${e.message}")
            result.error("RESULT_ERROR", "Failed to process pairing result: ${e.message}", null)
        }
    }
}
