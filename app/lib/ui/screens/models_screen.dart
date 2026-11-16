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
  void initState() {
    super.initState();
    widget.controller.addListener(_onControllerChanged);
  }

  @override
  void dispose() {
    widget.controller.removeListener(_onControllerChanged);
    super.dispose();
  }

  void _onControllerChanged() {
    final err = widget.controller.errorMessage;
    if (err != null && mounted) {
      widget.controller.clearError();
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(err),
          backgroundColor: Colors.red.shade800,
          behavior: SnackBarBehavior.floating,
        ),
      );
    }
  }

  void _showImportLocalDialog(BuildContext context) {
    final textController = TextEditingController();
    showDialog<void>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Import Local .GGUF Model'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            const Text(
              'Enter the absolute path to a .gguf model file on your local disk:',
            ),
            const SizedBox(height: 12),
            TextField(
              controller: textController,
              decoration: const InputDecoration(
                hintText: 'e.g. C:/models/custom-model.gguf',
                border: OutlineInputBorder(),
                prefixIcon: Icon(Icons.folder_open),
              ),
            ),
          ],
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () {
              final path = textController.text.trim();
              if (path.isNotEmpty) {
                widget.controller.importLocalGguf(path);
                Navigator.of(ctx).pop();
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text('Importing model from $path...')),
                );
              }
            },
            child: const Text('Import'),
          ),
        ],
      ),
    );
  }

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
                key: const Key('appbar_run_benchmark_button'),
                tooltip: activeModel == null
                    ? 'Load a model to run benchmarks'
                    : 'Run Inference Benchmark',
                icon: isBenchmarking
                    ? const SizedBox(
                        width: 18,
                        height: 18,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.speed),
                onPressed: (activeModel == null || isBenchmarking)
                    ? null
                    : () => widget.controller.runBenchmark(),
              ),
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
                activeModel: activeModel,
                benchmark: benchmark,
                isBenchmarking: isBenchmarking,
                onRunBenchmark: () => widget.controller.runBenchmark(),
              ),

              const SizedBox(height: 24),

              // Filter Segmented Buttons & Section Title & Import Local GGUF
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
                  Wrap(
                    spacing: 8,
                    runSpacing: 8,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: <Widget>[
                      OutlinedButton.icon(
                        key: const Key('import_local_gguf_button'),
                        onPressed: () => _showImportLocalDialog(context),
                        icon: const Icon(Icons.file_open_outlined, size: 16),
                        label: const Text('Import Local .GGUF'),
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
                    onDownload: () => widget.controller.downloadModel(model.id),
                    onCancelDownload: () => widget.controller.cancelDownload(model.id),
                    onDelete: () => widget.controller.deleteModel(model.id),
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
  final ModelItem? activeModel;
  final BenchmarkResult? benchmark;
  final bool isBenchmarking;
  final VoidCallback onRunBenchmark;

  const _BenchmarkCard({
    required this.activeModel,
    required this.benchmark,
    required this.isBenchmarking,
    required this.onRunBenchmark,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final isModelLoaded = activeModel != null;

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
                  onPressed: (!isModelLoaded || isBenchmarking) ? null : onRunBenchmark,
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
            if (!isModelLoaded) ...<Widget>[
              const SizedBox(height: 16),
              Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.5),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Row(
                  children: <Widget>[
                    Icon(Icons.info_outline, size: 20, color: theme.colorScheme.secondary),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        'No Active Model Loaded — Download or Load a model below to run benchmarks',
                        style: theme.textTheme.bodyMedium?.copyWith(
                          color: theme.colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ] else if (benchmark != null) ...<Widget>[
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
  final VoidCallback onDownload;
  final VoidCallback onCancelDownload;
  final VoidCallback onDelete;

  const _ModelCard({
    required this.model,
    required this.isActive,
    required this.onLoad,
    required this.onUnload,
    required this.onDownload,
    required this.onCancelDownload,
    required this.onDelete,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final status = isActive ? ModelLifecycleStatus.loadedActive : model.lifecycleState.status;

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
                          if (status == ModelLifecycleStatus.loadedActive)
                            Container(
                              padding: const EdgeInsets.symmetric(
                                  horizontal: 6, vertical: 2),
                              decoration: BoxDecoration(
                                color: Colors.green.withValues(alpha: 0.15),
                                borderRadius: BorderRadius.circular(8),
                              ),
                              child: Text(
                                'Active / Loaded',
                                style: theme.textTheme.labelSmall?.copyWith(
                                  color: Colors.green.shade700,
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
                _buildActionButton(context, status),
              ],
            ),
            if (status == ModelLifecycleStatus.downloading) ...<Widget>[
              const SizedBox(height: 12),
              ClipRRect(
                borderRadius: BorderRadius.circular(4),
                child: LinearProgressIndicator(
                  value: model.lifecycleState.progressPercentage > 0
                      ? model.lifecycleState.progressPercentage
                      : null,
                  minHeight: 6,
                ),
              ),
              const SizedBox(height: 4),
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: <Widget>[
                  Text(
                    '${(model.lifecycleState.progressPercentage * 100).toStringAsFixed(1)}% '
                    '(${model.lifecycleState.speedFormatted})',
                    style: theme.textTheme.labelSmall?.copyWith(color: Colors.grey.shade600),
                  ),
                  TextButton.icon(
                    onPressed: onCancelDownload,
                    icon: const Icon(Icons.close, size: 14),
                    label: const Text('Cancel'),
                    style: TextButton.styleFrom(
                      padding: const EdgeInsets.symmetric(horizontal: 8),
                      visualDensity: VisualDensity.compact,
                    ),
                  ),
                ],
              ),
            ],
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
                if ((status == ModelLifecycleStatus.downloadedOnDisk &&
                        model.lifecycleState.sha256Verified) ||
                    status == ModelLifecycleStatus.loadedActive)
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

  Widget _buildActionButton(BuildContext context, ModelLifecycleStatus status) {
    switch (status) {
      case ModelLifecycleStatus.loadedActive:
        return FilledButton.tonalIcon(
          onPressed: onUnload,
          icon: const Icon(Icons.check, size: 16),
          label: const Text('Loaded'),
        );

      case ModelLifecycleStatus.loadingIntoMemory:
        return const FilledButton.tonal(
          onPressed: null,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              SizedBox(
                width: 14,
                height: 14,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              SizedBox(width: 8),
              Text('Loading...'),
            ],
          ),
        );

      case ModelLifecycleStatus.downloading:
        return OutlinedButton.icon(
          onPressed: onCancelDownload,
          icon: const Icon(Icons.close, size: 16),
          label: const Text('Cancel'),
        );

      case ModelLifecycleStatus.verifyingHash:
        return const OutlinedButton(
          onPressed: null,
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              SizedBox(
                width: 14,
                height: 14,
                child: CircularProgressIndicator(strokeWidth: 2),
              ),
              SizedBox(width: 8),
              Text('Verifying...'),
            ],
          ),
        );

      case ModelLifecycleStatus.downloadedOnDisk:
        return Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            FilledButton.icon(
              key: Key('load_button_${model.id}'),
              onPressed: onLoad,
              icon: const Icon(Icons.memory, size: 16),
              label: const Text('Load'),
            ),
            const SizedBox(width: 4),
            IconButton(
              icon: const Icon(Icons.delete_outline, size: 20),
              tooltip: 'Delete weights from disk',
              onPressed: onDelete,
            ),
          ],
        );

      case ModelLifecycleStatus.error:
        return FilledButton.icon(
          onPressed: onDownload,
          icon: const Icon(Icons.refresh, size: 16),
          label: const Text('Retry'),
        );

      case ModelLifecycleStatus.notDownloaded:
        return FilledButton.icon(
          key: Key('download_button_${model.id}'),
          onPressed: onDownload,
          icon: const Icon(Icons.download, size: 16),
          label: Text('Download (${model.fileSizeFormatted})'),
        );
    }
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
