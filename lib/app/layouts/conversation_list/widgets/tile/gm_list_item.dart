import 'package:bluebubbles/helpers/helpers.dart';
import 'package:bluebubbles/services/services.dart';
import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:intl/intl.dart';

/// List item widget for Google Messages threads
/// Displays a GM thread in the conversation list with appropriate styling
class GmListItem extends StatelessWidget {
  final GmThreadSummary thread;
  final VoidCallback onTap;
  final VoidCallback? onLongPress;
  
  const GmListItem({
    super.key,
    required this.thread,
    required this.onTap,
    this.onLongPress,
  });
  
  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: onTap,
        onLongPress: onLongPress,
        child: Container(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
          decoration: BoxDecoration(
            border: Border(
              bottom: BorderSide(
                color: context.theme.dividerColor.withOpacity(0.3),
                width: 0.5,
              ),
            ),
          ),
          child: Row(
            children: [
              // Avatar
              _buildAvatar(context),
              const SizedBox(width: 12),
              
              // Content
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    // Title row (name + time)
                    Row(
                      children: [
                        // GM Badge
                        Container(
                          padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 1),
                          margin: const EdgeInsets.only(right: 6),
                          decoration: BoxDecoration(
                            color: Colors.green.withOpacity(0.2),
                            borderRadius: BorderRadius.circular(4),
                          ),
                          child: Text(
                            "GM",
                            style: TextStyle(
                              fontSize: 9,
                              fontWeight: FontWeight.w600,
                              color: Colors.green[700],
                            ),
                          ),
                        ),
                        
                        // Name
                        Expanded(
                          child: Text(
                            thread.displayName,
                            style: context.theme.textTheme.titleMedium?.copyWith(
                              fontWeight: thread.unreadCount > 0 
                                  ? FontWeight.bold 
                                  : FontWeight.normal,
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        
                        // Time
                        Text(
                          _formatTime(thread.lastMessageTime),
                          style: context.theme.textTheme.bodySmall?.copyWith(
                            color: thread.unreadCount > 0
                                ? context.theme.colorScheme.primary
                                : context.theme.colorScheme.onSurface.withOpacity(0.6),
                            fontWeight: thread.unreadCount > 0 
                                ? FontWeight.w600 
                                : FontWeight.normal,
                          ),
                        ),
                      ],
                    ),
                    
                    const SizedBox(height: 4),
                    
                    // Subtitle row (last message + unread badge)
                    Row(
                      children: [
                        // Group indicator
                        if (thread.isGroup) ...[
                          Icon(
                            Icons.group,
                            size: 14,
                            color: context.theme.colorScheme.onSurface.withOpacity(0.5),
                          ),
                          const SizedBox(width: 4),
                        ],
                        
                        // Last message preview
                        Expanded(
                          child: Text(
                            thread.lastMessageText,
                            style: context.theme.textTheme.bodyMedium?.copyWith(
                              color: thread.unreadCount > 0
                                  ? context.theme.colorScheme.onSurface
                                  : context.theme.colorScheme.onSurface.withOpacity(0.6),
                              fontWeight: thread.unreadCount > 0 
                                  ? FontWeight.w500 
                                  : FontWeight.normal,
                            ),
                            maxLines: 2,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                        
                        // Unread badge
                        if (thread.unreadCount > 0) ...[
                          const SizedBox(width: 8),
                          Container(
                            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                            decoration: BoxDecoration(
                              color: context.theme.colorScheme.primary,
                              borderRadius: BorderRadius.circular(10),
                            ),
                            child: Text(
                              thread.unreadCount > 99 ? "99+" : "${thread.unreadCount}",
                              style: TextStyle(
                                color: context.theme.colorScheme.onPrimary,
                                fontSize: 11,
                                fontWeight: FontWeight.bold,
                              ),
                            ),
                          ),
                        ],
                      ],
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
  
  Widget _buildAvatar(BuildContext context) {
    return Stack(
      children: [
        // Avatar circle
        CircleAvatar(
          radius: 26,
          backgroundColor: Colors.green.withOpacity(0.2),
          backgroundImage: thread.avatarUrl != null 
              ? NetworkImage(thread.avatarUrl!) 
              : null,
          child: thread.avatarUrl == null
              ? Text(
                  _getInitials(thread.displayName),
                  style: TextStyle(
                    color: Colors.green[700],
                    fontWeight: FontWeight.bold,
                    fontSize: 18,
                  ),
                )
              : null,
        ),
        
        // GM indicator
        Positioned(
          right: 0,
          bottom: 0,
          child: Container(
            width: 16,
            height: 16,
            decoration: BoxDecoration(
              color: Colors.green,
              shape: BoxShape.circle,
              border: Border.all(
                color: context.theme.colorScheme.surface,
                width: 2,
              ),
            ),
            child: const Icon(
              Icons.message,
              size: 8,
              color: Colors.white,
            ),
          ),
        ),
      ],
    );
  }
  
  String _getInitials(String name) {
    final parts = name.split(' ');
    if (parts.length >= 2) {
      return '${parts[0][0]}${parts[1][0]}'.toUpperCase();
    } else if (parts.isNotEmpty && parts[0].isNotEmpty) {
      return parts[0][0].toUpperCase();
    }
    return '?';
  }
  
  String _formatTime(DateTime time) {
    final now = DateTime.now();
    final diff = now.difference(time);
    
    if (diff.inDays == 0) {
      // Today - show time
      return DateFormat.jm().format(time);
    } else if (diff.inDays == 1) {
      return "Yesterday";
    } else if (diff.inDays < 7) {
      // This week - show day name
      return DateFormat.E().format(time);
    } else {
      // Older - show date
      return DateFormat.MMMd().format(time);
    }
  }
}
