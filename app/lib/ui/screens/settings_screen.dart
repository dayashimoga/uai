import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';

/// Settings and preferences screen for Universal AI Assistant.
class SettingsScreen extends StatelessWidget {
  final AssistantController controller;

  const SettingsScreen({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: const Text('Settings'),
      ),
      body: ListView(
        padding: const EdgeInsets.all(16.0),
        children: <Widget>[
          Text(
            'Appearance & Display',
            style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 8),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(16.0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    'Theme Selection',
                    style: theme.textTheme.bodyMedium?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                  ),
                  const SizedBox(height: 12),
                  SizedBox(
                    width: double.infinity,
                    child: SegmentedButton<String>(
                      segments: const <ButtonSegment<String>>[
                        ButtonSegment(
                          value: 'system',
                          label: Text('System'),
                          icon: Icon(Icons.brightness_auto),
                        ),
                        ButtonSegment(
                          value: 'light',
                          label: Text('Light'),
                          icon: Icon(Icons.light_mode),
                        ),
                        ButtonSegment(
                          value: 'dark',
                          label: Text('Dark'),
                          icon: Icon(Icons.dark_mode),
                        ),
                      ],
                      selected: <String>{controller.themeMode},
                      onSelectionChanged: (Set<String> newSelection) {
                        if (newSelection.isNotEmpty) {
                          controller.setThemeMode(newSelection.first);
                        }
                      },
                    ),
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 24),

          Text(
            'Offline Storage & Privacy',
            style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 8),
          Card(
            child: Column(
              children: <Widget>[
                const ListTile(
                  leading: Icon(Icons.storage_outlined),
                  title: Text('Database Storage Engine'),
                  subtitle: Text('SQLite 3 (WAL Mode, synchronous = NORMAL)'),
                ),
                const Divider(height: 1),
                const ListTile(
                  leading: Icon(Icons.lock_outline),
                  title: Text('Master Secret Encryption'),
                  subtitle: Text('XChaCha20-Poly1305 with OS Keystore / DPAPI seal'),
                ),
                const Divider(height: 1),
                const ListTile(
                  leading: Icon(Icons.history_edu_outlined),
                  title: Text('Audit Log Cryptography'),
                  subtitle: Text('BLAKE3 Hash Chained, Append-Only'),
                ),
                const Divider(height: 1),
                ListTile(
                  leading: const Icon(Icons.delete_sweep_outlined, color: Colors.orange),
                  title: const Text('Trim Cache Buffers'),
                  subtitle: const Text('Evict in-memory model caches and shrink SQLite memory'),
                  trailing: OutlinedButton(
                    onPressed: () => controller.triggerMemoryPressure(false),
                    child: const Text('Trim'),
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: 24),

          Text(
            'About Universal AI Assistant',
            style: theme.textTheme.titleMedium?.copyWith(fontWeight: FontWeight.bold),
          ),
          const SizedBox(height: 8),
          const Card(
            child: Column(
              children: <Widget>[
                ListTile(
                  leading: Icon(Icons.info_outline),
                  title: Text('Version'),
                  subtitle: Text('v0.1.0-alpha (Stage 1 Architecture)'),
                ),
                Divider(height: 1),
                ListTile(
                  leading: Icon(Icons.verified_outlined),
                  title: Text('Operating Mode'),
                  subtitle: Text('100% Offline-First (Zero Mandatory Cloud Dependencies)'),
                ),
                Divider(height: 1),
                ListTile(
                  leading: Icon(Icons.code),
                  title: Text('License'),
                  subtitle: Text('Apache 2.0 Open Source'),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
