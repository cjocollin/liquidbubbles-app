import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:bluebubbles/database/models.dart';
import 'package:bluebubbles/services/services.dart';
import 'package:bluebubbles/helpers/helpers.dart';
import 'package:bluebubbles/utils/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:flutter_inappwebview/flutter_inappwebview.dart';
import 'package:get/get.dart';
import 'package:universal_io/io.dart';

/// WebView screen for Google Messages pairing
/// Opens messages.google.com/web/authentication for QR code pairing
class GmPairingWebView extends StatefulWidget {
  const GmPairingWebView({super.key});

  @override
  State<GmPairingWebView> createState() => _GmPairingWebViewState();
}

class _GmPairingWebViewState extends State<GmPairingWebView> {
  InAppWebViewController? _webViewController;
  final RxBool _isLoading = true.obs;
  final RxString _statusMessage = "Loading Google Messages...".obs;
  Timer? _pairingCheckTimer;
  bool _pairingDetected = false;

  static const String _gmAuthUrl = "https://messages.google.com/web/authentication";
  static const String _gmConversationsUrl = "https://messages.google.com/web/conversations";

  // User agent that mimics Chrome on desktop (needed for GM web to work)
  static const String _desktopUserAgent = 
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
      "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

  @override
  void initState() {
    super.initState();
    ss.settings.gmPairingState.value = GmPairingState.waitingForPairing;
  }

  @override
  void dispose() {
    _pairingCheckTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!Platform.isAndroid) {
      return Scaffold(
        appBar: AppBar(title: const Text("Pair Google Messages")),
        body: const Center(
          child: Text("Google Messages pairing is only available on Android"),
        ),
      );
    }

