import 'dart:io';

import 'package:bluebubbles/app/layouts/settings/widgets/settings_widgets.dart';
import 'package:bluebubbles/app/wrappers/stateful_boilerplate.dart';
import 'package:bluebubbles/helpers/types/constants.dart';
import 'package:bluebubbles/services/services.dart';
import 'package:bluebubbles/utils/logger/logger.dart';
import 'package:flutter/cupertino.dart';
import 'package:flutter/material.dart';
import 'package:get/get.dart';

/// GoogleMessagesPanel provides settings for the experimental Google Messages integration.
/// 
/// This panel allows users to:
/// - Enable/disable the Google Messages feature
/// - Initiate WebView pairing with Google Messages web
/// - View pairing status
/// - Logout from Google Messages
/// 
/// Note: This feature is Android-only and experimental.
class GoogleMessagesPanel extends StatefulWidget {
  const GoogleMessagesPanel({super.key});

  @override
  State<StatefulWidget> createState() => _GoogleMessagesPanelState();
}

class _GoogleMessagesPanelState extends OptimizedState<GoogleMessagesPanel> {
  bool _isLoading = false;
  String _statusMessage = '';
  
  @override
  void initState() {
    super.initState();
    _updateStatusMessage();
  }
  
  void _updateStatusMessage() {
    switch (gms.pairingState.value) {
      case GMPairingState.notPaired:
        _statusMessage = 'Not paired';
        break;
      case GMPairingState.waitingForPairing:
        _statusMessage = 'Waiting for pairing...';
        break;
      case GMPairingState.paired:
        _statusMessage = 'Paired and active';
        break;
      case GMPairingState.expired:
        _statusMessage = 'Session expired - please re-pair';
        break;
      case GMPairingState.error:
        _statusMessage = 'Error - ${gms.lastError.value}';
        break;
    }
  }
  
  Future<void> _startPairing() async {
    setState(() {
      _isLoading = true;
      _statusMessage = 'Starting pairing...';
    });
    
    try {
      final result = await gms.startPairing();
      Logger.info('[GoogleMessagesPanel] Pairing result: $result');
      
      setState(() {
        _isLoading = false;
        _updateStatusMessage();
      });
      
      if (result['status'] == 'paired') {
        if (mounted) {
          showSnackbar('Success', 'Google Messages paired successfully!');
        }
      } else if (result['status'] == 'cancelled') {
        if (mounted) {
          showSnackbar('Cancelled', 'Pairing was cancelled');
        }
      } else if (result['status'] == 'error') {
        if (mounted) {
          showSnackbar('Error', result['message'] ?? 'Unknown error');
        }
      }
    } catch (e) {
      Logger.error('[GoogleMessagesPanel] Pairing error: $e');
      setState(() {
        _isLoading = false;
        _statusMessage = 'Error: $e';
      });
      if (mounted) {
        showSnackbar('Error', 'Failed to start pairing: $e');
      }
    }
  }
  
