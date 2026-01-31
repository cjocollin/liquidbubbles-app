package com.bluebubbles.messaging.services.gmessages

import android.annotation.SuppressLint
import android.app.Activity
import android.content.Intent
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.view.View
import android.view.ViewGroup
import android.webkit.CookieManager
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ProgressBar
import android.widget.TextView
import androidx.appcompat.app.AppCompatActivity
import com.bluebubbles.messaging.R
import com.bluebubbles.messaging.services.backend_ui_interop.MethodCallHandler
import org.json.JSONObject

/**
 * GMWebViewPairingActivity displays a WebView for Google Messages web pairing.
 * 
 * This activity:
 * 1. Loads messages.google.com/web in a WebView
 * 2. Detects successful pairing by monitoring URL changes and cookies
 * 3. Extracts and encrypts session cookies using Android Keystore
 * 4. Returns the encrypted session data to Flutter
 * 
 * The user scans the QR code displayed on this WebView using their phone's
 * Google Messages app to complete the pairing process.
 */
class GMWebViewPairingActivity : AppCompatActivity() {
    
    companion object {
        private const val TAG = "GMWebViewPairing"
        const val GOOGLE_MESSAGES_WEB_URL = "https://messages.google.com/web/authentication"
        const val RESULT_PAIRED = 1001
        const val RESULT_CANCELLED = 1002
        const val RESULT_ERROR = 1003
        
        const val EXTRA_ENCRYPTED_COOKIES = "encrypted_cookies"
        const val EXTRA_USER_AGENT = "user_agent"
        const val EXTRA_ERROR_MESSAGE = "error_message"
        
        // Keep track of current activity for callback
        var currentActivity: GMWebViewPairingActivity? = null
    }
    
    private lateinit var webView: WebView
    private lateinit var progressBar: ProgressBar
    private lateinit var statusText: TextView
    private var isPaired = false
    private var keystoreHelper: GMKeystoreHelper? = null
    
    @SuppressLint("SetJavaScriptEnabled")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        
        currentActivity = this
        keystoreHelper = GMKeystoreHelper(this)
        
        // Ensure encryption key exists before proceeding
        if (keystoreHelper?.ensureKeyExists() != true) {
            finishWithError("Failed to initialize encryption key")
            return
        }
        