    return Scaffold(
      appBar: AppBar(
        title: const Text("Pair Google Messages"),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            onPressed: () => _webViewController?.reload(),
            tooltip: "Reload",
          ),
        ],
      ),
      body: Stack(
        children: [
          // WebView
          InAppWebView(
            initialUrlRequest: URLRequest(url: WebUri(_gmAuthUrl)),
            initialSettings: InAppWebViewSettings(
              userAgent: _desktopUserAgent,
              javaScriptEnabled: true,
              domStorageEnabled: true,
              databaseEnabled: true,
              clearCache: false,
              cacheEnabled: true,
              // Needed for GM authentication
              thirdPartyCookiesEnabled: true,
              // Needed to capture cookies
              useShouldInterceptRequest: false,
            ),
            onWebViewCreated: (controller) {
              _webViewController = controller;
            },
            onLoadStart: (controller, url) {
              _isLoading.value = true;
              _statusMessage.value = "Loading...";
              Logger.debug("GM WebView loading: $url");
            },
            onLoadStop: (controller, url) async {
              _isLoading.value = false;
              
              final urlString = url?.toString() ?? "";
              Logger.debug("GM WebView loaded: $urlString");
              
              // Check if we've navigated to conversations (means pairing succeeded)
              if (urlString.contains("/web/conversations") || 
                  urlString.contains("/web/u/0/")) {
                _onPairingSuccess();
              } else if (urlString.contains("/web/authentication")) {
                _statusMessage.value = "Scan the QR code with Google Messages";
                _startPairingCheck();
              }
            },
            onConsoleMessage: (controller, consoleMessage) {
              Logger.debug("GM WebView console: ${consoleMessage.message}");
            },
            onLoadError: (controller, url, code, message) {
              _isLoading.value = false;
              _statusMessage.value = "Error loading: $message";
              Logger.error("GM WebView error: $code - $message");
            },
          ),
          
          // Loading overlay
          Obx(() {
            if (!_isLoading.value) return const SizedBox.shrink();
            
            return Container(
              color: Colors.black26,
              child: const Center(
                child: CircularProgressIndicator(),
              ),
            );
          }),
          
          // Status bar at bottom
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            child: Obx(() => Container(
              color: context.theme.colorScheme.surfaceVariant,
              padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
              child: Row(
                children: [
                  Icon(
                    _getStatusIcon(),
                    size: 16,
                    color: context.theme.colorScheme.onSurfaceVariant,
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      _statusMessage.value,
                      style: TextStyle(
                        fontSize: 12,
                        color: context.theme.colorScheme.onSurfaceVariant,
                      ),
                    ),
                  ),
                ],
              ),
            )),
          ),
        ],
      ),
    );
  }

  IconData _getStatusIcon() {
    if (_isLoading.value) return Icons.hourglass_empty;
    if (_pairingDetected) return Icons.check_circle;
    return Icons.qr_code;
  }

  /// Start polling to check if pairing is complete
  void _startPairingCheck() {
    _pairingCheckTimer?.cancel();
    _pairingCheckTimer = Timer.periodic(const Duration(seconds: 3), (timer) async {
      if (_pairingDetected) {
        timer.cancel();
        return;
      }
      
      // Check current URL
      final url = await _webViewController?.getUrl();
      final urlString = url?.toString() ?? "";
      
      if (urlString.contains("/web/conversations") || 
          urlString.contains("/web/u/0/")) {
        timer.cancel();
        _onPairingSuccess();
      }
    });
  }

  /// Called when pairing is detected as successful
  Future<void> _onPairingSuccess() async {
    if (_pairingDetected) return;
    _pairingDetected = true;
    
    _statusMessage.value = "Pairing successful! Extracting session...";
    Logger.info("GM pairing detected as successful");
    
    try {
      // Get cookies from the WebView
      final cookies = await _extractCookies();
      
      if (cookies.isEmpty) {
        throw Exception("No cookies found after pairing");
      }
      
      // Build cookie header string for Rust API
      final cookieHeader = cookies
          .map((c) => "${c.name}=${c.value}")
          .join("; ");
      
      Logger.info("GM: extracted ${cookies.length} cookies");
      
      // Build session data structure
      final sessionData = {
        'cookies': cookieHeader,
        'user_agent': _desktopUserAgent,
        'paired_at': DateTime.now().toIso8601String(),
      };
      
      // Save to Android Keystore-backed secure storage
      final sessionJson = jsonEncode(sessionData);
      final sessionBytes = Uint8List.fromList(utf8.encode(sessionJson));
      
      final saved = await GmSecureStorageService.saveSession(sessionBytes);
      if (!saved) {
        throw Exception("Failed to save session to secure storage");
      }
      
      Logger.info("GM: session saved to secure storage");
      
      // TODO: Once FRB bindings are regenerated, initialize Rust session:
      // await RustLib.instance.api.crateApiApiGmSetCookiesFromWebview(
      //   cookies: cookieHeader,
      //   userAgent: _desktopUserAgent,
      // );
      
      // Update pairing state
      ss.settings.gmPairingState.value = GmPairingState.paired;
      await ss.settings.saveOne('gmPairingState');
      
      _statusMessage.value = "Paired successfully!";
      
      // Show success and navigate back
      if (mounted) {
        showSnackbar("Success", "Google Messages paired successfully!");
        
        // Delay briefly then pop back
        await Future.delayed(const Duration(milliseconds: 1500));
        if (mounted) {
          Navigator.of(context).pop(true);
        }
      }
    } catch (e) {
      Logger.error("GM pairing failed to save session", error: e);
      _statusMessage.value = "Error saving session: $e";
      
      ss.settings.gmPairingState.value = GmPairingState.error;
      await ss.settings.saveOne('gmPairingState');
      
      if (mounted) {
        showSnackbar("Error", "Failed to save pairing: $e");
      }
    }
  }

  /// Extract cookies from the WebView
  Future<List<Cookie>> _extractCookies() async {
    final cookieManager = CookieManager.instance();
    
    // Get cookies for messages.google.com
    final cookies = await cookieManager.getCookies(
      url: WebUri("https://messages.google.com"),
    );
    
    // Also get cookies for google.com (for auth)
    final googleCookies = await cookieManager.getCookies(
      url: WebUri("https://www.google.com"),
    );
    
    // Combine, removing duplicates
    final allCookies = <String, Cookie>{};
    for (final c in [...cookies, ...googleCookies]) {
      allCookies[c.name] = c;
    }
    
    return allCookies.values.toList();
  }
}