  Future<void> _logout() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        backgroundColor: context.theme.colorScheme.properSurface,
        title: Text(
          'Disconnect Google Messages?',
          style: context.theme.textTheme.titleLarge,
        ),
        content: Text(
          'This will disconnect your Google Messages account. You can re-pair at any time.',
          style: context.theme.textTheme.bodyLarge,
        ),
        actions: [
          TextButton(
            child: Text(
              'Cancel',
              style: context.theme.textTheme.bodyLarge!.copyWith(
                color: context.theme.colorScheme.primary,
              ),
            ),
            onPressed: () => Navigator.of(context).pop(false),
          ),
          TextButton(
            child: Text(
              'Disconnect',
              style: context.theme.textTheme.bodyLarge!.copyWith(
                color: Colors.red,
              ),
            ),
            onPressed: () => Navigator.of(context).pop(true),
          ),
        ],
      ),
    );
    
    if (confirmed == true) {
      setState(() {
        _isLoading = true;
        _statusMessage = 'Disconnecting...';
      });
      
      try {
        await gms.logout();
        setState(() {
          _isLoading = false;
          _updateStatusMessage();
        });
        if (mounted) {
          showSnackbar('Success', 'Google Messages disconnected');
        }
      } catch (e) {
        setState(() {
          _isLoading = false;
          _statusMessage = 'Error: $e';
        });
        if (mounted) {
          showSnackbar('Error', 'Failed to disconnect: $e');
        }
      }
    }
  }

  Color _getStatusColor() {
    switch (gms.pairingState.value) {
      case GMPairingState.notPaired:
        return Colors.grey;
      case GMPairingState.waitingForPairing:
        return Colors.orange;
      case GMPairingState.paired:
        return Colors.green;
      case GMPairingState.expired:
        return Colors.orange;
      case GMPairingState.error:
        return Colors.red;
    }
  }

  @override
  Widget build(BuildContext context) {
    // Only available on Android
    if (!Platform.isAndroid) {
      return SettingsScaffold(
        title: "Google Messages",
        initialHeader: null,
        iosSubtitle: iosSubtitle,
        materialSubtitle: materialSubtitle,
        tileColor: tileColor,
        headerColor: headerColor,
        bodySlivers: [
          SliverList(
            delegate: SliverChildListDelegate([
              SettingsSection(
                backgroundColor: tileColor,
                children: [
                  Padding(
                    padding: const EdgeInsets.all(16.0),
                    child: Text(
                      'Google Messages integration is only available on Android.',
                      style: context.theme.textTheme.bodyLarge,
                    ),
                  ),
                ],
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
          delegate: SliverChildListDelegate(
            <Widget>[
              SettingsSection(
                backgroundColor: tileColor,
                children: [
                  Container(
                    padding: const EdgeInsets.all(16),
                    child: Row(
                      children: [
                        Icon(
                          Icons.science,
                          color: Colors.orange,
                          size: 28,
                        ),
                        const SizedBox(width: 12),
                        Expanded(
                          child: Text(
                            'This feature is experimental and may not work perfectly. '
                            'Google Messages integration allows you to send and receive '
                            'SMS/RCS messages through Google Messages web.',
                            style: context.theme.textTheme.bodyMedium,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
              SettingsHeader(
                iosSubtitle: iosSubtitle,
                materialSubtitle: materialSubtitle,
                text: "Feature Toggle",
              ),
              SettingsSection(
                backgroundColor: tileColor,
                children: [
                  Obx(() => SettingsSwitch(
                    onChanged: (bool val) async {
                      ss.settings.enableGoogleMessages.value = val;
                      ss.saveSettings();
                      
                      if (val) {
                        // Initialize the GM service when enabled
                        await gms.init();
                      }
                      
                      setState(() {
                        _updateStatusMessage();
                      });
                    },
                    initialVal: ss.settings.enableGoogleMessages.value,
                    title: "Enable Google Messages",
                    subtitle: "Enable experimental Google Messages integration",
                    backgroundColor: tileColor,
                  )),
                ],
              ),
              // Only show pairing section if feature is enabled
              Obx(() {
                if (!ss.settings.enableGoogleMessages.value) {
                  return const SizedBox.shrink();
                }
                
                return Column(
                  children: [
                    SettingsHeader(
                      iosSubtitle: iosSubtitle,
                      materialSubtitle: materialSubtitle,
                      text: "Connection Status",
                    ),
                    SettingsSection(
                      backgroundColor: tileColor,
                      children: [
                        Obx(() {
                          _updateStatusMessage();
                          return SettingsTile(
                            backgroundColor: tileColor,
                            title: "Status",
                            subtitle: _statusMessage,
                            leading: Container(
                              width: 12,
                              height: 12,
                              decoration: BoxDecoration(
                                shape: BoxShape.circle,
                                color: _getStatusColor(),
                              ),
                            ),
                          );
                        }),
                      ],
                    ),
                    SettingsHeader(
                      iosSubtitle: iosSubtitle,
                      materialSubtitle: materialSubtitle,
                      text: "Actions",
                    ),
                    SettingsSection(
                      backgroundColor: tileColor,
                      children: [
                        Obx(() {
                          final isPaired = gms.pairingState.value == GMPairingState.paired;
                          final isWaiting = gms.pairingState.value == GMPairingState.waitingForPairing;
                          
                          if (isPaired) {
                            return Column(
                              children: [
                                SettingsTile(
                                  backgroundColor: tileColor,
                                  title: "Connected",
                                  subtitle: "Google Messages is paired and active",
                                  leading: const SettingsLeadingIcon(
                                    iosIcon: CupertinoIcons.checkmark_circle_fill,
                                    materialIcon: Icons.check_circle,
                                    containerColor: Colors.green,
                                  ),
                                ),
                                const SettingsDivider(),
                                SettingsTile(
                                  backgroundColor: tileColor,
                                  title: "Disconnect",
                                  subtitle: "Disconnect from Google Messages",
                                  onTap: _isLoading ? null : _logout,
                                  leading: SettingsLeadingIcon(
                                    iosIcon: CupertinoIcons.xmark_circle_fill,
                                    materialIcon: Icons.logout,
                                    containerColor: Colors.red[600],
                                  ),
                                ),
                              ],
                            );
                          }
                          
                          return SettingsTile(
                            backgroundColor: tileColor,
                            title: isWaiting ? "Pairing in progress..." : "Pair with Google Messages",
                            subtitle: isWaiting 
                                ? "Scan the QR code in the popup" 
                                : "Opens a WebView to scan QR code with your phone",
                            onTap: (_isLoading || isWaiting) ? null : _startPairing,
                            leading: _isLoading || isWaiting
                                ? const SizedBox(
                                    width: 30,
                                    height: 30,
                                    child: Padding(
                                      padding: EdgeInsets.all(4.0),
                                      child: CircularProgressIndicator(strokeWidth: 2),
                                    ),
                                  )
                                : const SettingsLeadingIcon(
                                    iosIcon: CupertinoIcons.qrcode,
                                    materialIcon: Icons.qr_code_scanner,
                                    containerColor: Colors.blueAccent,
                                  ),
                          );
                        }),
                      ],
                    ),
                    SettingsHeader(
                      iosSubtitle: iosSubtitle,
                      materialSubtitle: materialSubtitle,
                      text: "How It Works",
                    ),
                    SettingsSection(
                      backgroundColor: tileColor,
                      children: [
                        Padding(
                          padding: const EdgeInsets.all(16),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              _buildStep(
                                context,
                                1,
                                'Enable the feature above',
                                Icons.toggle_on,
                              ),
                              const SizedBox(height: 12),
                              _buildStep(
                                context,
                                2,
                                'Tap "Pair with Google Messages"',
                                Icons.touch_app,
                              ),
                              const SizedBox(height: 12),
                              _buildStep(
                                context,
                                3,
                                'Open Google Messages on your phone',
                                Icons.phone_android,
                              ),
                              const SizedBox(height: 12),
                              _buildStep(
                                context,
                                4,
                                'Go to Settings > Device Pairing',
                                Icons.settings,
                              ),
                              const SizedBox(height: 12),
                              _buildStep(
                                context,
                                5,
                                'Scan the QR code shown in the app',
                                Icons.qr_code_scanner,
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ],
                );
              }),
            ],
          ),
        ),
      ],
    );
  }
  
  Widget _buildStep(BuildContext context, int number, String text, IconData icon) {
    return Row(
      children: [
        Container(
          width: 28,
          height: 28,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: context.theme.colorScheme.primary.withOpacity(0.2),
          ),
          child: Center(
            child: Text(
              '$number',
              style: TextStyle(
                color: context.theme.colorScheme.primary,
                fontWeight: FontWeight.bold,
              ),
            ),
          ),
        ),
        const SizedBox(width: 12),
        Icon(icon, size: 20, color: context.theme.colorScheme.outline),
        const SizedBox(width: 8),
        Expanded(
          child: Text(
            text,
            style: context.theme.textTheme.bodyMedium,
          ),
        ),
      ],
    );
  }
}
