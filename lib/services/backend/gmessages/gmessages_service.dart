import 'dart:async';
import 'dart:io';

import 'package:bluebubbles/database/database.dart';
import 'package:bluebubbles/helpers/types/constants.dart';
import 'package:bluebubbles/database/global/settings.dart';
import 'package:bluebubbles/utils/logger/logger.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';

/// GMessagesService manages the Google Messages integration.
/// 
/// This service handles:
/// - Keystore-backed encryption for session storage
/// - WebView pairing initiation
/// - Session lifecycle management
/// - Communication between Flutter and the native Android platform
/// 
/// Note: This feature is Android-only and experimental.
class GMessagesService extends GetxService {
  static const String _tag = 'GMessagesService';
  
  // Platform channel for native communication
  late final MethodChannel _channel;
  
  // Observable state
  final Rx<GMPairingState> pairingState = GMPairingState.notPaired.obs;
  final RxBool isInitialized = false.obs;
  final RxBool isPairing = false.obs;
  final RxString lastError = ''.obs;
  
  // Cached encrypted session data
  String? _encryptedCookies;
  String? _userAgent;
  
  /// Initializes the Google Messages service.
  /// Returns false if not on Android or if the feature is disabled.
  Future<bool> init() async {
    if (kIsWeb || !Platform.isAndroid) {
      Logger.debug('[$_tag] Not initializing - not Android');
      return false;
    }
    
    // Check if feature is enabled
    if (!ss.settings.enableGoogleMessages.value) {
      Logger.debug('[$_tag] Not initializing - feature disabled');
      return false;
    }
    
    Logger.info('[$_tag] Initializing Google Messages service');
    
    _channel = const MethodChannel('com.bluebubbles.messaging');
    
    // Listen for pairing events from native
    _setupMethodCallHandler();
    
    // Check for existing session
    await _checkExistingSession();
    
    isInitialized.value = true;
    Logger.info('[$_tag] Initialized successfully');
    return true;
  }
  
  void _setupMethodCallHandler() {
    // Note: This would need integration with the existing method channel handler
    // For now, we'll check pairing status on init
  }
  
  /// Checks if there's an existing paired session.
  Future<void> _checkExistingSession() async {
    try {
      final result = await _channel.invokeMethod('gm-check-pairing-status');
      if (result is Map) {
        final hasKey = result['has_encryption_key'] as bool? ?? false;
        if (hasKey) {
          // Try to load existing session
          pairingState.value = ss.settings.gmPairingState.value;
        }
      }
    } catch (e) {
      Logger.warn('[$_tag] Error checking existing session: $e');
    }
  }
  
  /// Returns true if the feature is available on this platform.
  bool get isAvailable => Platform.isAndroid;
  
  /// Returns true if Google Messages is enabled and paired.
  bool get isPaired => 
      ss.settings.enableGoogleMessages.value && 
      pairingState.value == GMPairingState.paired;
  
  /// Ensures the encryption key exists in Android Keystore.
  Future<bool> ensureKeystoreKey() async {
    if (!isAvailable) return false;
    
    try {
      final result = await _channel.invokeMethod('gm-keystore-ensure');
      return result as bool? ?? false;
    } catch (e) {
      Logger.error('[$_tag] Error ensuring keystore key: $e');
      lastError.value = e.toString();
      return false;
    }
  }
  
  /// Encrypts data using the Android Keystore-backed key.
  Future<String?> encryptData(String data) async {
    if (!isAvailable) return null;
    
    try {
      final result = await _channel.invokeMethod('gm-keystore-encrypt', {
        'data': data,
      });
      return result as String?;
    } catch (e) {
      Logger.error('[$_tag] Error encrypting data: $e');
      lastError.value = e.toString();
      return null;
    }
  }
  
  /// Decrypts data using the Android Keystore-backed key.
  Future<String?> decryptData(String encryptedData) async {
    if (!isAvailable) return null;
    
    try {
      final result = await _channel.invokeMethod('gm-keystore-decrypt', {
        'data': encryptedData,
      });
      return result as String?;
    } catch (e) {
      Logger.error('[$_tag] Error decrypting data: $e');
      lastError.value = e.toString();
      return null;
    }
  }
  
