import 'package:bluebubbles/app/layouts/settings/pages/gmessages/gm_diagnostics_screen.dart';
import 'package:bluebubbles/app/layouts/settings/pages/gmessages/gm_pairing_webview.dart';
import 'package:bluebubbles/app/layouts/settings/widgets/settings_widgets.dart';
import 'package:bluebubbles/app/wrappers/stateful_boilerplate.dart';
import 'package:bluebubbles/database/models.dart';
import 'package:bluebubbles/services/services.dart';
import 'package:bluebubbles/helpers/helpers.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:get/get.dart';
import 'package:universal_io/io.dart';

/// Google Messages settings panel (Experimental feature)
class GoogleMessagesPanel extends StatefulWidget {
  const GoogleMessagesPanel({super.key});

  @override
  State<GoogleMessagesPanel> createState() => _GoogleMessagesPanelState();
}

class _GoogleMessagesPanelState extends OptimizedState<GoogleMessagesPanel> {
  @override
  Widget build(BuildContext context) {
    // Only show on Android
    if (!Platform.isAndroid) {
      return SettingsScaffold(
        title: "Google Messages",
        initialHeader: "Unavailable",
        iosSubtitle: iosSubtitle,
        materialSubtitle: materialSubtitle,
        tileColor: tileColor,
        headerColor: headerColor,
        bodySlivers: [
          SliverList(
            delegate: SliverChildListDelegate([
              Padding(
                padding: const EdgeInsets.all(16.0),
                child: Text(
                  "Google Messages integration is only available on Android.",
                  style: context.theme.textTheme.bodyLarge,
                ),
              ),
            ]),
          ),
        ],
      );
    }

    return SettingsScaffold(
      title: "Google Messages",
      initialHeader: "Experimental Feature",
      iosSubtitle: iosSubtitle,
      materialSubtitle: materialSubtitle,
      tileColor: tileColor,
      headerColor: headerColor,
      bodySlivers: [
        SliverList(
          delegate: SliverChildListDelegate([
            // Warning banner
            Container(
              color: Colors.orange.withOpacity(0.1),
              padding: const EdgeInsets.all(16),
              child: Row(
                children: [
                  Icon(Icons.warning_amber_rounded, color: Colors.orange[700]),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Text(
                      "This is an experimental feature. Google Messages sessions can expire and require re-pairing.",
                      style: context.theme.textTheme.bodyMedium?.copyWith(
                        color: Colors.orange[900],
                      ),
                    ),
                  ),
                ],
              ),
            ),
            
            SettingsSection(
              backgroundColor: tileColor,
              children: [
                // Enable toggle
                Obx(() => SettingsSwitch(
                  onChanged: (bool val) async {
                    ss.settings.enableGoogleMessages.value = val;
                    await ss.settings.saveOne('enableGoogleMessages');
                    
                    if (val) {
                      // TODO: Initialize GM service
                      // await gmService.initialize();
                    } else {
                      // TODO: Stop GM service
                      // await gmService.stop();
                    }
                  },
                  initialVal: ss.settings.enableGoogleMessages.value,
                  title: "Enable Google Messages",
                  subtitle: "Access your Google Messages conversations",
                  backgroundColor: tileColor,
                  leading: SettingsLeadingIcon(
                    iosIcon: CupertinoIcons.chat_bubble_2_fill,
                    materialIcon: Icons.message,
                    containerColor: ss.settings.enableGoogleMessages.value 
                        ? Colors.green 
                        : Colors.grey,
                  ),
                )),
                
                // Pairing status (only show when enabled)
                Obx(() {
                  if (!ss.settings.enableGoogleMessages.value) {
                    return const SizedBox.shrink();
                  }
                  
                  return Column(
                    children: [
                      const SettingsDivider(),
                      SettingsTile(
                        backgroundColor: tileColor,
                        title: "Pairing Status",
                        subtitle: _getPairingStateText(ss.settings.gmPairingState.value),
                        leading: SettingsLeadingIcon(
                          iosIcon: _getPairingStateIcon(ss.settings.gmPairingState.value),
                          materialIcon: _getPairingStateIcon(ss.settings.gmPairingState.value),
                          containerColor: _getPairingStateColor(ss.settings.gmPairingState.value),
                        ),
                        trailing: _buildPairingAction(),
                        onTap: () => _handlePairingTap(),
                      ),
                    ],
                  );
                }),
              ],
            ),
            
            // Instructions section (only show when enabled)
            Obx(() {
              if (!ss.settings.enableGoogleMessages.value) {
                return const SizedBox.shrink();
              }
              
              return Column(
                children: [
                  SettingsHeader(
                    iosSubtitle: iosSubtitle,
                    materialSubtitle: materialSubtitle,
                    text: "How to Pair",
                  ),
                  SettingsSection(
                    backgroundColor: tileColor,
                    children: [
                      Padding(
                        padding: const EdgeInsets.all(16),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            _buildStep(1, "Open Google Messages on your phone"),
                            const SizedBox(height: 12),
                            _buildStep(2, "Tap the three dots menu → Device pairing"),
                            const SizedBox(height: 12),
                            _buildStep(3, "Tap 'Pair with QR code scanner' in this app"),
                            const SizedBox(height: 12),
                            _buildStep(4, "Scan the QR code shown in Google Messages"),
                          ],
                        ),
                      ),
                    ],
                  ),
                ],
              );
            }),
            
            // Limitations section
            Obx(() {
              if (!ss.settings.enableGoogleMessages.value) {
                return const SizedBox.shrink();
              }
              
              return Column(
                children: [
                  SettingsHeader(
                    iosSubtitle: iosSubtitle,
                    materialSubtitle: materialSubtitle,
                    text: "Limitations",
                  ),
                  SettingsSection(
                    backgroundColor: tileColor,
                    children: [
                      Padding(
                        padding: const EdgeInsets.all(16),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            _buildLimitation("Text messages only (no MMS/images yet)"),
                            const SizedBox(height: 8),
                            _buildLimitation("Session may expire after some time"),
                            const SizedBox(height: 8),
                            _buildLimitation("Requires internet connection"),
                            const SizedBox(height: 8),
                            _buildLimitation("Some features may not work with all carriers"),
                          ],
                        ),
                      ),
                    ],
                  ),
                ],
              );
            }),
            
            // Advanced section (diagnostics)
            Obx(() {
              if (!ss.settings.enableGoogleMessages.value) {
                return const SizedBox.shrink();
              }
              
              return Column(
                children: [
                  SettingsHeader(
                    iosSubtitle: iosSubtitle,
                    materialSubtitle: materialSubtitle,
                    text: "Advanced",
                  ),
                  SettingsSection(
                    backgroundColor: tileColor,
                    children: [
                      SettingsTile(
                        backgroundColor: tileColor,
                        title: "Diagnostics",
                        subtitle: "View connection status and debug info",
                        leading: SettingsLeadingIcon(
                          iosIcon: CupertinoIcons.wrench_fill,
                          materialIcon: Icons.bug_report,
                          containerColor: Colors.purple,
                        ),
                        onTap: () => Navigator.of(context).push(
                          MaterialPageRoute(
                            builder: (context) => const GmDiagnosticsScreen(),
                          ),
                        ),
                      ),
                    ],
                  ),
                ],
              );
            }),
            
            // Logout button (only show when paired)
            Obx(() {
              if (!ss.settings.enableGoogleMessages.value ||
                  ss.settings.gmPairingState.value != GmPairingState.paired) {
                return const SizedBox.shrink();
              }
              
              return Column(
                children: [
                  SettingsHeader(
                    iosSubtitle: iosSubtitle,
                    materialSubtitle: materialSubtitle,
                    text: "Danger Zone",
                  ),
                  SettingsSection(
                    backgroundColor: tileColor,
                    children: [
                      SettingsTile(
                        backgroundColor: tileColor,
                        title: "Disconnect Google Messages",
                        subtitle: "Remove pairing and clear cached data",
                        leading: SettingsLeadingIcon(
                          iosIcon: CupertinoIcons.xmark_circle_fill,
                          materialIcon: Icons.logout,
                          containerColor: Colors.red,
                        ),
                        onTap: () => _confirmLogout(),
                      ),
                    ],
                  ),
                ],
              );
            }),
            
            const SizedBox(height: 50),
          ]),
        ),
      ],
    );
  }

  Widget _buildStep(int number, String text) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          width: 24,
          height: 24,
          decoration: BoxDecoration(
            color: context.theme.colorScheme.primary,
            shape: BoxShape.circle,
          ),
          child: Center(
            child: Text(
              "$number",
              style: TextStyle(
                color: context.theme.colorScheme.onPrimary,
                fontWeight: FontWeight.bold,
                fontSize: 12,
              ),
            ),
          ),
        ),
        const SizedBox(width: 12),
        Expanded(
          child: Text(
            text,
            style: context.theme.textTheme.bodyMedium,
          ),
        ),
      ],
    );
  }

  Widget _buildLimitation(String text) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(
          Icons.info_outline,
          size: 16,
          color: context.theme.colorScheme.outline,
        ),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            text,
            style: context.theme.textTheme.bodySmall?.copyWith(
              color: context.theme.colorScheme.outline,
            ),
          ),
        ),
      ],
    );
  }

  String _getPairingStateText(GmPairingState state) {
    switch (state) {
      case GmPairingState.notPaired:
        return "Not paired";
      case GmPairingState.waitingForPairing:
        return "Waiting for pairing...";
      case GmPairingState.paired:
        return "Paired and connected";
      case GmPairingState.expired:
        return "Session expired - re-pair required";
      case GmPairingState.error:
        return "Error occurred";
    }
  }

  IconData _getPairingStateIcon(GmPairingState state) {
    switch (state) {
      case GmPairingState.notPaired:
        return Icons.link_off;
      case GmPairingState.waitingForPairing:
        return Icons.hourglass_empty;
      case GmPairingState.paired:
        return Icons.check_circle;
      case GmPairingState.expired:
        return Icons.refresh;
      case GmPairingState.error:
        return Icons.error;
    }
  }

  Color _getPairingStateColor(GmPairingState state) {
    switch (state) {
      case GmPairingState.notPaired:
        return Colors.grey;
      case GmPairingState.waitingForPairing:
        return Colors.orange;
      case GmPairingState.paired:
        return Colors.green;
      case GmPairingState.expired:
        return Colors.orange;
      case GmPairingState.error:
        return Colors.red;
    }
  }

  Widget _buildPairingAction() {
    final state = ss.settings.gmPairingState.value;
    
    switch (state) {
      case GmPairingState.notPaired:
      case GmPairingState.expired:
        return TextButton(
          onPressed: () => _startPairing(),
          child: const Text("Pair"),
        );
      case GmPairingState.waitingForPairing:
        return const SizedBox(
          width: 20,
          height: 20,
          child: CircularProgressIndicator(strokeWidth: 2),
        );
      case GmPairingState.paired:
        return const Icon(Icons.check, color: Colors.green);
      case GmPairingState.error:
        return TextButton(
          onPressed: () => _startPairing(),
          child: const Text("Retry"),
        );
    }
  }

  void _handlePairingTap() {
    final state = ss.settings.gmPairingState.value;
    
    if (state == GmPairingState.notPaired ||
        state == GmPairingState.expired ||
        state == GmPairingState.error) {
      _startPairing();
    }
  }

  void _startPairing() {
    // Navigate to WebView pairing screen
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (context) => const GmPairingWebView(),
      ),
    );
  }

  void _confirmLogout() {
    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text("Disconnect Google Messages?"),
        content: const Text(
          "This will remove the pairing and clear all cached Google Messages data. "
          "Your existing OpenBubbles/iMessage data will not be affected.",
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text("Cancel"),
          ),
          TextButton(
            onPressed: () async {
              Navigator.of(context).pop();
              await _performLogout();
            },
            child: Text(
              "Disconnect",
              style: TextStyle(color: Colors.red[700]),
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _performLogout() async {
    try {
      // Clear secure storage
      await GmSecureStorageService.clearSession();
      
      // TODO: Call Rust gm_logout once FRB bindings are regenerated
      // await RustLib.instance.api.crateApiApiGmLogout();
      
      ss.settings.gmPairingState.value = GmPairingState.notPaired;
      await ss.settings.saveOne('gmPairingState');
      
      showSnackbar("Success", "Google Messages disconnected");
    } catch (e) {
      showSnackbar("Error", "Failed to disconnect: $e");
    }
  }
}

