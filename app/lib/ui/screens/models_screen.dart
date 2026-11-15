import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/model_item.dart';

/// Models management and live on-device benchmark diagnostics screen.
class ModelsScreen extends StatefulWidget {
  final AssistantController controller;

  const ModelsScreen({super.key, required this.controller});

  @override
  State<ModelsScreen> createState() => _ModelsScreenState();
}

class _ModelsScreenState extends State<ModelsScreen> {
  String _selectedFilter = 'all'; // 'all', 'recommended', 'active'

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) {
        final hardware = widget.controller.hardware;
        final models = widget.controller.models;
        final activeModelId = widget.controller.activeModelId;
        final activeModel = models.cast<ModelItem?>().firstWhere(
              (m) => m?.id == activeModelId,
              orElse: () => null,
            );
        final benchmark = widget.controller.lastBenchmark;
        final isBenchmarking = widget.controller.isBenchmarking;

        final filteredModels = models.where((m) {
          if (_selectedFilter == 'recommended') {
            return m.isRecommended;
          }
          if (_selectedFilter == 'active') {
            return m.isActive;
          }
          return true;
        }).toList();

        return Scaffold(
          appBar: AppBar(
            title: const Text('Local Models & Benchmarks'),
            actions: <Widget>[
              IconButton(
                tooltip: 'Refresh Model Catalog',
                icon: const Icon(Icons.refresh),
                onPressed: widget.controller.refreshModelSnapshot,
              ),
            ],
          ),
          body: ListView(
            padding: const EdgeInsets.all(16.0),
            children: <Widget>[
              // Hardware & Headroom Card
              if (hardware != null) _HardwareOverviewCard(hardware: hardware),

              const SizedBox(height: 16),

              // Active Model Banner
              _ActiveModelBanner(
                activeModel: activeModel,
                onUnload: () => widget.controller.unloadModel(),
              ),

              const SizedBox(height: 16),

              // Benchmark Diagnostics Card
              _BenchmarkCard(
                benchmark: benchmark,
                isBenchmarking: isBenchmarking,
                onRunBenchmark: () => widget.controller.runBenchmark(),
              ),

              const SizedBox(height: 24),

              // Filter Segmented Buttons & Section Title
              Wrap(
                alignment: WrapAlignment.spaceBetween,
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: 12,
                runSpacing: 8,
                children: <Widget>[
                  Text(
                    'Curated Models Catalog',
                    style: Theme.of(context).textTheme.titleMedium?.copyWith(
                          fontWeight: FontWeight.bold,
                        ),
                  ),
                  SegmentedButton<String>(
                    segments: const <ButtonSegment<String>>[
                      ButtonSegment<String>(
                        value: 'all',
                        label: Text('All'),
                      ),
                      ButtonSegment<String>(
                        value: 'recommended',
                        label: Text('Recommended'),
                      ),
                      ButtonSegment<String>(
                        value: 'active',
                        label: Text('Active'),
                      ),
                    ],
                    selected: <String>{_selectedFilter},
                    onSelectionChanged: (set) {
                      setState(() => _selectedFilter = set.first);
                    },
                  ),
                ],
              ),

              const SizedBox(height: 12),

              // Model List
              if (filteredModels.isEmpty)
                const Card(
                  child: Padding(
                    padding: EdgeInsets.all(24.0),
                    child: Center(
                      child: Text('No models match the selected filter.'),
                    ),
                  ),
                )
              else
                ...filteredModels.map(
                  (model) => _ModelCard(
                    model: model,
                    isActive: model.id == activeModelId,
                    onLoad: () => widget.controller.loadModel(model.id),
                    onUnload: () => widget.controller.unloadModel(),
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}

class _HardwareOverviewCard extends StatelessWidget {
  final HardwareOverview hardware;

  const _HardwareOverviewCard({required this.hardware});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Wrap(
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Icon(Icons.memory, color: theme.colorScheme.primary),
                    const SizedBox(width: 8),
                    Text(
                      'Host Hardware & Inference Budget',
                      style: theme.textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ],
                ),
                Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
                  decoration: BoxDecoration(
                    color: Colors.green.withValues(alpha: 0.15),
                    borderRadius: BorderRadius.circular(12),
                  ),
                  child: Row(
                    mainAxisSize: MainAxisSize.min,
                    children: <Widget>[
                      const Icon(Icons.cloud_off, size: 14, color: Colors.green),
                      const SizedBox(width: 4),
                      Text(
                        '100% Offline',
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: Colors.green,
                          fontWeight: FontWeight.bold,
                        ),
                      ),
                    ],
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Wrap(
              spacing: 24,
              runSpacing: 12,
              children: <Widget>[
                _StatTile(
                  label: 'CPU Cores',
                  value: '${hardware.cpuArchitecture} (${hardware.cpuCores}T)',
                ),
                _StatTile(
                  label: 'Total RAM',
                  value: hardware.totalRamFormatted,
                ),
                _StatTile(
                  label: 'LLM Budget',
                  value: hardware.usableBudgetFormatted,
                ),
                _StatTile(
                  label: 'Recommended Tier',
                  value: hardware.recommendedTier,
                ),
              ],
            ),
            const SizedBox(height: 12),
            Wrap(
              spacing: 6,
              runSpacing: 4,
              children: <Widget>[
                ...hardware.simdFeatures.map(
                  (simd) => Chip(
                    label: Text(simd),
                    visualDensity: VisualDensity.compact,
                    labelStyle: theme.textTheme.labelSmall,
                  ),
                ),
                Chip(
                  label: Text('${hardware.recommendedContext} max context tokens'),
                  visualDensity: VisualDensity.compact,
                  backgroundColor: theme.colorScheme.surfaceContainerHighest,
                  labelStyle: theme.textTheme.labelSmall,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _ActiveModelBanner extends StatelessWidget {
  final ModelItem? activeModel;
  final VoidCallback onUnload;

  const _ActiveModelBanner({
    required this.activeModel,
    required this.onUnload,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    if (activeModel == null) {
      return Card(
        color: theme.colorScheme.surfaceContainerHighest,
        child: const Padding(
          padding: EdgeInsets.all(16.0),
          child: Row(
            children: <Widget>[
              Icon(Icons.info_outline, color: Colors.grey),
              SizedBox(width: 12),
              Expanded(
                child: Text(
                  'No LLM model loaded in memory. Select a model below to load.',
                ),
              ),
            ],
          ),
        ),
      );
    }

    return Card(
      color: theme.colorScheme.primaryContainer,
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Wrap(
          alignment: WrapAlignment.spaceBetween,
          crossAxisAlignment: WrapCrossAlignment.center,
          spacing: 12,
          runSpacing: 8,
          children: <Widget>[
            Row(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Icon(Icons.check_circle, color: theme.colorScheme.onPrimaryContainer),
                const SizedBox(width: 12),
                Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    Text(
                      'Active Model: ${activeModel!.name}',
                      style: theme.textTheme.titleSmall?.copyWith(
                        fontWeight: FontWeight.bold,
                        color: theme.colorScheme.onPrimaryContainer,
                      ),
                    ),
                    Text(
                      '${activeModel!.parameterCount} params • ${activeModel!.quantization} • ${activeModel!.fileSizeFormatted}',
                      style: theme.textTheme.bodySmall?.copyWith(
                        color: theme.colorScheme.onPrimaryContainer,
                      ),
                    ),
                  ],
                ),
              ],
            ),
            OutlinedButton.icon(
              onPressed: onUnload,
              icon: const Icon(Icons.eject, size: 16),
              label: const Text('Unload'),
              style: OutlinedButton.styleFrom(
                foregroundColor: theme.colorScheme.onPrimaryContainer,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _BenchmarkCard extends StatelessWidget {
  final BenchmarkResult? benchmark;
  final bool isBenchmarking;
  final VoidCallback onRunBenchmark;

  const _BenchmarkCard({
    required this.benchmark,
    required this.isBenchmarking,
    required this.onRunBenchmark,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Wrap(
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Icon(Icons.speed, color: theme.colorScheme.secondary),
                    const SizedBox(width: 8),
                    Text(
                      'Live Inference Benchmark',
                      style: theme.textTheme.titleMedium?.copyWith(
                        fontWeight: FontWeight.bold,
                      ),
                    ),
                  ],
                ),
                ElevatedButton.icon(
                  key: const Key('run_benchmark_button'),
                  onPressed: isBenchmarking ? null : onRunBenchmark,
                  icon: isBenchmarking
                      ? const SizedBox(
                          width: 16,
                          height: 16,
                          child: CircularProgressIndicator(strokeWidth: 2),
                        )
                      : const Icon(Icons.play_arrow, size: 18),
                  label: Text(
                    isBenchmarking ? 'Running...' : 'Run Benchmark',
                  ),
                ),
              ],
            ),
            if (benchmark != null) ...<Widget>[
              const SizedBox(height: 16),
              Wrap(
                spacing: 8,
                runSpacing: 8,
                children: <Widget>[
                  _BenchmarkMetricTile(
                    label: 'Generation Speed',
                    value:
                        '${benchmark!.tokensPerSecond.toStringAsFixed(1)} tok/s',
                    icon: Icons.bolt,
                  ),
                  _BenchmarkMetricTile(
                    label: 'Time to First Token',
                    value: '${benchmark!.ttftMs} ms',
                    icon: Icons.timer_outlined,
                  ),
                  _BenchmarkMetricTile(
                    label: 'Total Duration',
                    value: '${benchmark!.totalElapsedMs} ms',
                    icon: Icons.hourglass_bottom,
                  ),
                  _BenchmarkMetricTile(
                    label: 'Memory Footprint',
                    value: benchmark!.memoryUsedFormatted,
                    icon: Icons.pie_chart_outline,
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

class _BenchmarkMetricTile extends StatelessWidget {
  final String label;
  final String value;
  final IconData icon;

  const _BenchmarkMetricTile({
    required this.label,
    required this.value,
    required this.icon,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 4),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Icon(icon, size: 14, color: theme.colorScheme.secondary),
              const SizedBox(width: 4),
              Expanded(
                child: Text(
                  label,
                  style: theme.textTheme.labelSmall?.copyWith(color: Colors.grey),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Text(
            value,
            style: theme.textTheme.titleMedium?.copyWith(
              fontWeight: FontWeight.bold,
            ),
          ),
        ],
      ),
    );
  }
}

class _ModelCard extends StatelessWidget {
  final ModelItem model;
  final bool isActive;
  final VoidCallback onLoad;
  final VoidCallback onUnload;

  const _ModelCard({
    required this.model,
    required this.isActive,
    required this.onLoad,
    required this.onUnload,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Card(
      margin: const EdgeInsets.symmetric(vertical: 6.0),
      elevation: isActive ? 2 : 1,
      shape: RoundedRectangleBorder(
        side: isActive
            ? BorderSide(color: theme.colorScheme.primary, width: 2)
            : BorderSide.none,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: <Widget>[
                      Wrap(
                        crossAxisAlignment: WrapCrossAlignment.center,
                        spacing: 8,
                        runSpacing: 4,
                        children: <Widget>[
                          Text(
                            model.name,
                            style: theme.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                          if (model.isRecommended)
                            Container(
                              padding: const EdgeInsets.symmetric(
                                  horizontal: 6, vertical: 2),
                              decoration: BoxDecoration(
                                color: Colors.blue.withValues(alpha: 0.15),
                                borderRadius: BorderRadius.circular(8),
                              ),
                              child: Text(
                                'Recommended',
                                style: theme.textTheme.labelSmall?.copyWith(
                                  color: Colors.blue,
                                  fontWeight: FontWeight.bold,
                                ),
                              ),
                            ),
                        ],
                      ),
                      const SizedBox(height: 4),
                      Text(
                        model.description,
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: Colors.grey.shade600,
                        ),
                      ),
                    ],
                  ),
                ),
                const SizedBox(width: 12),
                if (isActive)
                  FilledButton.tonalIcon(
                    onPressed: onUnload,
                    icon: const Icon(Icons.check, size: 16),
                    label: const Text('Loaded'),
                  )
                else
                  FilledButton.icon(
                    key: Key('load_button_${model.id}'),
                    onPressed: onLoad,
                    icon: const Icon(Icons.download, size: 16),
                    label: const Text('Load'),
                  ),
              ],
            ),
            const Divider(height: 24),
            Wrap(
              spacing: 8,
              runSpacing: 4,
              children: <Widget>[
                _ChipBadge(label: '${model.parameterCount} params'),
                _ChipBadge(label: model.quantization),
                _ChipBadge(label: model.fileSizeFormatted),
                _ChipBadge(label: '${model.defaultContextWindow} ctx'),
                _ChipBadge(label: model.license),
                Tooltip(
                  message: 'SHA-256: ${model.sha256}',
                  child: const _ChipBadge(
                    label: 'SHA-256 Verified',
                    icon: Icons.verified,
                    color: Colors.green,
                  ),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _ChipBadge extends StatelessWidget {
  final String label;
  final IconData? icon;
  final Color? color;

  const _ChipBadge({
    required this.label,
    this.icon,
    this.color,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final effectiveColor = color ?? Colors.grey.shade700;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.6),
        borderRadius: BorderRadius.circular(6),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          if (icon != null) ...<Widget>[
            Icon(icon, size: 12, color: effectiveColor),
            const SizedBox(width: 4),
          ],
          Text(
            label,
            style: theme.textTheme.labelSmall?.copyWith(color: effectiveColor),
          ),
        ],
      ),
    );
  }
}

class _StatTile extends StatelessWidget {
  final String label;
  final String value;

  const _StatTile({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        Text(
          label,
          style: theme.textTheme.labelSmall?.copyWith(color: Colors.grey),
        ),
        const SizedBox(height: 2),
        Text(
          value,
          style: theme.textTheme.bodyMedium?.copyWith(
            fontWeight: FontWeight.w600,
          ),
        ),
      ],
    );
  }
}
