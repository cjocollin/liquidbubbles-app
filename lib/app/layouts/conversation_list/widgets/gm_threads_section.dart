import 'package:bluebubbles/app/layouts/conversation_list/widgets/tile/gm_list_item.dart';
import 'package:bluebubbles/database/models.dart';
import 'package:bluebubbles/helpers/helpers.dart';
import 'package:bluebubbles/services/services.dart';
import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:universal_io/io.dart';

/// A section widget that displays Google Messages threads in the conversation list
/// This appears when GM is enabled and paired
class GmThreadsSection extends StatefulWidget {
  final bool collapsed;
  final VoidCallback? onToggleCollapse;
  
  const GmThreadsSection({
    super.key,
    this.collapsed = false,
    this.onToggleCollapse,
  });
  
  @override
  State<GmThreadsSection> createState() => _GmThreadsSectionState();
}

class _GmThreadsSectionState extends State<GmThreadsSection> {
  @override
  Widget build(BuildContext context) {
    // Only show on Android when GM is enabled and paired
    if (!Platform.isAndroid) return const SizedBox.shrink();
    
    return Obx(() {
      if (!ss.settings.enableGoogleMessages.value) {
        return const SizedBox.shrink();
      }
      
      if (ss.settings.gmPairingState.value != GmPairingState.paired) {
        return const SizedBox.shrink();
      }
      
      return Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          // Section header
          _buildHeader(context),
          
          // Thread list
          if (!widget.collapsed) _buildThreadList(context),
        ],
      );
    });
  }
  
  Widget _buildHeader(BuildContext context) {
    return InkWell(
      onTap: widget.onToggleCollapse,
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
        decoration: BoxDecoration(
          color: Colors.green.withOpacity(0.1),
          border: Border(
            bottom: BorderSide(
              color: Colors.green.withOpacity(0.3),
              width: 1,
            ),
          ),
        ),
        child: Row(
          children: [
            Icon(
              Icons.message,
              size: 18,
              color: Colors.green[700],
            ),
            const SizedBox(width: 8),
            Text(
              "Google Messages",
              style: context.theme.textTheme.titleSmall?.copyWith(
                fontWeight: FontWeight.w600,
                color: Colors.green[700],
              ),
            ),
            const SizedBox(width: 8),
            
            // Sync indicator
            Obx(() {
              if (gmChats.isSyncing.value) {
                return SizedBox(
                  width: 14,
                  height: 14,
                  child: CircularProgressIndicator(
                    strokeWidth: 2,
                    color: Colors.green[700],
                  ),
                );
              }
              return const SizedBox.shrink();
            }),
            
            const Spacer(),
            
            // Thread count
            Obx(() => Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
              decoration: BoxDecoration(
                color: Colors.green.withOpacity(0.2),
                borderRadius: BorderRadius.circular(12),
              ),
              child: Text(
                "${gmChats.threads.length}",
                style: TextStyle(
                  fontSize: 12,
                  fontWeight: FontWeight.w600,
                  color: Colors.green[700],
                ),
              ),
            )),
            
            const SizedBox(width: 8),
            
            // Collapse indicator
            Icon(
              widget.collapsed 
                  ? Icons.keyboard_arrow_down 
                  : Icons.keyboard_arrow_up,
              size: 20,
              color: Colors.green[700],
            ),
          ],
        ),
      ),
    );
  }
  
  Widget _buildThreadList(BuildContext context) {
    return Obx(() {
      final threads = gmChats.threads;
      
      if (!gmChats.isInitialized.value) {
        return _buildLoadingState(context);
      }
      
      if (gmChats.errorMessage.value != null) {
        return _buildErrorState(context, gmChats.errorMessage.value!);
      }
      
      if (threads.isEmpty) {
        return _buildEmptyState(context);
      }
      
      // Show first 5 threads by default, with "Show more" option
      final displayThreads = threads.take(5).toList();
      final hasMore = threads.length > 5;
      
      return Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          // Thread tiles
          ...displayThreads.map((thread) => GmListItem(
            thread: thread,
            onTap: () => _openThread(thread),
            onLongPress: () => _showThreadOptions(context, thread),
          )),
          
          // "Show more" button if needed
          if (hasMore)
            InkWell(
              onTap: () => _openGmThreadsList(context),
              child: Container(
                padding: const EdgeInsets.symmetric(vertical: 12),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Text(
                      "Show all ${threads.length} conversations",
                      style: TextStyle(
                        color: Colors.green[700],
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    const SizedBox(width: 4),
                    Icon(
                      Icons.arrow_forward_ios,
                      size: 12,
                      color: Colors.green[700],
                    ),
                  ],
                ),
              ),
            ),
          
          // Divider at bottom
          Container(
            height: 8,
            color: context.theme.dividerColor.withOpacity(0.1),
          ),
        ],
      );
    });
  }
  
  Widget _buildLoadingState(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(24),
      child: Column(
        children: [
          const CircularProgressIndicator(),
          const SizedBox(height: 12),
          Text(
            "Loading Google Messages...",
            style: context.theme.textTheme.bodyMedium?.copyWith(
              color: context.theme.colorScheme.onSurface.withOpacity(0.6),
            ),
          ),
        ],
      ),
    );
  }
  
  Widget _buildErrorState(BuildContext context, String error) {
    return Container(
      padding: const EdgeInsets.all(16),
      child: Column(
        children: [
          Icon(
            Icons.error_outline,
            color: Colors.red[400],
            size: 32,
          ),
          const SizedBox(height: 8),
          Text(
            "Failed to load",
            style: context.theme.textTheme.bodyMedium?.copyWith(
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            error,
            style: context.theme.textTheme.bodySmall?.copyWith(
              color: context.theme.colorScheme.onSurface.withOpacity(0.6),
            ),
            textAlign: TextAlign.center,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
          ),
          const SizedBox(height: 12),
          TextButton.icon(
            onPressed: () => gmChats.syncNow(),
            icon: const Icon(Icons.refresh),
            label: const Text("Retry"),
          ),
        ],
      ),
    );
  }
  
  Widget _buildEmptyState(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(24),
      child: Column(
        children: [
          Icon(
            Icons.message_outlined,
            color: context.theme.colorScheme.onSurface.withOpacity(0.3),
            size: 40,
          ),
          const SizedBox(height: 8),
          Text(
            "No Google Messages conversations",
            style: context.theme.textTheme.bodyMedium?.copyWith(
              color: context.theme.colorScheme.onSurface.withOpacity(0.6),
            ),
          ),
        ],
      ),
    );
  }
  
  void _openThread(GmThreadSummary thread) {
    // TODO: Navigate to GM conversation view
    showSnackbar("Coming Soon", "Opening ${thread.displayName}");
    
    // Mark as read
    gmChats.markRead(thread.threadId);
  }
  
  void _showThreadOptions(BuildContext context, GmThreadSummary thread) {
    showModalBottomSheet(
      context: context,
      builder: (context) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: const Icon(Icons.mark_email_read),
              title: const Text("Mark as read"),
              onTap: () {
                Navigator.pop(context);
                gmChats.markRead(thread.threadId);
              },
            ),
            ListTile(
              leading: const Icon(Icons.info_outline),
              title: const Text("Thread info"),
              onTap: () {
                Navigator.pop(context);
                // TODO: Show thread info
              },
            ),
          ],
        ),
      ),
    );
  }
  
  void _openGmThreadsList(BuildContext context) {
    // TODO: Navigate to full GM threads list
    showSnackbar("Coming Soon", "Full GM threads list");
  }
}
