import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/capability_item.dart';

/// Platform capability matrix view displaying status badges and runtime permission toggles.
class CapabilitiesScreen extends StatelessWidget {
  final AssistantController controller;

  const CapabilitiesScreen({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    final capabilities = controller.capabilities;

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
              child: ListView.separated(
                padding: const EdgeInsets.all(16.0),
                itemCount: capabilities.length,
                separatorBuilder: (_, __) => const SizedBox(height: 8),
                itemBuilder: (context, index) {
                  final cap = capabilities[index];
                  return _CapabilityTile(
                    capability: cap,
                    onToggle: (val) => controller.toggleCapability(cap.name, val),
                  );
                },
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
                  child: Text(
                    capability.displayName,
                    style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
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