        // Create the layout programmatically
        val rootLayout = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setBackgroundColor(Color.WHITE)
        }
        
        // Status/instruction text at top
        statusText = TextView(this).apply {
            text = "Scan this QR code with Google Messages on your phone"
            textSize = 16f
            setTextColor(Color.BLACK)
            setPadding(32, 32, 32, 16)
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
        }
        rootLayout.addView(statusText)
        
        // Progress bar
        progressBar = ProgressBar(this, null, android.R.attr.progressBarStyleHorizontal).apply {
            isIndeterminate = true
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            ).apply {
                setMargins(32, 0, 32, 0)
            }
        }
        rootLayout.addView(progressBar)
        
        // WebView container
        val webViewContainer = FrameLayout(this).apply {
            layoutParams = LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                0,
                1f
            )
        }
        
        // Create and configure WebView
        webView = WebView(this).apply {
            layoutParams = FrameLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
        }
        
        configureWebView()
        webViewContainer.addView(webView)
        rootLayout.addView(webViewContainer)
        
        setContentView(rootLayout)
        
        // Clear existing cookies and start fresh
        clearCookiesAndLoad()
    }
    
    @SuppressLint("SetJavaScriptEnabled")
    private fun configureWebView() {
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
            databaseEnabled = true
            cacheMode = WebSettings.LOAD_DEFAULT
            userAgentString = getDesktopUserAgent()
            
            // Enable mixed content for HTTPS
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                mixedContentMode = WebSettings.MIXED_CONTENT_COMPATIBILITY_MODE
            }
            
            // Support zoom for QR code visibility
            setSupportZoom(true)
            builtInZoomControls = true
            displayZoomControls = false
            
            // Allow file access for caching
            allowFileAccess = true
            allowContentAccess = true
        }
        
        // Cookie manager setup
        CookieManager.getInstance().apply {
            setAcceptCookie(true)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
                setAcceptThirdPartyCookies(webView, true)
            }
        }
        
        webView.webViewClient = object : WebViewClient() {
            override fun onPageFinished(view: WebView?, url: String?) {
                super.onPageFinished(view, url)
                Log.d(TAG, "Page finished loading: $url")
                progressBar.visibility = View.GONE
                
                // Check if we've been redirected to the conversations page (successful pairing)
                if (url != null && isPairedUrl(url)) {
                    onPairingSuccess(url)
                }
            }
            
            override fun shouldOverrideUrlLoading(
                view: WebView?,
                request: WebResourceRequest?
            ): Boolean {
                val url = request?.url?.toString() ?: return false
                Log.d(TAG, "URL loading: $url")
                
                // Check if navigating to a paired state
                if (isPairedUrl(url)) {
                    // Let it load, we'll capture cookies after page finishes
                    return false
                }
                
                // Stay within Google Messages domain
                if (url.contains("messages.google.com")) {
                    return false
                }
                
                // Block external navigation
                return true
            }
        }
        
        webView.webChromeClient = object : WebChromeClient() {
            override fun onProgressChanged(view: WebView?, newProgress: Int) {
                super.onProgressChanged(view, newProgress)
                if (newProgress < 100) {
                    progressBar.visibility = View.VISIBLE
                    progressBar.progress = newProgress
                } else {
                    progressBar.visibility = View.GONE
                }
            }
        }
    }
    
    private fun getDesktopUserAgent(): String {
        // Use a Chrome desktop user agent to ensure desktop web experience
        return "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    }
    
    private fun clearCookiesAndLoad() {
        val cookieManager = CookieManager.getInstance()
        
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            cookieManager.removeAllCookies { success ->
                Log.d(TAG, "Cookies cleared: $success")
                cookieManager.flush()
                loadPairingPage()
            }
        } else {
            @Suppress("DEPRECATION")
            cookieManager.removeAllCookie()
            loadPairingPage()
        }
    }
    
    private fun loadPairingPage() {
        Log.i(TAG, "Loading Google Messages web pairing page")
        statusText.text = "Scan this QR code with Google Messages on your phone"
        webView.loadUrl(GOOGLE_MESSAGES_WEB_URL)
    }
    
    /**
     * Checks if the URL indicates successful pairing.
     * Google Messages redirects to the conversations view after successful pairing.
     */
    private fun isPairedUrl(url: String): Boolean {
        return url.contains("messages.google.com/web/conversations") ||
               url.contains("messages.google.com/web/c/") ||
               (url.contains("messages.google.com/web") && 
                !url.contains("authentication") && 
                !url.contains("signin"))
    }
    
    private fun onPairingSuccess(url: String) {
        if (isPaired) return
        isPaired = true
        
        Log.i(TAG, "Pairing successful! Extracting cookies...")
        statusText.text = "Pairing successful! Saving session..."
        
        try {
            // Get all cookies for Google Messages
            val cookieManager = CookieManager.getInstance()
            val cookies = cookieManager.getCookie("https://messages.google.com") ?: ""
            
            if (cookies.isEmpty()) {
                Log.w(TAG, "No cookies found after pairing")
                finishWithError("No session cookies found")
                return
            }
            
            Log.d(TAG, "Got cookies, encrypting...")
            
            // Encrypt the cookies using the Keystore-backed key
            val encryptedCookies = keystoreHelper?.encryptString(cookies)
            if (encryptedCookies == null) {
                finishWithError("Failed to encrypt session data")
                return
            }
            
            // Return the encrypted data to Flutter
            val resultIntent = Intent().apply {
                putExtra(EXTRA_ENCRYPTED_COOKIES, encryptedCookies)
                putExtra(EXTRA_USER_AGENT, webView.settings.userAgentString)
            }
            
            // Also notify Flutter via method channel
            notifyFlutter("gm-pairing-complete", mapOf(
                "encrypted_cookies" to encryptedCookies,
                "user_agent" to webView.settings.userAgentString
            ))
            
            setResult(RESULT_PAIRED, resultIntent)
            finish()
            
        } catch (e: Exception) {
            Log.e(TAG, "Error during pairing completion: ${e.message}")
            finishWithError("Error saving session: ${e.message}")
        }
    }
    
    private fun finishWithError(message: String) {
        Log.e(TAG, "Pairing error: $message")
        
        notifyFlutter("gm-pairing-error", mapOf(
            "error" to message
        ))
        
        val resultIntent = Intent().apply {
            putExtra(EXTRA_ERROR_MESSAGE, message)
        }
        setResult(RESULT_ERROR, resultIntent)
        finish()
    }
    
    private fun notifyFlutter(method: String, arguments: Map<String, Any>) {
        try {
            MethodCallHandler.invokeMethod(method, arguments)
        } catch (e: Exception) {
            Log.w(TAG, "Failed to notify Flutter: ${e.message}")
        }
    }
    
    override fun onBackPressed() {
        if (webView.canGoBack() && !isPaired) {
            webView.goBack()
        } else {
            notifyFlutter("gm-pairing-cancelled", emptyMap())
            setResult(RESULT_CANCELLED)
            super.onBackPressed()
        }
    }
    
    override fun onDestroy() {
        currentActivity = null
        
        // Clean up WebView
        webView.stopLoading()
        webView.destroy()
        
        super.onDestroy()
    }
}