  /// Starts the WebView pairing flow.
  /// Returns a map with 'status' and optional 'encrypted_cookies', 'user_agent'.
  Future<Map<String, dynamic>> startPairing() async {
    if (!isAvailable) {
      return {'status': 'error', 'message': 'Not available on this platform'};
    }
    
    if (isPairing.value) {
      return {'status': 'error', 'message': 'Pairing already in progress'};
    }
    
    Logger.info('[$_tag] Starting Google Messages pairing');
    isPairing.value = true;
    pairingState.value = GMPairingState.waitingForPairing;
    lastError.value = '';
    
    try {
      // First ensure keystore key exists
      final keyReady = await ensureKeystoreKey();
      if (!keyReady) {
        isPairing.value = false;
        pairingState.value = GMPairingState.error;
        return {'status': 'error', 'message': 'Failed to initialize encryption'};
      }
      
      // Launch the WebView pairing activity
      final result = await _channel.invokeMethod('gm-start-pairing');
      
      if (result is Map) {
        final status = result['status'] as String?;
        
        if (status == 'paired') {
          _encryptedCookies = result['encrypted_cookies'] as String?;
          _userAgent = result['user_agent'] as String?;
          
          pairingState.value = GMPairingState.paired;
          ss.settings.gmPairingState.value = GMPairingState.paired;
          ss.settings.save();
          
          Logger.info('[$_tag] Pairing successful');
        } else if (status == 'cancelled') {
          pairingState.value = GMPairingState.notPaired;
          Logger.info('[$_tag] Pairing cancelled');
        } else if (status == 'launched') {
          // Activity launched, waiting for callback
          Logger.info('[$_tag] Pairing activity launched');
        } else {
          pairingState.value = GMPairingState.error;
          lastError.value = result['message'] as String? ?? 'Unknown error';
        }
        
        isPairing.value = false;
        return Map<String, dynamic>.from(result);
      }
      
      isPairing.value = false;
      return {'status': 'unknown'};
      
    } on PlatformException catch (e) {
      Logger.error('[$_tag] Platform error during pairing: ${e.message}');
      isPairing.value = false;
      pairingState.value = GMPairingState.error;
      lastError.value = e.message ?? 'Platform error';
      return {'status': 'error', 'message': e.message};
    } catch (e) {
      Logger.error('[$_tag] Error during pairing: $e');
      isPairing.value = false;
      pairingState.value = GMPairingState.error;
      lastError.value = e.toString();
      return {'status': 'error', 'message': e.toString()};
    }
  }
  
  /// Cancels an ongoing pairing operation.
  Future<bool> cancelPairing() async {
    if (!isAvailable || !isPairing.value) return false;
    
    try {
      final result = await _channel.invokeMethod('gm-cancel-pairing');
      isPairing.value = false;
      pairingState.value = GMPairingState.notPaired;
      return result as bool? ?? false;
    } catch (e) {
      Logger.error('[$_tag] Error cancelling pairing: $e');
      return false;
    }
  }
  
  /// Logs out and clears the Google Messages session.
  Future<bool> logout() async {
    if (!isAvailable) return false;
    
    Logger.info('[$_tag] Logging out of Google Messages');
    
    try {
      // Delete the encryption key (which invalidates all encrypted data)
      await _channel.invokeMethod('gm-keystore-delete');
      
      // Clear local state
      _encryptedCookies = null;
      _userAgent = null;
      pairingState.value = GMPairingState.notPaired;
      ss.settings.gmPairingState.value = GMPairingState.notPaired;
      ss.settings.save();
      
      Logger.info('[$_tag] Logged out successfully');
      return true;
    } catch (e) {
      Logger.error('[$_tag] Error during logout: $e');
      lastError.value = e.toString();
      return false;
    }
  }
  
  /// Gets the decrypted session cookies if paired.
  Future<String?> getSessionCookies() async {
    if (!isPaired || _encryptedCookies == null) return null;
    
    return await decryptData(_encryptedCookies!);
  }
  
  /// Gets the user agent used during pairing.
  String? get userAgent => _userAgent;
  
  /// Called when pairing is complete (from native callback).
  void onPairingComplete(String encryptedCookies, String userAgent) {
    _encryptedCookies = encryptedCookies;
    _userAgent = userAgent;
    pairingState.value = GMPairingState.paired;
    isPairing.value = false;
    
    ss.settings.gmPairingState.value = GMPairingState.paired;
    ss.settings.save();
    
    Logger.info('[$_tag] Pairing complete callback received');
  }
  
  /// Called when pairing fails (from native callback).
  void onPairingError(String error) {
    lastError.value = error;
    pairingState.value = GMPairingState.error;
    isPairing.value = false;
    
    Logger.error('[$_tag] Pairing error callback: $error');
  }
  
  /// Called when pairing is cancelled (from native callback).
  void onPairingCancelled() {
    pairingState.value = GMPairingState.notPaired;
    isPairing.value = false;
    
    Logger.info('[$_tag] Pairing cancelled callback received');
  }
  
  /// Gets diagnostic information about the Google Messages integration.
  /// Useful for troubleshooting and debugging.
  Map<String, dynamic> getDiagnostics() {
    return {
      'isAvailable': isAvailable,
      'isInitialized': isInitialized.value,
      'featureEnabled': ss.settings.enableGoogleMessages.value,
      'pairingState': pairingState.value.toString(),
      'isPairing': isPairing.value,
      'isPaired': isPaired,
      'hasEncryptedCookies': _encryptedCookies != null,
      'hasUserAgent': _userAgent != null,
      'lastError': lastError.value,
      'timestamp': DateTime.now().toIso8601String(),
    };
  }
  
  /// Logs diagnostic information for debugging purposes.
  void logDiagnostics() {
    final diag = getDiagnostics();
    Logger.info('[$_tag] === Google Messages Diagnostics ===');
    diag.forEach((key, value) {
      Logger.info('[$_tag]   $key: $value');
    });
    Logger.info('[$_tag] === End Diagnostics ===');
  }
}

/// Global instance accessor
GMessagesService get gms {
  if (Get.isRegistered<GMessagesService>()) {
    return Get.find<GMessagesService>();
  }
  return Get.put(GMessagesService());
}
