import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/device_models.dart';

/// Screen for managing discovered LAN peers, out-of-band SAS pairing, and granular capability ACLs.
class DevicesScreen extends StatefulWidget {
  final AssistantController controller;

  const DevicesScreen({super.key, required this.controller});

  @override
  State<DevicesScreen> createState() => _DevicesScreenState();
}

class _DevicesScreenState extends State<DevicesScreen> {
  final TextEditingController _sasInputController = TextEditingController();

  @override
  void dispose() {
    _sasInputController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.controller,
      builder: (context, _) {
        final ctrl = widget.controller;

        return Scaffold(
          body: CustomScrollView(
            slivers: <Widget>[
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.all(16.0),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      _buildHeader(context, ctrl),
                      const SizedBox(height: 16),
                      if (ctrl.isPairing || ctrl.activePairingSession != null) ...<Widget>[
                        _buildPairingCeremonyCard(context, ctrl),
                        const SizedBox(height: 24),
                      ],
                      _buildDiscoveredPeersSection(context, ctrl),
                      const SizedBox(height: 24),
                      _buildPairedDevicesSection(context, ctrl),
                    ],
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildHeader(BuildContext context, AssistantController ctrl) {
    final theme = Theme.of(context);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Icon(Icons.devices, color: theme.colorScheme.primary, size: 28),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        'Devices & Cross-Device Mesh',
                        style: theme.textTheme.titleLarge?.copyWith(fontWeight: FontWeight.bold),
                      ),
                      const SizedBox(height: 4),
                      Text(
                        'Mutual-authenticated QUIC transport and out-of-band SAS verification',
                        style: theme.textTheme.bodyMedium?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                ),
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                  decoration: BoxDecoration(
                    color: Colors.green.withAlpha(30),
                    borderRadius: BorderRadius.circular(12),
                    border: Border.all(color: Colors.green.withAlpha(80)),
                  ),
                  child: const Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      Icon(Icons.lock, size: 14, color: Colors.green),
                      SizedBox(width: 4),
                      Text(
                        '100% Offline LAN',
                        style: TextStyle(
                          fontSize: 12,
                          fontWeight: FontWeight.bold,
                          color: Colors.green,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            const Divider(),
            const SizedBox(height: 8),
            Row(
              children: <Widget>[
                OutlinedButton.icon(
                  onPressed: () => ctrl.refreshDevices(),
                  icon: const Icon(Icons.refresh, size: 18),
                  label: const Text('Refresh'),
                ),
                const SizedBox(width: 8),
                OutlinedButton.icon(
                  onPressed: () {
                    final item = DiscoveredPeerItem(
                      instanceName: 'simulated-tablet-${DateTime.now().millisecond}',
                      platform: 'android',
                      port: 51820,
                      addresses: const <String>['192.168.1.188'],
                      fingerprint: 'a1b2c3d4e5f67890',
                      lastSeenMs: DateTime.now().millisecondsSinceEpoch,
                    );
                    ctrl.simulateDiscoveredPeer(item);
                  },
                  icon: const Icon(Icons.add_to_queue, size: 18),
                  label: const Text('Simulate Peer'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildPairingCeremonyCard(BuildContext context, AssistantController ctrl) {
    final theme = Theme.of(context);
    final session = ctrl.activePairingSession;
    final sasCode = session?.sasCode ?? '000-000';
    final peerInstance = session?.peerInstance ?? 'Remote Peer';

    return Card(
      color: theme.colorScheme.primaryContainer.withAlpha(50),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: theme.colorScheme.primary, width: 2),
      ),
      child: Padding(
        padding: const EdgeInsets.all(20.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: <Widget>[
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: <Widget>[
                Icon(Icons.qr_code_scanner, color: theme.colorScheme.primary),
                const SizedBox(width: 8),
                Text(
                  'Pairing Ceremony with $peerInstance',
                  style: theme.textTheme.titleMedium?.copyWith(
                    fontWeight: FontWeight.bold,
                    color: theme.colorScheme.primary,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 16),
            Text(
              'Verify this 6-digit Short Authentication String matches on both screens:',
              textAlign: TextAlign.center,
              style: theme.textTheme.bodyMedium,
            ),
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 12),
              decoration: BoxDecoration(
                color: theme.colorScheme.surface,
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: theme.colorScheme.outlineVariant),
              ),
              child: Text(
                sasCode,
                key: const Key('sas_code_display'),
                style: theme.textTheme.headlineLarge?.copyWith(
                  fontWeight: FontWeight.bold,
                  letterSpacing: 4.0,
                  fontFamily: 'monospace',
                  color: theme.colorScheme.primary,
                ),
              ),
            ),
            const SizedBox(height: 16),
            Container(
              width: 140,
              height: 140,
              decoration: BoxDecoration(
                color: Colors.white,
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: Colors.grey.shade400),
              ),
              child: Column(
                mainAxisAlignment: MainAxisAlignment.center,
                children: <Widget>[
                  const Icon(Icons.qr_code_2, size: 90, color: Colors.black87),
                  const SizedBox(height: 4),
                  Text(
                    'QR Payload Ready',
                    style: TextStyle(fontSize: 10, color: Colors.grey.shade800),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 16),
            if (ctrl.pairingError != null) ...<Widget>[
              Container(
                padding: const EdgeInsets.all(8),
                decoration: BoxDecoration(
                  color: Colors.red.withAlpha(20),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Text(
                  ctrl.pairingError!,
                  style: const TextStyle(color: Colors.red, fontWeight: FontWeight.bold),
                ),
              ),
              const SizedBox(height: 12),
            ],
            SizedBox(
              width: 260,
              child: TextField(
                key: const Key('pairing_code_input'),
                controller: _sasInputController,
                textAlign: TextAlign.center,
                maxLength: 7,
                decoration: const InputDecoration(
                  labelText: 'Enter SAS Code from peer',
                  hintText: 'e.g. 481-923',
                  border: OutlineInputBorder(),
                  counterText: '',
                ),
              ),
            ),
            const SizedBox(height: 16),
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: <Widget>[
                FilledButton.icon(
                  key: const Key('confirm_pairing_button'),
                  onPressed: () {
                    final code = _sasInputController.text.trim();
                    if (code.isNotEmpty) {
                      ctrl.confirmPairing(code);
                    } else {
                      ctrl.confirmPairing(sasCode);
                    }
                  },
                  icon: const Icon(Icons.check),
                  label: const Text('Confirm Pairing'),
                ),
                const SizedBox(width: 12),
                OutlinedButton.icon(
                  key: const Key('cancel_pairing_button'),
                  onPressed: () => ctrl.cancelPairing(),
                  icon: const Icon(Icons.close),
                  label: const Text('Cancel'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildDiscoveredPeersSection(BuildContext context, AssistantController ctrl) {
    final theme = Theme.of(context);
    final peers = ctrl.discoveredPeers;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          'Nearby Discovered Peers (${peers.length})',
          style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
        ),
        const SizedBox(height: 8),
        if (peers.isEmpty)
          Card(
            child: Padding(
              padding: const EdgeInsets.all(24.0),
              child: Center(
                child: Column(
                  children: <Widget>[
                    Icon(Icons.wifi_find, size: 40, color: theme.colorScheme.outline),
                    const SizedBox(height: 8),
                    Text(
                      'No new peers found on local network.',
                      style: theme.textTheme.bodyMedium?.copyWith(color: theme.colorScheme.outline),
                    ),
                  ],
                ),
              ),
            ),
          )
        else
          ...peers.map((peer) => _buildDiscoveredPeerCard(context, ctrl, peer)),
      ],
    );
  }

  Widget _buildDiscoveredPeerCard(
    BuildContext context,
    AssistantController ctrl,
    DiscoveredPeerItem peer,
  ) {
    final theme = Theme.of(context);
    final isDesktop = peer.platform == 'windows' || peer.platform == 'macos' || peer.platform == 'linux';

    return Card(
      margin: const EdgeInsets.only(bottom: 8.0),
      child: ListTile(
        leading: CircleAvatar(
          backgroundColor: theme.colorScheme.secondaryContainer,
          child: Icon(
            isDesktop ? Icons.computer : Icons.smartphone,
            color: theme.colorScheme.onSecondaryContainer,
          ),
        ),
        title: Row(
          children: <Widget>[
            Text(peer.instanceName, style: const TextStyle(fontWeight: FontWeight.bold)),
            const SizedBox(width: 8),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
              decoration: BoxDecoration(
                color: theme.colorScheme.surfaceContainerHighest,
                borderRadius: BorderRadius.circular(4),
              ),
              child: Text(
                peer.platform.toUpperCase(),
                style: const TextStyle(fontSize: 10, fontWeight: FontWeight.bold),
              ),
            ),
          ],
        ),
        subtitle: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            const SizedBox(height: 4),
            Text(
              'IP: ${peer.addresses.isNotEmpty ? peer.addresses.join(', ') : 'Unknown'} : ${peer.port}',
              style: TextStyle(fontSize: 12, color: theme.colorScheme.outline),
            ),
            Text(
              'Fingerprint: ${peer.fingerprint}',
              style: TextStyle(fontSize: 11, fontFamily: 'monospace', color: theme.colorScheme.outline),
            ),
          ],
        ),
        trailing: FilledButton.tonalIcon(
          key: Key('pair_button_${peer.instanceName}'),
          onPressed: () => ctrl.startPairing(peer.instanceName),
          icon: const Icon(Icons.link, size: 16),
          label: const Text('Pair'),
        ),
      ),
    );
  }

  Widget _buildPairedDevicesSection(BuildContext context, AssistantController ctrl) {
    final theme = Theme.of(context);
    final paired = ctrl.pairedDevices;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          'Paired Devices (${paired.length})',
          style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
        ),
        const SizedBox(height: 8),
        if (paired.isEmpty)
          Card(
            child: Padding(
              padding: const EdgeInsets.all(24.0),
              child: Center(
                child: Column(
                  children: <Widget>[
                    Icon(Icons.devices_other, size: 40, color: theme.colorScheme.outline),
                    const SizedBox(height: 8),
                    Text(
                      'No paired devices. Pair a nearby peer to securely sync clipboard and dispatch remote skills.',
                      textAlign: TextAlign.center,
                      style: theme.textTheme.bodyMedium?.copyWith(color: theme.colorScheme.outline),
                    ),
                  ],
                ),
              ),
            ),
          )
        else
          ...paired.map((dev) => _buildPairedDeviceCard(context, ctrl, dev)),
      ],
    );
  }

  Widget _buildPairedDeviceCard(
    BuildContext context,
    AssistantController ctrl,
    PairedDeviceItem dev,
  ) {
    final theme = Theme.of(context);
    final isDesktop = dev.platform == 'windows' || dev.platform == 'macos' || dev.platform == 'linux';
    final isTrusted = dev.trustState.toLowerCase() == 'trusted';

    return Card(
      margin: const EdgeInsets.only(bottom: 12.0),
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                CircleAvatar(
                  backgroundColor: theme.colorScheme.primaryContainer,
                  child: Icon(
                    isDesktop ? Icons.desktop_windows : Icons.phone_android,
                    color: theme.colorScheme.onPrimaryContainer,
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Text(
                            dev.name,
                            style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
                          ),
                          const SizedBox(width: 8),
                          Container(
                            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                            decoration: BoxDecoration(
                              color: isTrusted
                                  ? Colors.green.withAlpha(30)
                                  : (dev.trustState == 'revoked' ? Colors.red.withAlpha(30) : Colors.amber.withAlpha(30)),
                              borderRadius: BorderRadius.circular(4),
                              border: Border.all(
                                color: isTrusted
                                    ? Colors.green.withAlpha(80)
                                    : (dev.trustState == 'revoked'
                                        ? Colors.red.withAlpha(80)
                                        : Colors.amber.withAlpha(80)),
                              ),
                            ),
                            child: Text(
                              dev.trustState.toUpperCase(),
                              style: TextStyle(
                                fontSize: 10,
                                fontWeight: FontWeight.bold,
                                color: isTrusted
                                    ? Colors.green
                                    : (dev.trustState == 'revoked' ? Colors.red : Colors.amber.shade800),
                              ),
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 2),
                      Text(
                        'ID: ${dev.deviceId} | Platform: ${dev.platform.toUpperCase()}',
                        style: TextStyle(fontSize: 12, color: theme.colorScheme.outline),
                      ),
                      Text(
                        'Fingerprint: ${dev.fingerprint}',
                        style: TextStyle(fontSize: 11, fontFamily: 'monospace', color: theme.colorScheme.outline),
                      ),
                    ],
                  ),
                ),
                if (dev.trustState != 'revoked')
                  IconButton(
                    key: Key('revoke_button_${dev.deviceId}'),
                    icon: const Icon(Icons.block_outlined, color: Colors.orange),
                    tooltip: 'Revoke trust',
                    onPressed: () {
                      showDialog<void>(
                        context: context,
                        builder: (ctx) => AlertDialog(
                          title: const Text('Revoke Device Trust?'),
                          content: Text(
                            'Are you sure you want to revoke cryptographic trust for "${dev.name}"? '
                            'Future connections and skill requests from this peer will be rejected.',
                          ),
                          actions: <Widget>[
                            TextButton(
                              onPressed: () => Navigator.of(ctx).pop(),
                              child: const Text('Cancel'),
                            ),
                            FilledButton(
                              style: FilledButton.styleFrom(backgroundColor: Colors.orange.shade800),
                              onPressed: () {
                                Navigator.of(ctx).pop();
                                ctrl.revokeDevice(dev.deviceId);
                              },
                              child: const Text('Revoke'),
                            ),
                          ],
                        ),
                      );
                    },
                  ),
                IconButton(
                  key: Key('unpair_button_${dev.deviceId}'),
                  icon: const Icon(Icons.delete_outline, color: Colors.red),
                  tooltip: 'Unpair device',
                  onPressed: () {
                    showDialog<void>(
                      context: context,
                      builder: (ctx) => AlertDialog(
                        title: const Text('Unpair Device?'),
                        content: Text(
                          'Are you sure you want to unpair "${dev.name}"? '
                          'All granted permissions and cryptographic trust will be revoked immediately.',
                        ),
                        actions: <Widget>[
                          TextButton(
                            onPressed: () => Navigator.of(ctx).pop(),
                            child: const Text('Cancel'),
                          ),
                          FilledButton(
                            style: FilledButton.styleFrom(backgroundColor: Colors.red),
                            onPressed: () {
                              Navigator.of(ctx).pop();
                              ctrl.unpairDevice(dev.deviceId);
                            },
                            child: const Text('Unpair'),
                          ),
                        ],
                      ),
                    );
                  },
                ),
              ],
            ),
            const SizedBox(height: 12),
            const Divider(),
            const SizedBox(height: 8),
            Text(
              'Capability Access Control List (ACL)',
              style: theme.textTheme.bodyMedium?.copyWith(
                fontWeight: FontWeight.bold,
                color: theme.colorScheme.primary,
              ),
            ),
            const SizedBox(height: 8),
            ..._buildCapabilityAclRows(ctrl, dev),
          ],
        ),
      ),
    );
  }

  List<Widget> _buildCapabilityAclRows(AssistantController ctrl, PairedDeviceItem dev) {
    const capabilities = <String, String>{
      'local_llm': 'Local LLM Inference',
      'file_system': 'File System Storage',
      'application_management': 'App Management',
      'lan_peer_networking': 'LAN Mesh Networking',
    };

    return capabilities.entries.map((entry) {
      final capKey = entry.key;
      final capLabel = entry.value;

      final perm = dev.permissions.firstWhere(
        (p) => p.capability == capKey,
        orElse: () => PeerPermissionItem(capability: capKey, permissionLevel: 'ask_each_time'),
      );

      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 4.0),
        child: Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: <Widget>[
            Row(
              children: <Widget>[
                Icon(_iconForCapability(capKey), size: 16, color: Colors.grey.shade600),
                const SizedBox(width: 8),
                Text(capLabel, style: const TextStyle(fontSize: 13)),
              ],
            ),
            DropdownButton<String>(
              key: Key('acl_dropdown_${dev.deviceId}_$capKey'),
              value: perm.permissionLevel,
              underline: const SizedBox(),
              items: const <DropdownMenuItem<String>>[
                DropdownMenuItem(value: 'always_allow', child: Text('Always Allow')),
                DropdownMenuItem(value: 'ask_each_time', child: Text('Ask Each Time')),
                DropdownMenuItem(value: 'deny', child: Text('Deny')),
              ],
              onChanged: (newVal) {
                if (newVal != null && newVal != perm.permissionLevel) {
                  ctrl.updatePeerPermission(dev.deviceId, capKey, newVal);
                }
              },
            ),
          ],
        ),
      );
    }).toList();
  }

  IconData _iconForCapability(String capability) {
    switch (capability) {
      case 'local_llm':
        return Icons.memory;
      case 'file_system':
        return Icons.folder;
      case 'application_management':
        return Icons.apps;
      case 'lan_peer_networking':
        return Icons.lan;
      default:
        return Icons.security;
    }
  }
}
