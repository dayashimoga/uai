import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/capability_item.dart';

/// Platform capability matrix view displaying status badges and runtime permission toggles.
class CapabilitiesScreen extends StatelessWidget {
  final AssistantController controller;

  const CapabilitiesScreen({super.key, required this.controller});

  String _getPlatformDisplayName() {
    switch (defaultTargetPlatform) {
      case TargetPlatform.android:
        return 'Android';
      case TargetPlatform.iOS:
        return 'iOS';
      case TargetPlatform.linux:
        return 'Linux';
      case TargetPlatform.macOS:
        return 'macOS';
      case TargetPlatform.windows:
        return 'Windows';
      case TargetPlatform.fuchsia:
        return 'Fuchsia';
    }
  }

  bool _isDesktopPlatform() {
    return defaultTargetPlatform == TargetPlatform.windows ||
        defaultTargetPlatform == TargetPlatform.linux ||
        defaultTargetPlatform == TargetPlatform.macOS;
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final capabilities = controller.capabilities;

    final supportedCount = capabilities.where((c) => c.status == 'supported' && c.isActive).length;
    final degradedCount = capabilities.where((c) => c.status == 'degraded').length;
    final disabledCount = capabilities.where((c) => !c.isActive && c.status != 'unsupported').length;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Capability Matrix'),
        actions: <Widget>[
          IconButton(
            icon: const Icon(Icons.refresh),
            tooltip: 'Refresh Capabilities',
            onPressed: controller.refreshCapabilities,
          ),
        ],
      ),
      body: capabilities.isEmpty
          ? const Center(child: Text('No capabilities reported by core'))
          : RefreshIndicator(
              onRefresh: () async => controller.refreshCapabilities(),
              child: ListView(
                padding: const EdgeInsets.all(16.0),
                children: <Widget>[
                  // Host Platform Runner Banner
                  Card(
                    color: theme.colorScheme.surfaceContainerHighest,
                    child: Padding(
                      padding: const EdgeInsets.all(16.0),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: <Widget>[
                          Row(
                            children: <Widget>[
                              Icon(
                                _isDesktopPlatform() ? Icons.desktop_windows : Icons.smartphone,
                                color: theme.colorScheme.primary,
                                size: 28,
                              ),
                              const SizedBox(width: 12),
                              Expanded(
                                child: Column(
                                  crossAxisAlignment: CrossAxisAlignment.start,
                                  children: <Widget>[
                                    Text(
                                      '${_getPlatformDisplayName()} Host Runner',
                                      style: theme.textTheme.titleMedium?.copyWith(
                                        fontWeight: FontWeight.bold,
                                      ),
                                    ),
                                    Text(
                                      _isDesktopPlatform()
                                          ? 'Desktop Mode (Unconstrained Background & System Control)'
                                          : 'Mobile Mode (OS Sandbox & Background Lifecycle Rules)',
                                      style: theme.textTheme.bodySmall?.copyWith(
                                        color: theme.colorScheme.onSurfaceVariant,
                                      ),
                                    ),
                                  ],
                                ),
                              ),
                            ],
                          ),
                          const SizedBox(height: 12),
                          Wrap(
                            spacing: 8,
                            runSpacing: 4,
                            children: <Widget>[
                              Chip(
                                avatar: const Icon(Icons.check_circle, size: 16, color: Colors.green),
                                label: Text('$supportedCount Active'),
                                visualDensity: VisualDensity.compact,
                              ),
                              if (degradedCount > 0)
                                Chip(
                                  avatar: const Icon(Icons.warning_amber, size: 16, color: Colors.orange),
                                  label: Text('$degradedCount Degraded'),
                                  visualDensity: VisualDensity.compact,
                                ),
                              if (disabledCount > 0)
                                Chip(
                                  avatar: const Icon(Icons.block, size: 16, color: Colors.grey),
                                  label: Text('$disabledCount Policy Disabled'),
                                  visualDensity: VisualDensity.compact,
                                ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ),
                  const SizedBox(height: 16),
                  Text(
                    'Platform Capabilities (${capabilities.length})',
                    style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
                  ),
                  const SizedBox(height: 8),
                  ...capabilities.map(
                    (cap) => Padding(
                      padding: const EdgeInsets.only(bottom: 8.0),
                      child: _CapabilityTile(
                        capability: cap,
                        onToggle: (val) => controller.toggleCapability(cap.name, val),
                      ),
                    ),
                  ),
                ],
              ),
            ),
    );
  }
}

class _CapabilityTile extends StatelessWidget {
  final CapabilityItem capability;
  final ValueChanged<bool> onToggle;

  const _CapabilityTile({
    required this.capability,
    required this.onToggle,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    Color badgeColor;
    String badgeText;

    if (capability.status == 'supported') {
      badgeColor = Colors.green;
      badgeText = 'Supported';
    } else if (capability.status == 'requires_permission') {
      badgeColor = Colors.orange;
      badgeText = 'Requires Permission';
    } else if (capability.status == 'degraded') {
      badgeColor = Colors.amber.shade800;
      badgeText = 'Degraded';
    } else {
      badgeColor = Colors.grey;
      badgeText = 'Unsupported';
    }

    final isInteractive = capability.status != 'unsupported';

    return Card(
      elevation: 1,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16.0, vertical: 12.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Text(
                        capability.displayName,
                        style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
                      ),
                      const SizedBox(height: 2),
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                        decoration: BoxDecoration(
                          color: theme.colorScheme.secondaryContainer.withValues(alpha: 0.6),
                          borderRadius: BorderRadius.circular(4),
                        ),
                        child: Text(
                          capability.tierDisplayName,
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: theme.colorScheme.onSecondaryContainer,
                            fontSize: 10,
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                  decoration: BoxDecoration(
                    color: badgeColor.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(16),
                    border: Border.all(color: badgeColor.withValues(alpha: 0.5)),
                  ),
                  child: Text(
                    badgeText,
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: badgeColor,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ),
                if (isInteractive) ...<Widget>[
                  const SizedBox(width: 8),
                  Switch(
                    value: capability.isActive,
                    onChanged: onToggle,
                  ),
                ],
              ],
            ),
            if (capability.missingPermission != null) ...<Widget>[
              const SizedBox(height: 6),
              Row(
                children: <Widget>[
                  const Icon(Icons.lock_outline, size: 14, color: Colors.orange),
                  const SizedBox(width: 4),
                  Text(
                    'Permission: ${capability.missingPermission}',
                    style: theme.textTheme.bodySmall?.copyWith(color: Colors.orange.shade800),
                  ),
                ],
              ),
            ],
            if (capability.degradationReason != null) ...<Widget>[
              const SizedBox(height: 4),
              Row(
                children: <Widget>[
                  const Icon(Icons.info_outline, size: 14, color: Colors.grey),
                  const SizedBox(width: 4),
                  Expanded(
                    child: Text(
                      capability.degradationReason!,
                      style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                    ),
                  ),
                ],
              ),
            ],
          ],
        ),
      ),
    );
  }
}
