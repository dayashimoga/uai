import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/health_report.dart';

/// Diagnostics dashboard screen displaying real-time system health and subsystem status.
class DiagnosticsScreen extends StatelessWidget {
  final AssistantController controller;

  const DiagnosticsScreen({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    final health = controller.health;
    final theme = Theme.of(context);

    if (controller.isLoading && health == null) {
      return const Center(child: CircularProgressIndicator());
    }

    if (health == null) {
      return Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: <Widget>[
            const Icon(Icons.error_outline, size: 48, color: Colors.red),
            const SizedBox(height: 16),
            Text(
              controller.errorMessage ?? 'Unable to connect to core engine',
              style: theme.textTheme.titleMedium,
            ),
            const SizedBox(height: 16),
            ElevatedButton.icon(
              onPressed: controller.initialize,
              icon: const Icon(Icons.refresh),
              label: const Text('Retry Connection'),
            ),
          ],
        ),
      );
    }

    final Color statusColor;
    final IconData statusIcon;
    switch (health.status) {
      case HealthStatus.healthy:
        statusColor = Colors.green;
        statusIcon = Icons.check_circle_outline;
        break;
      case HealthStatus.degraded:
        statusColor = Colors.orange;
        statusIcon = Icons.warning_amber_outlined;
        break;
      case HealthStatus.critical:
        statusColor = Colors.red;
        statusIcon = Icons.error_outline;
        break;
    }

    return Scaffold(
      appBar: AppBar(
        title: const Text('System Diagnostics'),
        actions: <Widget>[
          IconButton(
            icon: const Icon(Icons.refresh),
            tooltip: 'Refresh Diagnostics',
            onPressed: controller.refreshDiagnostics,
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16.0),
        children: <Widget>[
          // Overall Health Banner Card
          Card(
            elevation: 2,
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
            child: Padding(
              padding: const EdgeInsets.all(20.0),
              child: Row(
                children: <Widget>[
                  Icon(statusIcon, size: 48, color: statusColor),
                  const SizedBox(width: 16),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: <Widget>[
                        Text(
                          'Overall Status: ${health.status.label}',
                          style: theme.textTheme.titleLarge?.copyWith(
                            fontWeight: FontWeight.bold,
                            color: statusColor,
                          ),
                        ),
                        const SizedBox(height: 4),
                        Text(
                          'Lifecycle State: ${health.lifecycleState.toUpperCase()}',
                          style: theme.textTheme.bodyMedium?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                  Chip(
                    avatar: const Icon(Icons.work_outline, size: 16),
                    label: Text('${health.activeTasks} Active Tasks'),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),

          // Subsystem Diagnostics Grid
          Text(
            'Core Subsystems',
            style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 8),

          GridView.count(
            crossAxisCount: MediaQuery.of(context).size.width > 700 ? 4 : 2,
            crossAxisSpacing: 12,
            mainAxisSpacing: 12,
            shrinkWrap: true,
            physics: const NeverScrollableScrollPhysics(),
            children: <Widget>[
              _SubsystemCard(
                title: 'SQLite WAL DB',
                subtitle: health.databaseOk ? 'Operational' : 'Unavailable',
                icon: Icons.storage,
                isOk: health.databaseOk,
              ),
              _SubsystemCard(
                title: 'Security Audit Log',
                subtitle: health.auditOk ? '${health.auditCount} Verified Entries' : 'Compromised',
                icon: Icons.security,
                isOk: health.auditOk,
              ),
              _SubsystemCard(
                title: 'AEAD Keystore',
                subtitle: health.keystoreOk ? 'XChaCha20 Sealed' : 'Missing Key',
                icon: Icons.vpn_key,
                isOk: health.keystoreOk,
              ),
              _SubsystemCard(
                title: 'Platform Capabilities',
                subtitle: '${health.activeCapabilities} of ${health.totalCapabilities} Active',
                icon: Icons.extension,
                isOk: health.activeCapabilities > 0,
              ),
            ],
          ),
          const SizedBox(height: 24),

          // Diagnostic Warnings & Degradations (if any)
          if (health.details.isNotEmpty) ...<Widget>[
            Text(
              'Diagnostic Notices',
              style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 8),
            Card(
              color: theme.colorScheme.surfaceContainerHighest,
              child: Padding(
                padding: const EdgeInsets.all(12.0),
                child: Column(
                  children: health.details.map((detail) {
                    return ListTile(
                      dense: true,
                      leading: const Icon(Icons.info_outline, size: 20),
                      title: Text(detail),
                    );
                  }).toList(),
                ),
              ),
            ),
            const SizedBox(height: 24),
          ],

          // Runtime Lifecycle & Memory Actions
          Text(
            'Supervisor Actions',
            style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 8),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16.0),
              child: Wrap(
                spacing: 12,
                runSpacing: 12,
                children: <Widget>[
                  OutlinedButton.icon(
                    onPressed: () => controller.triggerMemoryPressure(false),
                    icon: const Icon(Icons.cleaning_services),
                    label: const Text('Trim Caches (Moderate)'),
                  ),
                  OutlinedButton.icon(
                    onPressed: () => controller.triggerMemoryPressure(true),
                    icon: const Icon(Icons.memory),
                    label: const Text('Flush WAL & Shrink (Critical)'),
                  ),
                  FilledButton.tonalIcon(
                    onPressed: () {
                      showDialog<void>(
                        context: context,
                        builder: (ctx) => AlertDialog(
                          title: const Text('Graceful Shutdown'),
                          content: const Text(
                            'Are you sure you want to stop all background workers and flush database buffers?',
                          ),
                          actions: <Widget>[
                            TextButton(
                              onPressed: () => Navigator.pop(ctx),
                              child: const Text('Cancel'),
                            ),
                            FilledButton(
                              onPressed: () {
                                Navigator.pop(ctx);
                                controller.triggerShutdown();
                              },
                              child: const Text('Shutdown'),
                            ),
                          ],
                        ),
                      );
                    },
                    icon: const Icon(Icons.power_settings_new),
                    label: const Text('Request Graceful Shutdown'),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _SubsystemCard extends StatelessWidget {
  final String title;
  final String subtitle;
  final IconData icon;
  final bool isOk;

  const _SubsystemCard({
    required this.title,
    required this.subtitle,
    required this.icon,
    required this.isOk,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final color = isOk ? Colors.green : Colors.red;

    return Card(
      elevation: 1,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(14.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: <Widget>[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: <Widget>[
                Icon(icon, color: theme.colorScheme.primary),
                Icon(isOk ? Icons.check_circle : Icons.cancel, color: color, size: 20),
              ],
            ),
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  title,
                  style: theme.textTheme.bodyMedium?.copyWith(fontWeight: FontWeight.bold),
                ),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
