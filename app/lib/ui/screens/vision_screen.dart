import 'dart:async';
import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/vision_models.dart';

/// Responsive Vision & OCR screen providing offline text extraction,
/// layout and bounding box inspection, and multimodal analysis.
class VisionScreen extends StatefulWidget {
  final AssistantController controller;

  const VisionScreen({super.key, required this.controller});

  @override
  State<VisionScreen> createState() => _VisionScreenState();
}

class _VisionScreenState extends State<VisionScreen> {
  int _selectedSampleIndex = 0;

  static const List<_ImageSample> _samples = <_ImageSample>[
    _ImageSample(
      title: 'Invoice / Receipt',
      description: 'Standard 640x480 document receipt with tabular totals',
      bytes: <int>[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG magic
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x02, 0x80, // width: 640
        0x00, 0x00, 0x01, 0xE0, // height: 480
        0x08, 0x02, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
      ],
    ),
    _ImageSample(
      title: 'Architecture Diagram',
      description: 'System topology blueprint with components and data flows',
      bytes: <int>[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x03, 0x20, // width: 800
        0x00, 0x00, 0x02, 0x58, // height: 600
        0x08, 0x02, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00,
      ],
    ),
    _ImageSample(
      title: 'Terminal Diagnostics',
      description: 'Monochrome console log output snapshot',
      bytes: <int>[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x01, 0xE0, // width: 480
        0x00, 0x00, 0x01, 0x40, // height: 320
        0x08, 0x00, 0x00, 0x00, 0x00, // Grayscale
        0x00, 0x00, 0x00, 0x00,
      ],
    ),
  ];

  void _runOcr() {
    final sample = _samples[_selectedSampleIndex];
    widget.controller.extractOcr(sample.bytes);
  }

