import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/interpreter_models.dart';

/// Responsive Live Voice Interpreter screen facilitating bi-directional alternating
/// conversations between two participants in different languages.
class InterpreterScreen extends StatefulWidget {
  final AssistantController controller;

  const InterpreterScreen({super.key, required this.controller});

  @override
  State<InterpreterScreen> createState() => _InterpreterScreenState();
}

class _InterpreterScreenState extends State<InterpreterScreen> {
  final TextEditingController _inputController = TextEditingController();

  static const List<String> _quickPhrases = <String>[
    'Hello',
    'How are you?',
    'Thank you',
    'Where is the station?',
    'I need assistance',
  ];

  @override
  void dispose() {
    _inputController.dispose();
    super.dispose();
  }

  void _submitTurn() {
    final text = _inputController.text.trim();
    if (text.isNotEmpty) {
      widget.controller.submitInterpreterTurn(text);
      _inputController.clear();
    }
  }

  void _submitQuickPhrase(String phrase) {
    widget.controller.submitInterpreterTurn(phrase);
  }

  void _copyToClipboard(String text) {
    Clipboard.setData(ClipboardData(text: text));
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(
        content: Text('Translated text copied to clipboard'),
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
        final status = widget.controller.interpreterStatus;
        final turns = widget.controller.interpreterTurns;
        final isProcessing = widget.controller.isProcessingInterpreterTurn;
        final error = widget.controller.interpreterError;

        final partyACode = status?.partyALang ?? 'en';
        final partyBCode = status?.partyBLang ?? 'es';
        final langA = LanguageItem.findByCode(partyACode);
        final langB = LanguageItem.findByCode(partyBCode);
        final isPartyAActive = status?.isPartyAActive ?? true;

        return Scaffold(
          appBar: AppBar(
            title: const Text('Live Voice Interpreter'),
            actions: <Widget>[
              if (turns.isNotEmpty)
                IconButton(
                  key: const Key('interpreter_clear_button'),
                  tooltip: 'Clear Conversation',
                  icon: const Icon(Icons.delete_sweep_outlined),
                  onPressed: widget.controller.clearInterpreterSession,
                ),
            ],
          ),
          body: Column(
            children: <Widget>[
              Expanded(
                child: ListView(
                  padding: const EdgeInsets.all(16.0),
                  children: <Widget>[
                    _buildHeaderCard(theme),
                    const SizedBox(height: 16),
                    _buildLanguageConfigCard(theme, langA, langB, isPartyAActive),
                    if (isProcessing) ...<Widget>[
                      const SizedBox(height: 12),
                      const LinearProgressIndicator(
                        key: Key('interpreter_progress_indicator'),
                      ),
                    ],
                    if (error != null) ...<Widget>[
                      const SizedBox(height: 12),
                      _buildErrorCard(theme, error),
                    ],
                    const SizedBox(height: 16),
                    _buildTurnsHeader(theme, turns.length),
                    const SizedBox(height: 8),
                    if (turns.isEmpty)
                      _buildEmptyState(theme)
                    else
                      ...turns.reversed.map((turn) => _buildTurnCard(theme, turn)),
                  ],
                ),
              ),
              _buildInputArea(theme, isProcessing, isPartyAActive, langA, langB),
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
                  Icons.translate,
                  color: theme.colorScheme.primary,
                  size: 28,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    'Offline Bi-Directional Voice Interpreter',
                    style: theme.textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            Text(
              'Face-to-face live interpretation with automatic alternating speaker detection. 100% offline edge inference with zero telemetry.',
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
                  avatar: const Icon(Icons.record_voice_over, size: 16),
                  label: const Text('Live Alternating Mode'),
                  backgroundColor: theme.colorScheme.surface,
                ),
                Chip(
                  avatar: const Icon(Icons.lock_outline, size: 16),
                  label: const Text('Zero Cloud Transfer'),
                  backgroundColor: theme.colorScheme.surface,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildLanguageConfigCard(
    ThemeData theme,
    LanguageItem langA,
    LanguageItem langB,
    bool isPartyAActive,
  ) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: _buildParticipantTag(
                    theme,
                    title: 'Party A',
                    lang: langA,
                    isActive: isPartyAActive,
                  ),
                ),
                IconButton.filledTonal(
                  key: const Key('interpreter_swap_button'),
                  tooltip: 'Swap Languages',
                  icon: const Icon(Icons.swap_horiz),
                  onPressed: widget.controller.swapInterpreterLanguages,
                ),
                Expanded(
                  child: _buildParticipantTag(
                    theme,
                    title: 'Party B',
                    lang: langB,
                    isActive: !isPartyAActive,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 12),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              decoration: BoxDecoration(
                color: isPartyAActive
                    ? theme.colorScheme.primaryContainer.withAlpha(80)
                    : theme.colorScheme.secondaryContainer.withAlpha(80),
                borderRadius: BorderRadius.circular(8),
              ),
              child: Row(
                children: <Widget>[
                  Icon(
                    Icons.mic,
                    size: 20,
                    color: isPartyAActive
                        ? theme.colorScheme.primary
                        : theme.colorScheme.secondary,
                  ),
                  const SizedBox(width: 8),
                  Text(
                    isPartyAActive
                        ? "Party A's Turn (${langA.name} -> ${langB.name})"
                        : "Party B's Turn (${langB.name} -> ${langA.name})",
                    style: theme.textTheme.bodyMedium?.copyWith(
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildParticipantTag(
    ThemeData theme, {
    required String title,
    required LanguageItem lang,
    required bool isActive,
  }) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: isActive
            ? theme.colorScheme.primaryContainer.withAlpha(120)
            : theme.colorScheme.surfaceContainerHighest.withAlpha(60),
        borderRadius: BorderRadius.circular(8),
        border: Border.all(
          color: isActive ? theme.colorScheme.primary : Colors.transparent,
          width: 1.5,
        ),
      ),
      child: Column(
        children: <Widget>[
          Text(
            title,
            style: theme.textTheme.labelSmall?.copyWith(
              color: theme.colorScheme.onSurfaceVariant,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            lang.nativeName,
            style: theme.textTheme.titleSmall?.copyWith(
              fontWeight: FontWeight.bold,
            ),
            textAlign: TextAlign.center,
          ),
          Text(
            lang.name,
            style: theme.textTheme.bodySmall?.copyWith(
              color: theme.colorScheme.outline,
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildErrorCard(ThemeData theme, String error) {
    return Card(
      color: theme.colorScheme.errorContainer,
      child: Padding(
        padding: const EdgeInsets.all(12.0),
        child: Row(
          children: <Widget>[
            Icon(Icons.error_outline, color: theme.colorScheme.error),
            const SizedBox(width: 8),
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

  Widget _buildTurnsHeader(ThemeData theme, int count) {
    return Row(
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: <Widget>[
        Text(
          'Conversation Transcript ($count)',
          style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
        ),
      ],
    );
  }

  Widget _buildTurnCard(ThemeData theme, InterpreterTurnItem turn) {
    final isPartyA = turn.isPartyA;
    final confPercent = (turn.confidence * 100).toStringAsFixed(0);

    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(14.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                CircleAvatar(
                  radius: 12,
                  backgroundColor: isPartyA
                      ? theme.colorScheme.primary
                      : theme.colorScheme.secondary,
                  child: Text(
                    isPartyA ? 'A' : 'B',
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 12,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  isPartyA ? 'Party A' : 'Party B',
                  style: theme.textTheme.bodyMedium?.copyWith(
                    fontWeight: FontWeight.bold,
                  ),
                ),
                const SizedBox(width: 8),
                Text(
                  '${turn.sourceLang.toUpperCase()} -> ${turn.targetLang.toUpperCase()}',
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: theme.colorScheme.outline,
                  ),
                ),
                const Spacer(),
                Chip(
                  padding: EdgeInsets.zero,
                  labelPadding: const EdgeInsets.symmetric(horizontal: 6),
                  label: Text('$confPercent%'),
                  backgroundColor: turn.confidence >= 0.95
                      ? Colors.green.withAlpha(40)
                      : Colors.orange.withAlpha(40),
                ),
              ],
            ),
            const SizedBox(height: 8),
            Text(
              turn.sourceText,
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.onSurfaceVariant,
                fontStyle: FontStyle.italic,
              ),
            ),
            const SizedBox(height: 6),
            Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Expanded(
                  child: Text(
                    turn.translatedText,
                    style: theme.textTheme.titleMedium?.copyWith(
                      fontWeight: FontWeight.bold,
                      color: theme.colorScheme.primary,
                    ),
                  ),
                ),
                IconButton(
                  key: Key('copy_turn_${turn.turnId}'),
                  icon: const Icon(Icons.copy, size: 18),
                  tooltip: 'Copy translation',
                  onPressed: () => _copyToClipboard(turn.translatedText),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildEmptyState(ThemeData theme) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 32.0),
        child: Column(
          children: <Widget>[
            Icon(
              Icons.forum_outlined,
              size: 56,
              color: theme.colorScheme.outline,
            ),
            const SizedBox(height: 8),
            Text(
              'No Conversation Turns Yet',
              style: theme.textTheme.titleSmall?.copyWith(
                color: theme.colorScheme.outline,
              ),
            ),
            const SizedBox(height: 4),
            Text(
              'Type or speak a phrase below to begin live interpretation.',
              style: theme.textTheme.bodySmall?.copyWith(
                color: theme.colorScheme.outline,
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildInputArea(
    ThemeData theme,
    bool isProcessing,
    bool isPartyAActive,
    LanguageItem langA,
    LanguageItem langB,
  ) {
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: theme.colorScheme.surface,
        border: Border(
          top: BorderSide(color: theme.colorScheme.outlineVariant.withAlpha(80)),
        ),
      ),
      child: SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            SingleChildScrollView(
              scrollDirection: Axis.horizontal,
              child: Row(
                children: _quickPhrases.map((phrase) {
                  return Padding(
                    padding: const EdgeInsets.only(right: 8.0),
                    child: ActionChip(
                      label: Text(phrase),
                      onPressed: isProcessing ? null : () => _submitQuickPhrase(phrase),
                    ),
                  );
                }).toList(),
              ),
            ),
            const SizedBox(height: 8),
            Row(
              children: <Widget>[
                Expanded(
                  child: TextField(
                    key: const Key('interpreter_input_field'),
                    controller: _inputController,
                    decoration: InputDecoration(
                      hintText: isPartyAActive
                          ? 'Speak or type in ${langA.name}...'
                          : 'Speak or type in ${langB.name}...',
                      border: const OutlineInputBorder(),
                      contentPadding: const EdgeInsets.symmetric(
                        horizontal: 14,
                        vertical: 12,
                      ),
                    ),
                    onSubmitted: isProcessing ? null : (_) => _submitTurn(),
                  ),
                ),
                const SizedBox(width: 8),
                IconButton.filled(
                  key: const Key('interpreter_submit_button'),
                  icon: const Icon(Icons.send),
                  tooltip: 'Submit Turn',
                  onPressed: isProcessing ? null : _submitTurn,
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
