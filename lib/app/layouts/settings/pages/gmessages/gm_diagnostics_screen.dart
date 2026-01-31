import 'dart:convert';
import 'package:bluebubbles/app/layouts/settings/widgets/settings_widgets.dart';
import 'package:bluebubbles/app/wrappers/stateful_boilerplate.dart';
import 'package:bluebubbles/database/models.dart';
import 'package:bluebubbles/helpers/helpers.dart';
import 'package:bluebubbles/services/services.dart';
import 'package:bluebubbles/utils/logger/logger.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get/get.dart';
import 'package:intl/intl.dart';
import 'package:universal_io/io.dart';

/// Diagnostics screen for Google Messages integration
/// Shows current state, session info, and debugging tools
class GmDiagnosticsScreen extends StatefulWidget {
  const GmDiagnosticsScreen({super.key});

  @override
  State<GmDiagnosticsScreen> createState() => _GmDiagnosticsScreenState();
}

class _GmDiagnosticsScreenState extends OptimizedState<GmDiagnosticsScreen> {
  final RxBool _isLoading = true.obs;
  final RxBool _hasSession = false.obs;
  final RxnString _sessionInfo = RxnString(null);
  final RxList<_DiagnosticItem> _diagnostics = <_DiagnosticItem>[].obs;

  @override
  void initState() {
    super.initState();
    _loadDiagnostics();
  }

  Future<void> _loadDiagnostics() async {
    _isLoading.value = true;
    _diagnostics.clear();

    try {
      // Check platform
      _diagnostics.add(_DiagnosticItem(
        name: "Platform",
        value: Platform.isAndroid ? "Android" : Platform.operatingSystem,
        status: Platform.isAndroid ? _DiagStatus.ok : _DiagStatus.error,
        detail: Platform.isAndroid ? null : "Google Messages only works on Android",
      ));

      // Check feature flag
      _diagnostics.add(_DiagnosticItem(
        name: "Feature Enabled",
        value: ss.settings.enableGoogleMessages.value ? "Yes" : "No",
        status: ss.settings.enableGoogleMessages.value ? _DiagStatus.ok : _DiagStatus.warning,
      ));

      // Check pairing state
      final pairingState = ss.settings.gmPairingState.value;
      _diagnostics.add(_DiagnosticItem(
        name: "Pairing State",
        value: _pairingStateToString(pairingState),
        status: _pairingStateToStatus(pairingState),
        detail: _pairingStateDetail(pairingState),
      ));

      // Check secure storage
      if (Platform.isAndroid) {
        _hasSession.value = await GmSecureStorageService.hasSession();
        _diagnostics.add(_DiagnosticItem(
          name: "Session Stored",
          value: _hasSession.value ? "Yes" : "No",
          status: _hasSession.value ? _DiagStatus.ok : _DiagStatus.warning,
        ));

        // Load session info if available
        if (_hasSession.value) {
          final sessionData = await GmSecureStorageService.loadSession();
          if (sessionData != null) {
            try {
              final sessionJson = utf8.decode(sessionData);
              final session = jsonDecode(sessionJson) as Map<String, dynamic>;
              _sessionInfo.value = "Paired: ${session['paired_at'] ?? 'Unknown'}";
              
              _diagnostics.add(_DiagnosticItem(
                name: "Session Size",
                value: "${sessionData.length} bytes",
                status: _DiagStatus.ok,
              ));
              
              _diagnostics.add(_DiagnosticItem(
                name: "Paired At",
                value: session['paired_at'] ?? "Unknown",
                status: _DiagStatus.ok,
              ));
            } catch (e) {
              _diagnostics.add(_DiagnosticItem(
                name: "Session Parse",
                value: "Error",
                status: _DiagStatus.error,
                detail: e.toString(),
              ));
            }
          }
        }
      }

      // Check GM service state
      _diagnostics.add(_DiagnosticItem(
        name: "Service Initialized",
        value: gmChats.isInitialized.value ? "Yes" : "No",
        status: gmChats.isInitialized.value ? _DiagStatus.ok : _DiagStatus.warning,
      ));

      _diagnostics.add(_DiagnosticItem(
        name: "Sync In Progress",
        value: gmChats.isSyncing.value ? "Yes" : "No",
        status: _DiagStatus.ok,
      ));

      _diagnostics.add(_DiagnosticItem(
        name: "Thread Count",
        value: "${gmChats.threads.length}",
        status: _DiagStatus.ok,
      ));

      if (gmChats.lastSyncTime != null) {
        _diagnostics.add(_DiagnosticItem(
          name: "Last Sync",
          value: DateFormat.yMd().add_jms().format(gmChats.lastSyncTime!),
          status: _DiagStatus.ok,
        ));
      }

      if (gmChats.errorMessage.value != null) {
        _diagnostics.add(_DiagnosticItem(
          name: "Last Error",
          value: gmChats.errorMessage.value!,
          status: _DiagStatus.error,
        ));
      }
    } catch (e) {
      Logger.error("Failed to load diagnostics", error: e);
      _diagnostics.add(_DiagnosticItem(
        name: "Diagnostics Error",
        value: e.toString(),
        status: _DiagStatus.error,
      ));
    } finally {
      _isLoading.value = false;
    }
  }

