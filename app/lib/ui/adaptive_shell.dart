import 'package:flutter/material.dart';

import '../core/assistant_controller.dart';
import 'screens/audit_log_screen.dart';
import 'screens/capabilities_screen.dart';
import 'screens/devices_screen.dart';
import 'screens/diagnostics_screen.dart';
import 'screens/knowledge_screen.dart';
import 'screens/models_screen.dart';
import 'screens/settings_screen.dart';
import 'screens/voice_screen.dart';

/// Responsive adaptive shell providing NavigationRail on desktop/tablet and NavigationBar on mobile.
class AdaptiveShell extends StatefulWidget {
  final AssistantController controller;

  const AdaptiveShell({super.key, required this.controller});

  @override
  State<AdaptiveShell> createState() => _AdaptiveShellState();
}

class _AdaptiveShellState extends State<AdaptiveShell> {
  int _selectedIndex = 0;

  @override
  Widget build(BuildContext context) {
    final width = MediaQuery.of(context).size.width;
    final isDesktop = width >= 640;

    final screens = <Widget>[
      DiagnosticsScreen(controller: widget.controller),
      VoiceScreen(controller: widget.controller),
      ModelsScreen(controller: widget.controller),
      KnowledgeScreen(controller: widget.controller),
      DevicesScreen(controller: widget.controller),
      CapabilitiesScreen(controller: widget.controller),
      AuditLogScreen(controller: widget.controller),
      SettingsScreen(controller: widget.controller),
    ];

    const destinations = <NavigationDestination>[
      NavigationDestination(
        icon: Icon(Icons.speed_outlined),
        selectedIcon: Icon(Icons.speed),
        label: 'Diagnostics',
      ),
      NavigationDestination(
        icon: Icon(Icons.mic_none_outlined),
        selectedIcon: Icon(Icons.mic),
        label: 'Voice',
      ),
      NavigationDestination(
        icon: Icon(Icons.memory_outlined),
        selectedIcon: Icon(Icons.memory),
        label: 'Models',
      ),
      NavigationDestination(
        icon: Icon(Icons.menu_book_outlined),
        selectedIcon: Icon(Icons.menu_book),
        label: 'Knowledge',
      ),
      NavigationDestination(
        icon: Icon(Icons.devices_outlined),
        selectedIcon: Icon(Icons.devices),
        label: 'Devices',
      ),
      NavigationDestination(
        icon: Icon(Icons.grid_view_outlined),
        selectedIcon: Icon(Icons.grid_view),
        label: 'Capabilities',
      ),
      NavigationDestination(
        icon: Icon(Icons.security_outlined),
        selectedIcon: Icon(Icons.security),
        label: 'Audit Log',
      ),
      NavigationDestination(
        icon: Icon(Icons.settings_outlined),
        selectedIcon: Icon(Icons.settings),
        label: 'Settings',
      ),
    ];

    if (isDesktop) {
      return Scaffold(
        body: Row(
          children: <Widget>[
            NavigationRail(
              selectedIndex: _selectedIndex,
              onDestinationSelected: (idx) => setState(() => _selectedIndex = idx),
              extended: width >= 900,
              leading: Padding(
                padding: const EdgeInsets.symmetric(vertical: 16.0),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Icon(
                      Icons.auto_awesome,
                      color: Theme.of(context).colorScheme.primary,
                      size: 28,
                    ),
                    if (width >= 900) ...<Widget>[
                      const SizedBox(width: 8),
                      Text(
                        'AIssistant',
                        style: Theme.of(context).textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                    ],
                  ],
                ),
              ),
              destinations: const <NavigationRailDestination>[
                NavigationRailDestination(
                  icon: Icon(Icons.speed_outlined),
                  selectedIcon: Icon(Icons.speed),
                  label: Text('Diagnostics'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.mic_none_outlined),
                  selectedIcon: Icon(Icons.mic),
                  label: Text('Voice'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.memory_outlined),
                  selectedIcon: Icon(Icons.memory),
                  label: Text('Models'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.menu_book_outlined),
                  selectedIcon: Icon(Icons.menu_book),
                  label: Text('Knowledge'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.devices_outlined),
                  selectedIcon: Icon(Icons.devices),
                  label: Text('Devices'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.grid_view_outlined),
                  selectedIcon: Icon(Icons.grid_view),
                  label: Text('Capabilities'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.security_outlined),
                  selectedIcon: Icon(Icons.security),
                  label: Text('Audit Log'),
                ),
                NavigationRailDestination(
                  icon: Icon(Icons.settings_outlined),
                  selectedIcon: Icon(Icons.settings),
                  label: Text('Settings'),
                ),
              ],
            ),
            const VerticalDivider(thickness: 1, width: 1),
            Expanded(child: screens[_selectedIndex]),
          ],
        ),
      );
    }

    return Scaffold(
      body: screens[_selectedIndex],
      bottomNavigationBar: NavigationBar(
        selectedIndex: _selectedIndex,
        onDestinationSelected: (idx) => setState(() => _selectedIndex = idx),
        destinations: destinations,
      ),
    );
  }
}
