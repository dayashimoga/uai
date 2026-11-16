import 'dart:convert';
import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/automation_models.dart';
import '../../core/models/smart_home_models.dart';

/// Interactive UI for Routine Automation Management, Smart-Home Control & Execution History.
class AutomationScreen extends StatefulWidget {
  final AssistantController controller;

  const AutomationScreen({super.key, required this.controller});

  @override
  State<AutomationScreen> createState() => _AutomationScreenState();
}

class _AutomationScreenState extends State<AutomationScreen>
    with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  String _selectedHomeDomain = 'All';

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 3, vsync: this);
    _tabController.addListener(() {
      if (mounted) {
        setState(() {});
      }
    });
    widget.controller.loadAutomations();
    widget.controller.loadAutomationHistory();
    widget.controller.loadHomeEntities();
  }

  @override
  void dispose() {
    _tabController.dispose();
    super.dispose();
  }

  void _showAddRoutineDialog() {
    final nameCtrl = TextEditingController(text: 'Morning Routine');
    var triggerType = AutomationTriggerType.cron;
    final cronExpCtrl = TextEditingController(text: '@daily');
    final eventNameCtrl = TextEditingController(text: 'battery_low');
    final notifTitleCtrl = TextEditingController(text: 'Daily Brief');
    final notifMsgCtrl = TextEditingController(text: 'Ready for today.');

    showDialog<void>(
      context: context,
      builder: (ctx) {
        return StatefulBuilder(
          builder: (dialogCtx, setDialogState) {
            return AlertDialog(
              title: const Text('Create Automation Routine'),
              content: SingleChildScrollView(
                child: SizedBox(
                  width: 480,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      TextField(
                        controller: nameCtrl,
                        decoration: const InputDecoration(
                          labelText: 'Routine Name',
                          hintText: 'e.g. Bedtime Routine or Low Battery Mute',
                          border: OutlineInputBorder(),
                        ),
                      ),
                      const SizedBox(height: 16),
                      DropdownButtonFormField<AutomationTriggerType>(
                        initialValue: triggerType,
                        decoration: const InputDecoration(
                          labelText: 'Trigger Type',
                          border: OutlineInputBorder(),
                        ),
                        items: AutomationTriggerType.values.map((t) {
                          return DropdownMenuItem(
                            value: t,
                            child: Text(t.label),
                          );
                        }).toList(),
                        onChanged: (val) {
                          if (val != null) {
                            setDialogState(() => triggerType = val);
                          }
                        },
                      ),
                      const SizedBox(height: 16),
                      if (triggerType == AutomationTriggerType.cron) ...[
                        TextField(
                          controller: cronExpCtrl,
                          decoration: const InputDecoration(
                            labelText: 'Cron Expression / Interval',
                            hintText: '@daily, @hourly, or interval:15m',
                            border: OutlineInputBorder(),
                          ),
                        ),
                      ] else if (triggerType == AutomationTriggerType.event) ...[
                        TextField(
                          controller: eventNameCtrl,
                          decoration: const InputDecoration(
                            labelText: 'System Event Name',
                            hintText: 'e.g. battery_low, network_disconnected',
                            border: OutlineInputBorder(),
                          ),
                        ),
                      ] else ...[
                        const Text(
                          'Manual trigger routines run only on demand from UI or voice.',
                          style: TextStyle(color: Colors.grey),
                        ),
                      ],
                      const SizedBox(height: 20),
                      Text(
                        'Initial Action (Notification)',
                        style: Theme.of(ctx).textTheme.titleSmall?.copyWith(
                              fontWeight: FontWeight.bold,
                            ),
                      ),
                      const SizedBox(height: 8),
                      TextField(
                        controller: notifTitleCtrl,
                        decoration: const InputDecoration(
                          labelText: 'Notification Title',
                          border: OutlineInputBorder(),
                        ),
                      ),
                      const SizedBox(height: 12),
                      TextField(
                        controller: notifMsgCtrl,
                        decoration: const InputDecoration(
                          labelText: 'Notification Message',
                          border: OutlineInputBorder(),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(ctx).pop(),
                  child: const Text('Cancel'),
                ),
                FilledButton(
                  onPressed: () {
                    final name = nameCtrl.text.trim();
                    if (name.isEmpty) return;

                    final now = DateTime.now().millisecondsSinceEpoch;
                    final ruleId =
                        'auto_${now.toRadixString(16)}';

                    final triggerConfigJson = triggerType ==
                            AutomationTriggerType.cron
                        ? jsonEncode({
                            'type': 'cron',
                            'expression': cronExpCtrl.text.trim().isEmpty
                                ? '@daily'
                                : cronExpCtrl.text.trim()
                          })
                        : triggerType == AutomationTriggerType.event
                            ? jsonEncode({
                                'type': 'event',
                                'event_name': eventNameCtrl.text.trim().isEmpty
                                    ? 'system_event'
                                    : eventNameCtrl.text.trim(),
                              })
                            : jsonEncode({'type': 'manual'});

                    final actionsJson = jsonEncode([
                      {
                        'action_type': 'notification',
                        'title': notifTitleCtrl.text.trim().isEmpty
                            ? 'Routine Alert'
                            : notifTitleCtrl.text.trim(),
                        'message': notifMsgCtrl.text.trim().isEmpty
                            ? 'Action triggered'
                            : notifMsgCtrl.text.trim(),
                      }
                    ]);

                    final newRule = AutomationRuleItem(
                      id: ruleId,
                      name: name,
                      enabled: true,
                      triggerType: triggerType.value,
                      triggerConfigJson: triggerConfigJson,
                      actionsJson: actionsJson,
                      createdAtMs: now,
                      updatedAtMs: now,
                    );

                    widget.controller.saveAutomation(newRule);
                    Navigator.of(ctx).pop();
                  },
                  child: const Text('Create Routine'),
                ),
              ],
            );
          },
        );
      },
    );
  }

  void _showAddSmartDeviceDialog() {
    final idCtrl = TextEditingController(text: 'light.desk_lamp');
    final nameCtrl = TextEditingController(text: 'Desk Lamp');
    var domain = SmartHomeDomain.light;
    var protocol = SmartHomeProtocol.homeAssistant;

    showDialog<void>(
      context: context,
      builder: (ctx) {
        return StatefulBuilder(
          builder: (dialogCtx, setDialogState) {
            return AlertDialog(
              title: const Text('Register Smart Home Device'),
              content: SingleChildScrollView(
                child: SizedBox(
                  width: 440,
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      TextField(
                        controller: idCtrl,
                        decoration: const InputDecoration(
                          labelText: 'Entity ID',
                          hintText: 'e.g. light.desk or climate.living_room',
                          border: OutlineInputBorder(),
                        ),
                      ),
                      const SizedBox(height: 12),
                      TextField(
                        controller: nameCtrl,
                        decoration: const InputDecoration(
                          labelText: 'Friendly Name',
                          hintText: 'e.g. Desk Lamp',
                          border: OutlineInputBorder(),
                        ),
                      ),
                      const SizedBox(height: 12),
                      DropdownButtonFormField<SmartHomeDomain>(
                        initialValue: domain,
                        decoration: const InputDecoration(
                          labelText: 'Device Domain',
                          border: OutlineInputBorder(),
                        ),
                        items: SmartHomeDomain.values.map((d) {
                          return DropdownMenuItem(
                            value: d,
                            child: Text(d.label),
                          );
                        }).toList(),
                        onChanged: (val) {
                          if (val != null) {
                            setDialogState(() => domain = val);
                          }
                        },
                      ),
                      const SizedBox(height: 12),
                      DropdownButtonFormField<SmartHomeProtocol>(
                        initialValue: protocol,
                        decoration: const InputDecoration(
                          labelText: 'Protocol',
                          border: OutlineInputBorder(),
                        ),
                        items: SmartHomeProtocol.values.map((p) {
                          return DropdownMenuItem(
                            value: p,
                            child: Text(p.label),
                          );
                        }).toList(),
                        onChanged: (val) {
                          if (val != null) {
                            setDialogState(() => protocol = val);
                          }
                        },
                      ),
                    ],
                  ),
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.of(ctx).pop(),
                  child: const Text('Cancel'),
                ),
                FilledButton(
                  onPressed: () {
                    final id = idCtrl.text.trim();
                    final name = nameCtrl.text.trim();
                    if (id.isEmpty || name.isEmpty) return;

                    final now = DateTime.now().millisecondsSinceEpoch;
                    final stateJson = domain == SmartHomeDomain.climate
                        ? jsonEncode({'state': 'heat', 'target_temperature': 21.0})
                        : jsonEncode({'state': 'off'});

                    final entity = SmartHomeEntityItem(
                      id: id,
                      domain: domain,
                      name: name,
                      stateJson: stateJson,
                      attributesJson: jsonEncode({'friendly_name': name}),
                      protocol: protocol,
                      isAvailable: true,
                      lastUpdatedMs: now,
                    );

                    widget.controller.upsertHomeEntity(entity);
                    Navigator.of(ctx).pop();
                  },
                  child: const Text('Register Device'),
                ),
              ],
            );
          },
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) {
        final automations = widget.controller.automations;
        final history = widget.controller.automationHistory;
        final homeEntities = widget.controller.homeEntities;
        final isLoading = widget.controller.isAutomationLoading ||
            widget.controller.isHomeLoading;

        return Scaffold(
          appBar: AppBar(
            title: const Text('Automations & Smart Home'),
            actions: [
              Container(
                margin: const EdgeInsets.only(right: 16),
                padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 4),
                decoration: BoxDecoration(
                  color: Colors.green.withValues(alpha: 0.15),
                  borderRadius: BorderRadius.circular(16),
                  border: Border.all(color: Colors.green.withValues(alpha: 0.4)),
                ),
                child: const Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(Icons.shield_outlined, size: 16, color: Colors.green),
                    SizedBox(width: 4),
                    Text(
                      '100% Offline',
                      style: TextStyle(
                        color: Colors.green,
                        fontWeight: FontWeight.bold,
                        fontSize: 12,
                      ),
                    ),
                  ],
                ),
              ),
            ],
            bottom: TabBar(
              controller: _tabController,
              tabs: [
                Tab(
                  icon: const Icon(Icons.auto_mode),
                  text: 'Routines (${automations.length})',
                ),
                Tab(
                  icon: const Icon(Icons.home_outlined),
                  text: 'Smart Home (${homeEntities.length})',
                ),
                Tab(
                  icon: const Icon(Icons.history),
                  text: 'History (${history.length})',
                ),
              ],
            ),
          ),
          body: isLoading && automations.isEmpty && homeEntities.isEmpty
              ? const Center(child: CircularProgressIndicator())
              : TabBarView(
                  controller: _tabController,
                  children: [
                    _buildRoutinesTab(context, automations),
                    _buildSmartHomeTab(context, homeEntities),
                    _buildHistoryTab(context, history),
                  ],
                ),
          floatingActionButton: _tabController.index == 0
              ? FloatingActionButton.extended(
                  onPressed: _showAddRoutineDialog,
                  icon: const Icon(Icons.add),
                  label: const Text('New Routine'),
                )
              : _tabController.index == 1
                  ? FloatingActionButton.extended(
                      onPressed: _showAddSmartDeviceDialog,
                      icon: const Icon(Icons.add),
                      label: const Text('Add Device'),
                    )
                  : null,
        );
      },
    );
  }

  Widget _buildRoutinesTab(
    BuildContext context,
    List<AutomationRuleItem> automations,
  ) {
    final theme = Theme.of(context);
    final report = widget.controller.lastWorkflowReport;

    return RefreshIndicator(
      onRefresh: () async {
        await widget.controller.loadAutomations();
        await widget.controller.loadAutomationHistory();
      },
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          // Last Execution Banner if present
          if (report != null) ...[
            Card(
              color: report.status == 'success'
                  ? Colors.green.withValues(alpha: 0.1)
                  : Colors.red.withValues(alpha: 0.1),
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(12),
                side: BorderSide(
                  color: report.status == 'success'
                      ? Colors.green.withValues(alpha: 0.4)
                      : Colors.red.withValues(alpha: 0.4),
                ),
              ),
              child: Padding(
                padding: const EdgeInsets.all(12),
                child: Row(
                  children: [
                    Icon(
                      report.status == 'success'
                          ? Icons.check_circle_outline
                          : Icons.error_outline,
                      color:
                          report.status == 'success' ? Colors.green : Colors.red,
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            'Last Run: ${report.automationId} • ${report.status.toUpperCase()}',
                            style: const TextStyle(fontWeight: FontWeight.bold),
                          ),
                          Text(
                            'Execution duration: ${report.durationMs}ms • Triggered by: ${report.triggeredBy}',
                            style: theme.textTheme.bodySmall,
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 16),
          ],

          if (automations.isEmpty)
            Center(
              child: Padding(
                padding: const EdgeInsets.only(top: 60),
                child: Column(
                  children: [
                    Icon(
                      Icons.auto_mode,
                      size: 64,
                      color: theme.colorScheme.primary.withValues(alpha: 0.5),
                    ),
                    const SizedBox(height: 16),
                    Text(
                      'No Active Automation Routines',
                      style: theme.textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                    const SizedBox(height: 8),
                    const Text(
                      'Tap "New Routine" to configure local cron schedules or event triggers.',
                      textAlign: TextAlign.center,
                    ),
                  ],
                ),
              ),
            )
          else
            ...automations.map((rule) => _buildRoutineCard(context, rule)),
        ],
      ),
    );
  }

  Widget _buildRoutineCard(BuildContext context, AutomationRuleItem rule) {
    final theme = Theme.of(context);
    final triggerType = AutomationTriggerType.fromString(rule.triggerType);

    final lastRunText = rule.lastExecutedMs != null
        ? DateTime.fromMillisecondsSinceEpoch(rule.lastExecutedMs!)
            .toLocal()
            .toString()
            .split('.')
            .first
        : 'Never executed';

    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: BorderSide(
          color: rule.enabled
              ? theme.colorScheme.primary.withValues(alpha: 0.3)
              : Colors.grey.withValues(alpha: 0.2),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Flexible(
                            child: Text(
                              rule.name,
                              style: theme.textTheme.titleMedium?.copyWith(
                                fontWeight: FontWeight.bold,
                              ),
                            ),
                          ),
                          const SizedBox(width: 8),
                          Chip(
                            label: Text(
                              triggerType.label,
                              style: const TextStyle(fontSize: 10),
                            ),
                            padding: EdgeInsets.zero,
                            visualDensity: VisualDensity.compact,
                          ),
                        ],
                      ),
                      const SizedBox(height: 4),
                      Text(
                        'ID: ${rule.id} • Last Run: $lastRunText',
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: Colors.grey[600],
                        ),
                      ),
                    ],
                  ),
                ),
                Switch(
                  value: rule.enabled,
                  onChanged: (val) {
                    widget.controller.toggleAutomation(rule.id, val);
                  },
                ),
              ],
            ),
            const Divider(height: 24),
            Row(
              mainAxisAlignment: MainAxisAlignment.end,
              children: [
                OutlinedButton.icon(
                  onPressed: () {
                    widget.controller.deleteAutomation(rule.id);
                  },
                  icon: const Icon(Icons.delete_outline, size: 18),
                  label: const Text('Delete'),
                ),
                const SizedBox(width: 8),
                FilledButton.tonalIcon(
                  onPressed: () {
                    widget.controller.triggerAutomation(rule.id);
                  },
                  icon: const Icon(Icons.play_arrow, size: 18),
                  label: const Text('Run Now'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildSmartHomeTab(
    BuildContext context,
    List<SmartHomeEntityItem> homeEntities,
  ) {
    final theme = Theme.of(context);

    // Filter categories
    final categories = <String>[
      'All',
      'light',
      'climate',
      'switch',
      'fan',
      'cover',
      'sensor',
      'lock',
      'media_player',
    ];

    final filtered = _selectedHomeDomain == 'All'
        ? homeEntities
        : homeEntities
            .where((e) => e.domain.value == _selectedHomeDomain)
            .toList();

    return RefreshIndicator(
      onRefresh: () async {
        await widget.controller.loadHomeEntities();
      },
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          // Domain Filter Chips
          SingleChildScrollView(
            scrollDirection: Axis.horizontal,
            child: Row(
              children: categories.map((cat) {
                final isSelected = _selectedHomeDomain == cat;
                return Padding(
                  padding: const EdgeInsets.only(right: 8),
                  child: FilterChip(
                    label: Text(cat.toUpperCase()),
                    selected: isSelected,
                    onSelected: (val) {
                      setState(() {
                        _selectedHomeDomain = cat;
                      });
                    },
                  ),
                );
              }).toList(),
            ),
          ),
          const SizedBox(height: 16),

          if (filtered.isEmpty)
            Center(
              child: Padding(
                padding: const EdgeInsets.only(top: 60),
                child: Column(
                  children: [
                    Icon(
                      Icons.home_outlined,
                      size: 64,
                      color: theme.colorScheme.primary.withValues(alpha: 0.5),
                    ),
                    const SizedBox(height: 16),
                    Text(
                      'No Smart Home Devices in "$_selectedHomeDomain"',
                      style: theme.textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                    const SizedBox(height: 8),
                    const Text(
                      'Tap "Add Device" to register a local MQTT or Home Assistant entity.',
                    ),
                  ],
                ),
              ),
            )
          else
            ...filtered.map((entity) => _buildSmartDeviceCard(context, entity)),
        ],
      ),
    );
  }

  Widget _buildSmartDeviceCard(
    BuildContext context,
    SmartHomeEntityItem entity,
  ) {
    final theme = Theme.of(context);
    final isTurnedOn = entity.isTurnedOn;

    IconData domainIcon;
    switch (entity.domain) {
      case SmartHomeDomain.light:
        domainIcon = Icons.lightbulb_outline;
        break;
      case SmartHomeDomain.climate:
        domainIcon = Icons.thermostat_outlined;
        break;
      case SmartHomeDomain.switchDevice:
        domainIcon = Icons.toggle_on_outlined;
        break;
      case SmartHomeDomain.fan:
        domainIcon = Icons.mode_fan_off_outlined;
        break;
      case SmartHomeDomain.cover:
        domainIcon = Icons.curtains_outlined;
        break;
      case SmartHomeDomain.sensor:
        domainIcon = Icons.sensors_outlined;
        break;
      case SmartHomeDomain.lock:
        domainIcon = Icons.lock_outline;
        break;
      case SmartHomeDomain.mediaPlayer:
        domainIcon = Icons.tv_outlined;
        break;
    }

    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: BorderSide(
          color: isTurnedOn
              ? theme.colorScheme.primary.withValues(alpha: 0.4)
              : Colors.grey.withValues(alpha: 0.2),
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          children: [
            Row(
              children: [
                CircleAvatar(
                  backgroundColor: isTurnedOn
                      ? theme.colorScheme.primary.withValues(alpha: 0.15)
                      : Colors.grey.withValues(alpha: 0.1),
                  child: Icon(
                    domainIcon,
                    color: isTurnedOn
                        ? theme.colorScheme.primary
                        : Colors.grey[600],
                  ),
                ),
                const SizedBox(width: 14),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        entity.name,
                        style: theme.textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                      const SizedBox(height: 2),
                      Text(
                        '${entity.id} • ${entity.protocol.label}',
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: Colors.grey[600],
                        ),
                      ),
                    ],
                  ),
                ),
                if (entity.domain == SmartHomeDomain.light ||
                    entity.domain == SmartHomeDomain.switchDevice ||
                    entity.domain == SmartHomeDomain.fan) ...[
                  Switch(
                    value: isTurnedOn,
                    onChanged: (val) {
                      widget.controller.controlHomeDevice(
                        SmartHomeCommandItem(
                          entityId: entity.id,
                          command: val ? 'turn_on' : 'turn_off',
                        ),
                      );
                    },
                  ),
                ] else if (entity.domain == SmartHomeDomain.climate) ...[
                  Chip(
                    label: Text(
                      '${entity.targetTemperature ?? 21.0}°C',
                      style: const TextStyle(fontWeight: FontWeight.bold),
                    ),
                  ),
                ],
              ],
            ),
            const SizedBox(height: 8),
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: [
                Row(
                  children: [
                    Container(
                      width: 8,
                      height: 8,
                      decoration: BoxDecoration(
                        color: entity.isAvailable ? Colors.green : Colors.grey,
                        shape: BoxShape.circle,
                      ),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      entity.isAvailable ? 'Online (LAN)' : 'Offline',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: entity.isAvailable ? Colors.green : Colors.grey,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                  ],
                ),
                Row(
                  children: [
                    if (entity.domain == SmartHomeDomain.climate) ...[
                      IconButton(
                        icon: const Icon(Icons.remove, size: 18),
                        onPressed: () {
                          final current = entity.targetTemperature ?? 21.0;
                          widget.controller.controlHomeDevice(
                            SmartHomeCommandItem(
                              entityId: entity.id,
                              command: 'set_temperature',
                              targetCelsius: current - 0.5,
                            ),
                          );
                        },
                      ),
                      IconButton(
                        icon: const Icon(Icons.add, size: 18),
                        onPressed: () {
                          final current = entity.targetTemperature ?? 21.0;
                          widget.controller.controlHomeDevice(
                            SmartHomeCommandItem(
                              entityId: entity.id,
                              command: 'set_temperature',
                              targetCelsius: current + 0.5,
                            ),
                          );
                        },
                      ),
                    ],
                    IconButton(
                      icon: const Icon(Icons.delete_outline, size: 18),
                      onPressed: () {
                        widget.controller.deleteHomeEntity(entity.id);
                      },
                    ),
                  ],
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildHistoryTab(
    BuildContext context,
    List<AutomationHistoryItem> history,
  ) {
    final theme = Theme.of(context);

    return RefreshIndicator(
      onRefresh: () async {
        await widget.controller.loadAutomationHistory();
      },
      child: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              Text(
                'Execution Logs (${history.length})',
                style: theme.textTheme.titleMedium?.copyWith(
                  fontWeight: FontWeight.bold,
                ),
              ),
              if (history.isNotEmpty)
                TextButton.icon(
                  onPressed: () {
                    widget.controller.clearAutomationHistory();
                  },
                  icon: const Icon(Icons.clear_all, size: 18),
                  label: const Text('Clear All'),
                ),
            ],
          ),
          const SizedBox(height: 12),

          if (history.isEmpty)
            Center(
              child: Padding(
                padding: const EdgeInsets.only(top: 60),
                child: Column(
                  children: [
                    Icon(
                      Icons.history,
                      size: 64,
                      color: theme.colorScheme.primary.withValues(alpha: 0.5),
                    ),
                    const SizedBox(height: 16),
                    Text(
                      'No Execution History',
                      style: theme.textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                    const SizedBox(height: 8),
                    const Text('Past routine runs and trigger logs will appear here.'),
                  ],
                ),
              ),
            )
          else
            ...history.map((entry) => _buildHistoryCard(context, entry)),
        ],
      ),
    );
  }

  Widget _buildHistoryCard(
    BuildContext context,
    AutomationHistoryItem entry,
  ) {
    final theme = Theme.of(context);
    final isSuccess = entry.status == 'success';
    final dateStr = DateTime.fromMillisecondsSinceEpoch(entry.executedAtMs)
        .toLocal()
        .toString()
        .split('.')
        .first;

    return Card(
      margin: const EdgeInsets.only(bottom: 10),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(10),
        side: BorderSide(color: Colors.grey.withValues(alpha: 0.15)),
      ),
      child: ListTile(
        leading: Icon(
          isSuccess ? Icons.check_circle : Icons.error,
          color: isSuccess ? Colors.green : Colors.red,
        ),
        title: Text(
          entry.automationId,
          style: const TextStyle(fontWeight: FontWeight.bold),
        ),
        subtitle: Text(
          'Triggered: ${entry.triggeredBy} • ${entry.durationMs}ms\n$dateStr',
          style: theme.textTheme.bodySmall,
        ),
        trailing: Chip(
          label: Text(
            entry.status.toUpperCase(),
            style: TextStyle(
              fontSize: 10,
              fontWeight: FontWeight.bold,
              color: isSuccess ? Colors.green[800] : Colors.red[800],
            ),
          ),
          backgroundColor: isSuccess
              ? Colors.green.withValues(alpha: 0.15)
              : Colors.red.withValues(alpha: 0.15),
          padding: EdgeInsets.zero,
          visualDensity: VisualDensity.compact,
        ),
      ),
    );
  }
}