  String _pairingStateToString(GmPairingState state) {
    switch (state) {
      case GmPairingState.notPaired:
        return "Not Paired";
      case GmPairingState.waitingForPairing:
        return "Waiting for Pairing";
      case GmPairingState.paired:
        return "Paired";
      case GmPairingState.expired:
        return "Session Expired";
      case GmPairingState.error:
        return "Error";
    }
  }

  _DiagStatus _pairingStateToStatus(GmPairingState state) {
    switch (state) {
      case GmPairingState.notPaired:
        return _DiagStatus.warning;
      case GmPairingState.waitingForPairing:
        return _DiagStatus.warning;
      case GmPairingState.paired:
        return _DiagStatus.ok;
      case GmPairingState.expired:
        return _DiagStatus.error;
      case GmPairingState.error:
        return _DiagStatus.error;
    }
  }

  String? _pairingStateDetail(GmPairingState state) {
    switch (state) {
      case GmPairingState.notPaired:
        return "Go to Settings > Google Messages to pair";
      case GmPairingState.waitingForPairing:
        return "Scan the QR code with Google Messages";
      case GmPairingState.paired:
        return null;
      case GmPairingState.expired:
        return "Session expired, re-pair required";
      case GmPairingState.error:
        return "An error occurred during pairing";
    }
  }

