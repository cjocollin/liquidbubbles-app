import 'dart:typed_data';
import 'package:bluebubbles/utils/logger/logger.dart';
import 'package:flutter/services.dart';
import 'package:universal_io/io.dart';

/// Flutter service to interact with Android's secure storage for Google Messages.
/// Uses Android Keystore for encryption on Android, and is a no-op on other platforms.
class GmSecureStorageService {
  static const MethodChannel _channel = MethodChannel('com.bluebubbles.messaging');
  
  /// Check if this platform supports secure storage for GM
  static bool get isSupported => Platform.isAndroid;
  
  /// Save session data to secure storage
  /// [data] should be the JSON-serialized session data as bytes
  static Future<bool> saveSession(Uint8List data) async {
    if (!isSupported) {
      Logger.warn("GmSecureStorage: Platform not supported");
      return false;
    }
    
    try {
      final result = await _channel.invokeMethod<bool>('gm-save-session', {
        'data': data,
      });
      return result ?? false;
    } on PlatformException catch (e) {
      Logger.error("GmSecureStorage: Failed to save session", error: e);
      return false;
    }
  }
  
  /// Load session data from secure storage
  /// Returns null if no session exists or decryption fails
  static Future<Uint8List?> loadSession() async {
    if (!isSupported) {
      Logger.warn("GmSecureStorage: Platform not supported");
      return null;
    }
    
    try {
      final result = await _channel.invokeMethod<Uint8List>('gm-load-session');
      return result;
    } on PlatformException catch (e) {
      Logger.error("GmSecureStorage: Failed to load session", error: e);
      return null;
    }
  }
  
  /// Clear the stored session
  static Future<void> clearSession() async {
    if (!isSupported) return;
    
    try {
      await _channel.invokeMethod('gm-clear-session');
    } on PlatformException catch (e) {
      Logger.error("GmSecureStorage: Failed to clear session", error: e);
    }
  }
  
  /// Check if a session is stored
  static Future<bool> hasSession() async {
    if (!isSupported) return false;
    
    try {
      final result = await _channel.invokeMethod<bool>('gm-has-session');
      return result ?? false;
    } on PlatformException catch (e) {
      Logger.error("GmSecureStorage: Failed to check session", error: e);
      return false;
    }
  }
  
  /// Delete the encryption key (full reset)
  static Future<void> deleteKey() async {
    if (!isSupported) return;
    
    try {
      await _channel.invokeMethod('gm-delete-key');
    } on PlatformException catch (e) {
      Logger.error("GmSecureStorage: Failed to delete key", error: e);
    }
  }
}