  Future<void> _showBrowseFileDialog() async {
    final textController = TextEditingController();
    final result = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Open Local Image or Document'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            const Text(
              'Enter the absolute path to a local .png, .jpg, .jpeg, .webp, .bmp, or .pdf file:',
            ),
            const SizedBox(height: 12),
            TextField(
              key: const Key('ocr_file_path_input'),
              controller: textController,
              decoration: const InputDecoration(
                hintText: '/path/to/document.png',
                border: OutlineInputBorder(),
                prefixIcon: Icon(Icons.insert_drive_file),
              ),
              autofocus: true,
            ),
          ],
        ),
        actions: <Widget>[
          TextButton(
            key: const Key('ocr_cancel_file_button'),
            onPressed: () => Navigator.of(ctx).pop(),
            child: const Text('Cancel'),
          ),
          ElevatedButton(
            key: const Key('ocr_submit_file_button'),
            onPressed: () => Navigator.of(ctx).pop(textController.text.trim()),
            child: const Text('Open & Extract'),
          ),
        ],
      ),
    );

    if (result != null && result.isNotEmpty) {
      await widget.controller.extractOcrFromFile(result);
    }
  }

  Future<void> _pasteFromClipboard() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text?.trim() ?? '';
    if (text.isNotEmpty) {
      await widget.controller.extractOcr(text.codeUnits);
    } else {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('No text/image data found on clipboard'),
            duration: Duration(seconds: 2),
          ),
        );
      }
    }
  }

  void _captureScreen() {
    unawaited(widget.controller.captureScreenAndExtractOcr());
  }

  void _copyToClipboard(String text) {
    Clipboard.setData(ClipboardData(text: text));
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('OCR text copied to clipboard'),
        duration: Duration(seconds: 2),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    return ListenableBuilder(
      listenable: widget.controller,
      builder: (context, _) {
        final ocr = widget.controller.activeOcrResult;
        final isProcessing = widget.controller.isProcessingOcr;
        final error = widget.controller.ocrError;

        return Scaffold(
          appBar: AppBar(
            title: const Text('Offline Vision & OCR'),
            actions: <Widget>[
              if (ocr != null)
                IconButton(
                  key: const Key('ocr_clear_button'),
                  tooltip: 'Clear OCR Result',
                  icon: const Icon(Icons.clear_all),
                  onPressed: widget.controller.clearOcrResult,
                ),
            ],
          ),
          body: ListView(
            padding: const EdgeInsets.all(16.0),
            children: <Widget>[
              _buildHeaderCard(theme),
              const SizedBox(height: 16),
              _buildImageSourceToolbar(theme, isProcessing),
              const SizedBox(height: 16),
              _buildSampleSelectorCard(theme, isProcessing),
              if (isProcessing) ...<Widget>[
                const SizedBox(height: 16),
                const LinearProgressIndicator(key: Key('ocr_progress_indicator')),
              ],
              if (error != null) ...<Widget>[
                const SizedBox(height: 16),
                _buildErrorCard(theme, error),
              ],
              if (ocr != null) ...<Widget>[
                const SizedBox(height: 16),
                _buildMetricsCard(theme, ocr),
                const SizedBox(height: 16),
                _buildBoundingBoxOverlayCard(theme, ocr),
                const SizedBox(height: 16),
                _buildFullTextCard(theme, ocr),
                const SizedBox(height: 16),
                _buildLinesBreakdownCard(theme, ocr),
              ] else if (!isProcessing && error == null) ...<Widget>[
                const SizedBox(height: 32),
                _buildEmptyStateCard(theme),
              ],
            ],
          ),
        );
      },
    );
  }

  Widget _buildHeaderCard(ThemeData theme) {
    return Card(
      elevation: 0,
      color: theme.colorScheme.primaryContainer.withAlpha(50),
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Icon(
                  Icons.document_scanner,
                  color: theme.colorScheme.primary,
                  size: 28,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    'Multimodal Vision & Optical Character Recognition',
                    style: theme.textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            Text(
              'Process images, extract bounding boxes, and analyze text 100% locally on-device. Zero telemetry, zero network dependency, complete cryptographic privacy.',
              style: theme.textTheme.bodyMedium?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 4,
              children: <Widget>[
                Chip(
                  avatar: const Icon(Icons.wifi_off, size: 16),
                  label: const Text('100% Offline'),
                  backgroundColor: theme.colorScheme.surface,
                ),
                Chip(
                  avatar: const Icon(Icons.speed, size: 16),
                  label: const Text('Edge Preprocessing'),
                  backgroundColor: theme.colorScheme.surface,
                ),
                Chip(
                  avatar: const Icon(Icons.lock_outline, size: 16),
                  label: const Text('Local Privacy'),
                  backgroundColor: theme.colorScheme.surface,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildImageSourceToolbar(ThemeData theme, bool isProcessing) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              'Image Source & Local Input',
              style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 8),
            Text(
              'Load an image or document from disk, paste text or clipboard bytes, or capture an instant desktop screen snapshot.',
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: <Widget>[
                ElevatedButton.icon(
                  key: const Key('ocr_browse_button'),
                  icon: const Icon(Icons.folder_open_outlined, size: 18),
                  label: const Text('Browse Image...'),
                  onPressed: isProcessing ? null : _showBrowseFileDialog,
                ),
                OutlinedButton.icon(
                  key: const Key('ocr_paste_button'),
                  icon: const Icon(Icons.paste_outlined, size: 18),
                  label: const Text('Paste from Clipboard'),
                  onPressed: isProcessing ? null : _pasteFromClipboard,
                ),
                OutlinedButton.icon(
                  key: const Key('ocr_capture_screen_button'),
                  icon: const Icon(Icons.screenshot_outlined, size: 18),
                  label: const Text('Capture Screen'),
                  onPressed: isProcessing ? null : _captureScreen,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildSampleSelectorCard(ThemeData theme, bool isProcessing) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              'Select Input Sample',
              style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 12),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: List<Widget>.generate(_samples.length, (idx) {
                final sample = _samples[idx];
                final isSelected = _selectedSampleIndex == idx;
                return ChoiceChip(
                  key: Key('sample_chip_$idx'),
                  label: Text(sample.title),
                  selected: isSelected,
                  onSelected: isProcessing
                      ? null
                      : (val) {
                          if (val) {
                            setState(() => _selectedSampleIndex = idx);
                          }
                        },
                );
              }),
            ),
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.all(12),
              decoration: BoxDecoration(
                color: theme.colorScheme.surfaceContainerHighest.withAlpha(100),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Row(
                children: <Widget>[
                  Icon(Icons.info_outline, size: 20, color: theme.colorScheme.primary),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      _samples[_selectedSampleIndex].description,
                      style: theme.textTheme.bodySmall,
                    ),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 16),
            SizedBox(
              width: double.infinity,
              child: ElevatedButton.icon(
                key: const Key('ocr_extract_button'),
                icon: const Icon(Icons.text_snippet_outlined),
                label: Text(isProcessing ? 'Processing OCR...' : 'Extract OCR Text'),
                onPressed: isProcessing ? null : _runOcr,
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildErrorCard(ThemeData theme, String error) {
    return Card(
      color: theme.colorScheme.errorContainer,
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Row(
          children: <Widget>[
            Icon(Icons.error_outline, color: theme.colorScheme.error),
            const SizedBox(width: 12),
            Expanded(
              child: Text(
                error,
                style: TextStyle(color: theme.colorScheme.onErrorContainer),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildMetricsCard(ThemeData theme, OcrDocumentItem ocr) {
    final confPercent = (ocr.averageConfidence * 100).toStringAsFixed(1);

    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              'Extraction Metrics',
              style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 12),
            Row(
              children: <Widget>[
                _buildMetricColumn(
                  theme,
                  keyStr: 'ocr_confidence_metric',
                  label: 'Avg Confidence',
                  value: '$confPercent%',
                  icon: Icons.verified_outlined,
                  color: ocr.averageConfidence >= 0.9 ? Colors.green : Colors.orange,
                ),
                _buildMetricColumn(
                  theme,
                  keyStr: 'ocr_lines_metric',
                  label: 'Lines Detected',
                  value: '${ocr.lines.length}',
                  icon: Icons.format_list_numbered,
                  color: theme.colorScheme.primary,
                ),
                _buildMetricColumn(
                  theme,
                  keyStr: 'ocr_time_metric',
                  label: 'Latency',
                  value: '${ocr.processingTimeMs} ms',
                  icon: Icons.timer_outlined,
                  color: theme.colorScheme.secondary,
                ),
                _buildMetricColumn(
                  theme,
                  keyStr: 'ocr_dims_metric',
                  label: 'Dimensions',
                  value: '${ocr.imageWidth}x${ocr.imageHeight}',
                  icon: Icons.aspect_ratio,
                  color: theme.colorScheme.tertiary,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildMetricColumn(
    ThemeData theme, {
    required String keyStr,
    required String label,
    required String value,
    required IconData icon,
    required Color color,
  }) {
    return Expanded(
      child: Column(
        key: Key(keyStr),
        children: <Widget>[
          Icon(icon, color: color, size: 22),
          const SizedBox(height: 4),
          Text(
            value,
            style: theme.textTheme.titleMedium?.copyWith(
              fontWeight: FontWeight.bold,
              color: color,
            ),
          ),
          const SizedBox(height: 2),
          Text(
            label,
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
            textAlign: TextAlign.center,
          ),
        ],
      ),
    );
  }

  Widget _buildBoundingBoxOverlayCard(ThemeData theme, OcrDocumentItem ocr) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: <Widget>[
                Text(
                  'Visual Detection Overlay & Bounding Boxes',
                  style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
                ),
                Chip(
                  visualDensity: VisualDensity.compact,
                  avatar: const Icon(Icons.aspect_ratio, size: 14),
                  label: Text('${ocr.imageWidth} × ${ocr.imageHeight} px'),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Container(
              height: 220,
              width: double.infinity,
              decoration: BoxDecoration(
                color: const Color(0xFF0F172A),
                borderRadius: BorderRadius.circular(8),
                border: Border.all(color: theme.colorScheme.outlineVariant),
              ),
              child: ClipRRect(
                borderRadius: BorderRadius.circular(8),
                child: LayoutBuilder(
                  builder: (context, constraints) {
                    final w = constraints.maxWidth;
                    final h = constraints.maxHeight;

                    return Stack(
                      children: <Widget>[
                        Positioned.fill(
                          child: CustomPaint(
                            painter: _BlueprintGridPainter(),
                          ),
                        ),
                        for (final line in ocr.lines)
                          Positioned(
                            left: (line.bbox.x * w).clamp(0.0, (w - 60.0).clamp(0.0, w)),
                            top: (line.bbox.y * h).clamp(0.0, (h - 26.0).clamp(0.0, h)),
                            width: (line.bbox.width * w).clamp(40.0, w),
                            height: (line.bbox.height * h).clamp(24.0, h),
                            child: Container(
                              key: Key('ocr_bbox_box_${line.lineNumber}'),
                              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                              decoration: BoxDecoration(
                                color: theme.colorScheme.primary.withAlpha(50),
                                border: Border.all(
                                  color: theme.colorScheme.primary,
                                  width: 1.5,
                                ),
                                borderRadius: BorderRadius.circular(4),
                              ),
                              child: Text(
                                'L${line.lineNumber}: ${line.text}',
                                style: const TextStyle(
                                  color: Colors.white,
                                  fontSize: 10,
                                  fontWeight: FontWeight.bold,
                                ),
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                          ),
                      ],
                    );
                  },
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildFullTextCard(ThemeData theme, OcrDocumentItem ocr) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: <Widget>[
                Text(
                  'Recognized Full Text',
                  style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
                ),
                Wrap(
                  spacing: 4,
                  children: <Widget>[
                    TextButton.icon(
                      key: const Key('ocr_copy_button'),
                      icon: const Icon(Icons.copy, size: 16),
                      label: const Text('Copy Text'),
                      onPressed: () => _copyToClipboard(ocr.fullText),
                    ),
                    IconButton(
                      key: const Key('ocr_copy_all_button'),
                      tooltip: 'Copy All',
                      icon: const Icon(Icons.content_copy, size: 16),
                      onPressed: () => _copyToClipboard(ocr.fullText),
                    ),
                    IconButton(
                      key: const Key('ocr_export_button'),
                      tooltip: 'Export as JSON',
                      icon: const Icon(Icons.download, size: 16),
                      onPressed: () {
                        Clipboard.setData(ClipboardData(text: jsonEncode(ocr.toJson())));
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Exported OCR document JSON to clipboard'),
                            duration: Duration(seconds: 2),
                          ),
                        );
                      },
                    ),
                  ],
                ),
              ],
            ),
            const Divider(),
            const SizedBox(height: 8),
            SelectableText(
              key: const Key('ocr_full_text_view'),
              ocr.fullText.isNotEmpty ? ocr.fullText : '(No text recognized)',
              style: theme.textTheme.bodyMedium?.copyWith(
                fontFamily: 'monospace',
                height: 1.5,
              ),
            ),
            const SizedBox(height: 16),
            SizedBox(
              width: double.infinity,
              child: ElevatedButton.icon(
                key: const Key('ocr_ask_assistant_button'),
                icon: const Icon(Icons.chat_outlined),
                label: const Text('Ask Assistant About This Image'),
                onPressed: () {
                  widget.controller.askAssistantAboutOcrText(ocr.fullText);
                  ScaffoldMessenger.of(context).showSnackBar(
                    const SnackBar(
                      content: Text('OCR prompt sent to Assistant Chat'),
                      duration: Duration(seconds: 2),
                    ),
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildLinesBreakdownCard(ThemeData theme, OcrDocumentItem ocr) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              'Layout & Bounding Box Breakdown',
              style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 12),
            if (ocr.lines.isEmpty)
              const Text('No line segments detected.')
            else
              ...ocr.lines.map((line) => _buildLineRow(theme, line)),
          ],
        ),
      ),
    );
  }

  Widget _buildLineRow(ThemeData theme, OcrTextLineItem line) {
    final bbox = line.bbox;
    final bboxStr =
        '[${bbox.x.toStringAsFixed(2)}, ${bbox.y.toStringAsFixed(2)}, ${bbox.width.toStringAsFixed(2)}, ${bbox.height.toStringAsFixed(2)}]';
    final confPercent = (line.confidence * 100).toStringAsFixed(0);

    return Container(
      margin: const EdgeInsets.only(bottom: 8.0),
      padding: const EdgeInsets.all(10.0),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest.withAlpha(60),
        borderRadius: BorderRadius.circular(8.0),
        border: Border.all(
          color: theme.colorScheme.outlineVariant.withAlpha(80),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Container(
                padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
                decoration: BoxDecoration(
                  color: theme.colorScheme.primaryContainer,
                  borderRadius: BorderRadius.circular(4),
                ),
                child: Text(
                  'L${line.lineNumber}',
                  style: theme.textTheme.labelSmall?.copyWith(
                    fontWeight: FontWeight.bold,
                    color: theme.colorScheme.onPrimaryContainer,
                  ),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: Text(
                  line.text,
                  style: theme.textTheme.bodyMedium?.copyWith(
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
              Chip(
                padding: EdgeInsets.zero,
                labelPadding: const EdgeInsets.symmetric(horizontal: 6),
                label: Text(
                  '$confPercent%',
                  style: theme.textTheme.labelSmall,
                ),
                backgroundColor: line.confidence >= 0.95
                    ? Colors.green.withAlpha(40)
                    : Colors.amber.withAlpha(40),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Row(
            children: <Widget>[
              Icon(Icons.crop_free, size: 14, color: theme.colorScheme.onSurfaceVariant),
              const SizedBox(width: 4),
              Text(
                'Box: $bboxStr',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                  fontFamily: 'monospace',
                ),
              ),
              const SizedBox(width: 12),
              Icon(Icons.short_text, size: 14, color: theme.colorScheme.onSurfaceVariant),
              const SizedBox(width: 4),
              Text(
                '${line.words.length} words',
                style: theme.textTheme.labelSmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildEmptyStateCard(ThemeData theme) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: <Widget>[
          Icon(
            Icons.image_search,
            size: 64,
            color: theme.colorScheme.outline,
          ),
          const SizedBox(height: 12),
          Text(
            'Ready to Analyze Images',
            style: theme.textTheme.titleMedium?.copyWith(
              color: theme.colorScheme.outline,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            'Select an image sample above and click "Extract OCR Text"',
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.outline,
            ),
          ),
        ],
      ),
    );
  }
}

class _ImageSample {
  final String title;
  final String description;
  final List<int> bytes;

  const _ImageSample({
    required this.title,
    required this.description,
    required this.bytes,
  });
}

class _BlueprintGridPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = const Color(0xFF1E293B)
      ..strokeWidth = 1.0;

    const step = 20.0;
    for (double x = 0; x <= size.width; x += step) {
      canvas.drawLine(Offset(x, 0), Offset(x, size.height), paint);
    }
    for (double y = 0; y <= size.height; y += step) {
      canvas.drawLine(Offset(0, y), Offset(size.width, y), paint);
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}
