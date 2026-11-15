import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/audit_item.dart';

/// Cryptographic security audit log inspector with real-time hash-chain verification.
class AuditLogScreen extends StatelessWidget {
  final AssistantController controller;

  const AuditLogScreen({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    final auditLogs = controller.auditLogs;
    final integrity = controller.auditIntegrity;
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Security Audit Log'),
        actions: <Widget>[
          IconButton(
            icon: const Icon(Icons.refresh),
            tooltip: 'Refresh Audit Log',
            onPressed: controller.refreshAuditLog,
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.all(16.0),
        children: <Widget>[
          // Cryptographic Chain Integrity Card
          Card(
            elevation: 2,
            shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
            child: Padding(
              padding: const EdgeInsets.all(16.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Row(
                    mainAxisAlignment: MainAxisAlignment.spaceBetween,
                    children: <Widget>[
                      Row(
                        children: <Widget>[
                          Icon(
                            integrity?.isValid == true ? Icons.verified_user : Icons.gpp_bad,
                            color: integrity?.isValid == true ? Colors.green : Colors.red,
                            size: 28,
                          ),
                          const SizedBox(width: 10),
                          Text(
                            integrity?.isValid == true ? 'Chain Integrity Intact' : 'Integrity Violated',
                            style: theme.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.bold,
                              color: integrity?.isValid == true ? Colors.green : Colors.red,
                            ),
                          ),
                        ],
                      ),
                      FilledButton.tonalIcon(
                        onPressed: controller.verifyAuditIntegrity,
                        icon: const Icon(Icons.security_update_good, size: 18),
                        label: const Text('Verify Chain'),
                      ),
                    ],
                  ),
                  if (integrity != null) ...<Widget>[
                    const SizedBox(height: 10),
                    Text(
                      'Verified Count: ${integrity.verifiedCount} records from genesis',
                      style: theme.textTheme.bodyMedium,
                    ),
                    if (integrity.headHash.isNotEmpty) ...<Widget>[
                      const SizedBox(height: 4),
                      SelectableText(
                        'Tip Hash: ${integrity.headHash}',
                        style: theme.textTheme.bodySmall?.copyWith(
                          fontFamily: 'monospace',
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                    if (integrity.errorMessage != null) ...<Widget>[
                      const SizedBox(height: 6),
                      Text(
                        'Error: ${integrity.errorMessage}',
                        style: theme.textTheme.bodySmall?.copyWith(color: Colors.red),
                      ),
                    ],
                  ],
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),

          // Audit Records Header
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: <Widget>[
              Text(
                'Historical Records',
                style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
              ),
              Text(
                'Showing ${auditLogs.length} entries',
                style: theme.textTheme.bodySmall?.copyWith(color: theme.colorScheme.onSurfaceVariant),
              ),
            ],
          ),
          const SizedBox(height: 8),

          // Audit Records List
          if (auditLogs.isEmpty)
            const Padding(
              padding: EdgeInsets.symmetric(vertical: 32.0),
              child: Center(child: Text('No audit entries recorded yet')),
            )
          else
            ...auditLogs.map((entry) => _AuditLogCard(entry: entry)),
        ],
      ),
    );
  }
}

class _AuditLogCard extends StatelessWidget {
  final AuditItem entry;

  const _AuditLogCard({required this.entry});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    Color decisionColor;
    if (entry.decision == 'allow') {
      decisionColor = Colors.green;
    } else if (entry.decision == 'require_confirmation') {
      decisionColor = Colors.orange;
    } else {
      decisionColor = Colors.red;
    }

    final isSuccess = entry.executionStatus == 'success';

    return Card(
      margin: const EdgeInsets.only(bottom: 8.0),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(12.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Chip(
                      label: Text('#${entry.sequence}'),
                      padding: EdgeInsets.zero,
                      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    ),
                    const SizedBox(width: 8),
                    Text(
                      entry.skillId,
                      style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
                    ),
                  ],
                ),
                Row(
                  children: <Widget>[
                    Container(
                      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                      decoration: BoxDecoration(
                        color: decisionColor.withValues(alpha: 0.12),
                        borderRadius: BorderRadius.circular(12),
                        border: Border.all(color: decisionColor.withValues(alpha: 0.4)),
                      ),
                      child: Text(
                        entry.decision.toUpperCase(),
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: decisionColor,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                    ),
                    const SizedBox(width: 6),
                    Icon(
                      isSuccess ? Icons.check_circle : Icons.cancel,
                      size: 16,
                      color: isSuccess ? Colors.green : Colors.red,
                    ),
                  ],
                ),
              ],
            ),
            const SizedBox(height: 8),
            Row(
              children: <Widget>[
                const Icon(Icons.person_outline, size: 14, color: Colors.grey),
                const SizedBox(width: 4),
                Text(
                  'Actor: ${entry.actor}',
                  style: theme.textTheme.bodySmall,
                ),
                const SizedBox(width: 16),
                const Icon(Icons.access_time, size: 14, color: Colors.grey),
                const SizedBox(width: 4),
                Text(
                  entry.dateTime.toIso8601String().split('.').first.replaceAll('T', ' '),
                  style: theme.textTheme.bodySmall,
                ),
              ],
            ),
            const SizedBox(height: 6),
            Text(
              'Payload: ${entry.payloadSummary}',
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
            const Divider(height: 16),
            SelectableText(
              'Hash: ${entry.entryHash.substring(0, 16.clamp(0, entry.entryHash.length))}... | Prev: ${entry.prevHash.substring(0, 16.clamp(0, entry.prevHash.length))}...',
              style: theme.textTheme.labelSmall?.copyWith(
                fontFamily: 'monospace',
                color: theme.colorScheme.outline,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