  @override
  Widget build(BuildContext context) {
    return SettingsScaffold(
      title: "GM Diagnostics",
      initialHeader: "Status",
      iosSubtitle: iosSubtitle,
      materialSubtitle: materialSubtitle,
      tileColor: tileColor,
      headerColor: headerColor,
      bodySlivers: [
        SliverList(
          delegate: SliverChildListDelegate([
            // Refresh button
            Padding(
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  Expanded(
                    child: ElevatedButton.icon(
                      onPressed: () => _loadDiagnostics(),
                      icon: const Icon(Icons.refresh),
                      label: const Text("Refresh Diagnostics"),
                    ),
                  ),
                  const SizedBox(width: 8),
                  ElevatedButton.icon(
                    onPressed: () => _copyDiagnostics(),
                    icon: const Icon(Icons.copy),
                    label: const Text("Copy"),
                    style: ElevatedButton.styleFrom(
                      backgroundColor: context.theme.colorScheme.secondaryContainer,
                      foregroundColor: context.theme.colorScheme.onSecondaryContainer,
                    ),
                  ),
                ],
              ),
            ),

            // Loading indicator
            Obx(() {
              if (_isLoading.value) {
                return const Padding(
                  padding: EdgeInsets.all(24),
                  child: Center(child: CircularProgressIndicator()),
                );
              }
              return const SizedBox.shrink();
            }),

            // Diagnostics list
            Obx(() {
              if (_isLoading.value) return const SizedBox.shrink();

              return SettingsSection(
                backgroundColor: tileColor,
                children: _diagnostics.map((item) => _buildDiagnosticTile(item)).toList(),
              );
            }),

            // Actions section
            SettingsHeader(
              iosSubtitle: iosSubtitle,
              materialSubtitle: materialSubtitle,
              text: "Actions",
            ),
            SettingsSection(
              backgroundColor: tileColor,
              children: [
                SettingsTile(
                  backgroundColor: tileColor,
                  title: "Force Sync",
                  subtitle: "Trigger an immediate sync",
                  leading: SettingsLeadingIcon(
                    iosIcon: CupertinoIcons.arrow_2_circlepath,
                    materialIcon: Icons.sync,
                    containerColor: Colors.blue,
                  ),
                  onTap: () async {
                    await gmChats.syncNow();
                    await _loadDiagnostics();
                    showSnackbar("Done", "Sync triggered");
                  },
                ),
                const SettingsDivider(),
                SettingsTile(
                  backgroundColor: tileColor,
                  title: "Reinitialize Service",
                  subtitle: "Restart the GM service",
                  leading: SettingsLeadingIcon(
                    iosIcon: CupertinoIcons.power,
                    materialIcon: Icons.restart_alt,
                    containerColor: Colors.orange,
                  ),
                  onTap: () async {
                    await gmChats.stop();
                    await gmChats.initialize();
                    await _loadDiagnostics();
                    showSnackbar("Done", "Service reinitialized");
                  },
                ),
                const SettingsDivider(),
                SettingsTile(
                  backgroundColor: tileColor,
                  title: "Clear Session",
                  subtitle: "Remove stored session data (requires re-pairing)",
                  leading: SettingsLeadingIcon(
                    iosIcon: CupertinoIcons.trash,
                    materialIcon: Icons.delete_outline,
                    containerColor: Colors.red,
                  ),
                  onTap: () => _confirmClearSession(),
                ),
              ],
            ),

            const SizedBox(height: 50),
          ]),
        ),
      ],
    );
  }

  Widget _buildDiagnosticTile(_DiagnosticItem item) {
    return ListTile(
      leading: Icon(
        item.status == _DiagStatus.ok
            ? Icons.check_circle
            : item.status == _DiagStatus.warning
                ? Icons.warning
                : Icons.error,
        color: item.status == _DiagStatus.ok
            ? Colors.green
            : item.status == _DiagStatus.warning
                ? Colors.orange
                : Colors.red,
      ),
      title: Text(item.name),
      subtitle: item.detail != null
          ? Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  item.value,
                  style: const TextStyle(fontWeight: FontWeight.bold),
                ),
                Text(
                  item.detail!,
                  style: TextStyle(
                    fontSize: 12,
                    color: context.theme.colorScheme.onSurface.withOpacity(0.6),
                  ),
                ),
              ],
            )
          : Text(
              item.value,
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
      dense: true,
    );
  }

  void _copyDiagnostics() {
    final buffer = StringBuffer();
    buffer.writeln("=== Google Messages Diagnostics ===");
    buffer.writeln("Generated: ${DateTime.now().toIso8601String()}");
    buffer.writeln("");
    
    for (final item in _diagnostics) {
      buffer.writeln("${item.name}: ${item.value}");
      if (item.detail != null) {
        buffer.writeln("  Detail: ${item.detail}");
      }
    }
    
    Clipboard.setData(ClipboardData(text: buffer.toString()));
    showSnackbar("Copied", "Diagnostics copied to clipboard");
  }

  void _confirmClearSession() {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text("Clear Session?"),
        content: const Text(
          "This will remove the stored session data. You will need to re-pair with Google Messages.",
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text("Cancel"),
          ),
          TextButton(
            onPressed: () async {
              Navigator.pop(context);
              await GmSecureStorageService.clearSession();
              ss.settings.gmPairingState.value = GmPairingState.notPaired;
              await ss.settings.saveOne('gmPairingState');
              await gmChats.stop();
              await _loadDiagnostics();
              showSnackbar("Done", "Session cleared");
            },
            child: Text(
              "Clear",
              style: TextStyle(color: Colors.red[700]),
            ),
          ),
        ],
      ),
    );
  }
}

enum _DiagStatus { ok, warning, error }

class _DiagnosticItem {
  final String name;
  final String value;
  final _DiagStatus status;
  final String? detail;

  _DiagnosticItem({
    required this.name,
    required this.value,
    required this.status,
    this.detail,
  });
}
