import 'package:bluebubbles/database/models.dart';
import 'package:bluebubbles/services/services.dart';
import 'package:bluebubbles/utils/logger/logger.dart';

/// Message source types for routing
enum MessageSource {
  /// iMessage / SMS via BlueBubbles server or RustPush
  iMessage,
  /// Google Messages web integration
  googleMessages,
}

/// Utility class for routing messages based on their source
/// Google Messages chats use "gm-" prefix in their GUIDs
class GmMessageRouter {
  /// Prefix used for Google Messages chat GUIDs
  static const String gmGuidPrefix = "gm-";
  
  /// Determine the message source from a chat's GUID
  static MessageSource getSourceFromChat(Chat chat) {
    if (chat.guid.startsWith(gmGuidPrefix)) {
      return MessageSource.googleMessages;
    }
    return MessageSource.iMessage;
  }
  
  /// Determine the message source from a chat GUID string
  static MessageSource getSourceFromGuid(String guid) {
    if (guid.startsWith(gmGuidPrefix)) {
      return MessageSource.googleMessages;
    }
    return MessageSource.iMessage;
  }
  
  /// Check if a chat is a Google Messages chat
  static bool isGmChat(Chat chat) {
    return chat.guid.startsWith(gmGuidPrefix);
  }
  
  /// Check if a GUID is for a Google Messages chat
  static bool isGmGuid(String guid) {
    return guid.startsWith(gmGuidPrefix);
  }
  
  /// Send a message, routing to the appropriate backend
  static Future<void> sendMessage({
    required Chat chat,
    required Message message,
    Message? selected,
    String? reaction,
  }) async {
    final source = getSourceFromChat(chat);
    
    switch (source) {
      case MessageSource.iMessage:
        // Use the existing iMessage/BlueBubbles pipeline
        Logger.debug("GmMessageRouter: Routing message to iMessage backend");
        await ah.sendMessage(chat, message, selected, reaction);
        break;
        
      case MessageSource.googleMessages:
        // Use the Google Messages backend
        Logger.debug("GmMessageRouter: Routing message to Google Messages backend");
        await _sendGmMessage(chat, message);
        break;
    }
  }
  
  /// Send a message via Google Messages
  static Future<void> _sendGmMessage(Chat chat, Message message) async {
    try {
      // Extract thread ID from GUID (remove prefix)
      final threadId = chat.guid.substring(gmGuidPrefix.length);
      
      // Check if GM service is ready
      if (!gmChats.isInitialized.value) {
        throw Exception("Google Messages not initialized");
      }
      
      // Send via GM service
      final result = await gmChats.sendMessage(
        threadId: threadId,
        text: message.text ?? '',
      );
      
      if (result == null || !result.success) {
        // Mark message as failed
        message.error = MessageError.serverError.code;
        message.guid = message.guid;
        Logger.error("GmMessageRouter: GM send failed: ${result?.error}");
        throw Exception(result?.error ?? "Failed to send via Google Messages");
      }
      
      // Update message with result
      Logger.info("GmMessageRouter: GM message sent successfully: ${result.messageId}");
    } catch (e) {
      Logger.error("GmMessageRouter: Failed to send GM message", error: e);
      rethrow;
    }
  }
  
  /// Send an attachment, routing to the appropriate backend
  static Future<void> sendAttachment({
    required Chat chat,
    required Message message,
    bool isAudio = false,
  }) async {
    final source = getSourceFromChat(chat);
    
    switch (source) {
      case MessageSource.iMessage:
        // Use the existing iMessage/BlueBubbles pipeline
        Logger.debug("GmMessageRouter: Routing attachment to iMessage backend");
        await ah.sendAttachment(chat, message, isAudio);
        break;
        
      case MessageSource.googleMessages:
        // Google Messages attachment sending not yet implemented
        Logger.warn("GmMessageRouter: GM attachments not yet supported");
        throw UnimplementedError("Google Messages attachments not yet supported");
    }
  }
  
  /// Create a GM chat GUID from a thread ID
  static String createGmGuid(String threadId) {
    return "$gmGuidPrefix$threadId";
  }
  
  /// Extract thread ID from a GM chat GUID
  static String? extractThreadId(String guid) {
    if (!isGmGuid(guid)) return null;
    return guid.substring(gmGuidPrefix.length);
  }
}
