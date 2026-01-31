import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:bluebubbles/database/models.dart';
import 'package:bluebubbles/services/services.dart';
import 'package:bluebubbles/utils/logger/logger.dart';
import 'package:get/get.dart';
import 'package:universal_io/io.dart';

/// Service to manage Google Messages threads and messages
/// This is kept separate from the main ChatsService to avoid disrupting iMessage functionality
class GmChatsService extends GetxService {
  static GmChatsService get to => Get.find<GmChatsService>();
  
  /// Whether the service is initialized and ready
  final RxBool isInitialized = false.obs;
  
  /// Whether sync is currently in progress
  final RxBool isSyncing = false.obs;
  
  /// List of GM thread summaries (lightweight thread info)
  final RxList<GmThreadSummary> threads = <GmThreadSummary>[].obs;
  
  /// Map of thread ID to full message list
  final RxMap<String, List<GmMessageData>> messages = <String, List<GmMessageData>>{}.obs;
  
  /// Last sync timestamp
  DateTime? lastSyncTime;
  
  /// Sync timer for periodic updates
  Timer? _syncTimer;
  
  /// Error message if something went wrong
  final RxnString errorMessage = RxnString(null);

  @override
  void onInit() {
    super.onInit();
    _initializeIfEnabled();
  }
  
  @override
  void onClose() {
    _syncTimer?.cancel();
    super.onClose();
  }
  
  /// Initialize the service if GM is enabled
  Future<void> _initializeIfEnabled() async {
    if (!Platform.isAndroid) return;
    if (!ss.settings.enableGoogleMessages.value) return;
    if (ss.settings.gmPairingState.value != GmPairingState.paired) return;
    
    await initialize();
  }
  
  /// Initialize the service and start syncing
  Future<void> initialize() async {
    if (!Platform.isAndroid) {
      Logger.warn("GmChatsService: Not on Android, skipping initialization");
      return;
    }
    
    try {
      Logger.info("GmChatsService: Initializing...");
      
      // Load session from secure storage
      final sessionData = await GmSecureStorageService.loadSession();
      if (sessionData == null) {
        Logger.warn("GmChatsService: No session found");
        ss.settings.gmPairingState.value = GmPairingState.notPaired;
        await ss.settings.saveOne('gmPairingState');
        return;
      }
      
      // Parse session data
      final sessionJson = utf8.decode(sessionData);
      final session = jsonDecode(sessionJson) as Map<String, dynamic>;
      
      Logger.info("GmChatsService: Session loaded, paired at ${session['paired_at']}");
      
      // TODO: Initialize Rust session with cookies
      // await RustLib.instance.api.crateApiApiGmSetCookiesFromWebview(
      //   cookies: session['cookies'],
      //   userAgent: session['user_agent'],
      // );
      
      isInitialized.value = true;
      
      // Start periodic sync (every 30 seconds when in foreground)
      _startSyncTimer();
      
      // Do an initial sync
      await syncNow();
      
      Logger.info("GmChatsService: Initialized successfully");
    } catch (e) {
      Logger.error("GmChatsService: Failed to initialize", error: e);
      errorMessage.value = e.toString();
    }
  }
  
  /// Stop the service and clear state
  Future<void> stop() async {
    Logger.info("GmChatsService: Stopping...");
    _syncTimer?.cancel();
    isInitialized.value = false;
    threads.clear();
    messages.clear();
    errorMessage.value = null;
  }
  
  /// Start the periodic sync timer
  void _startSyncTimer() {
    _syncTimer?.cancel();
    _syncTimer = Timer.periodic(const Duration(seconds: 30), (timer) {
      if (isInitialized.value && !isSyncing.value) {
        syncNow();
      }
    });
  }
  
  /// Trigger an immediate sync
  Future<void> syncNow() async {
    if (!isInitialized.value) return;
    if (isSyncing.value) return;
    
    try {
      isSyncing.value = true;
      errorMessage.value = null;
      
      Logger.debug("GmChatsService: Starting sync...");
      
      // TODO: Call Rust sync function once FRB bindings are regenerated
      // final result = await RustLib.instance.api.crateApiApiGmSyncNow();
      // if (result != null) {
      //   _updateThreadsFromRust(result.threads);
      // }
      
      lastSyncTime = DateTime.now();
      
      Logger.debug("GmChatsService: Sync completed at $lastSyncTime");
    } catch (e) {
      Logger.error("GmChatsService: Sync failed", error: e);
      errorMessage.value = e.toString();
      
      // Check if session expired
      if (e.toString().contains("401") || e.toString().contains("session")) {
        ss.settings.gmPairingState.value = GmPairingState.expired;
        await ss.settings.saveOne('gmPairingState');
      }
    } finally {
      isSyncing.value = false;
    }
  }
  
  /// Get messages for a specific thread
  Future<List<GmMessageData>> getMessages(String threadId) async {
    if (messages.containsKey(threadId)) {
      return messages[threadId]!;
    }
    
    // TODO: Fetch messages from Rust
    // final result = await RustLib.instance.api.crateApiApiGmGetMessages(threadId: threadId);
    // messages[threadId] = result;
    // return result;
    
    return [];
  }
  
  /// Send a text message
  Future<GmSendResultData?> sendMessage({
    required String threadId,
    required String text,
  }) async {
    if (!isInitialized.value) {
      Logger.warn("GmChatsService: Cannot send message - not initialized");
      return null;
    }
    
    try {
      Logger.info("GmChatsService: Sending message to $threadId");
      
      // TODO: Call Rust send function
      // final result = await RustLib.instance.api.crateApiApiGmSendText(
      //   threadId: threadId,
      //   text: text,
      // );
      // return result;
      
      return null;
    } catch (e) {
      Logger.error("GmChatsService: Failed to send message", error: e);
      return null;
    }
  }
  
  /// Mark a thread as read
  Future<void> markRead(String threadId) async {
    if (!isInitialized.value) return;
    
    try {
      // TODO: Call Rust mark read function
      // await RustLib.instance.api.crateApiApiGmMarkRead(threadId: threadId);
      
      // Update local state
      final index = threads.indexWhere((t) => t.threadId == threadId);
      if (index >= 0) {
        threads[index] = threads[index].copyWith(unreadCount: 0);
      }
    } catch (e) {
      Logger.error("GmChatsService: Failed to mark thread as read", error: e);
    }
  }
  
  /// Logout and clear all GM data
  Future<void> logout() async {
    Logger.info("GmChatsService: Logging out...");
    
    await stop();
    await GmSecureStorageService.clearSession();
    
    // TODO: Call Rust logout function
    // await RustLib.instance.api.crateApiApiGmLogout();
    
    ss.settings.gmPairingState.value = GmPairingState.notPaired;
    await ss.settings.saveOne('gmPairingState');
    
    Logger.info("GmChatsService: Logged out");
  }
}

/// Thread summary data for display
class GmThreadSummary {
  final String threadId;
  final String displayName;
  final String? avatarUrl;
  final String lastMessageText;
  final DateTime lastMessageTime;
  final int unreadCount;
  final bool isGroup;
  
  GmThreadSummary({
    required this.threadId,
    required this.displayName,
    this.avatarUrl,
    required this.lastMessageText,
    required this.lastMessageTime,
    required this.unreadCount,
    this.isGroup = false,
  });
  
  GmThreadSummary copyWith({
    String? threadId,
    String? displayName,
    String? avatarUrl,
    String? lastMessageText,
    DateTime? lastMessageTime,
    int? unreadCount,
    bool? isGroup,
  }) {
    return GmThreadSummary(
      threadId: threadId ?? this.threadId,
      displayName: displayName ?? this.displayName,
      avatarUrl: avatarUrl ?? this.avatarUrl,
      lastMessageText: lastMessageText ?? this.lastMessageText,
      lastMessageTime: lastMessageTime ?? this.lastMessageTime,
      unreadCount: unreadCount ?? this.unreadCount,
      isGroup: isGroup ?? this.isGroup,
    );
  }
}

/// Message data for display
class GmMessageData {
  final String messageId;
  final String threadId;
  final String text;
  final DateTime timestamp;
  final bool isFromMe;
  final String? senderName;
  final GmMessageStatusData status;
  
  GmMessageData({
    required this.messageId,
    required this.threadId,
    required this.text,
    required this.timestamp,
    required this.isFromMe,
    this.senderName,
    required this.status,
  });
}

/// Message status
enum GmMessageStatusData {
  sending,
  sent,
  delivered,
  read,
  failed,
}

/// Send result
class GmSendResultData {
  final bool success;
  final String? messageId;
  final String? error;
  
  GmSendResultData({
    required this.success,
    this.messageId,
    this.error,
  });
}

// Global accessor for convenience
GmChatsService get gmChats => Get.isRegistered<GmChatsService>() 
    ? Get.find<GmChatsService>() 
    : Get.put(GmChatsService());
