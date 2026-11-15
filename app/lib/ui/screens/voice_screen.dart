import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/voice_state.dart';

/// Voice interaction screen providing real-time waveform visual feedback,
/// push-to-talk gesture control, hands-free wake-word spotting, and live transcript streaming.
class VoiceScreen extends StatelessWidget {
  final AssistantController controller;

  const VoiceScreen({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, _) {
        final voiceState = controller.voiceState;
        final theme = Theme.of(context);
        final isListening = voiceState.isListening;
        final isWakeMode = voiceState.mode == VoiceMode.wakeWord;

        return Scaffold(
          appBar: AppBar(
            title: const Text('Voice Assistant'),
            actions: <Widget>[
              // Privacy and offline assurance badge
              Padding(
                padding: const EdgeInsets.only(right: 16.0),
                child: Tooltip(
                  message: 'Zero cloud transit: audio is processed in memory and never persisted',
                  child: Chip(
                    avatar: const Icon(Icons.shield_outlined, size: 16),
                    label: const Text('100% Offline'),
                    backgroundColor: theme.colorScheme.surfaceContainerHighest,
                    side: BorderSide.none,
                  ),
                ),
              ),
            ],
          ),
          body: LayoutBuilder(
            builder: (context, constraints) {
              return SingleChildScrollView(
                padding: const EdgeInsets.all(24.0),
                child: Center(
                  child: ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 800),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: <Widget>[
                        // Status Card & Privacy Indicator
                        _VoiceStatusHeader(
                          voiceState: voiceState,
                          isListening: isListening,
                          isWakeMode: isWakeMode,
                        ),
                        const SizedBox(height: 24),

                        // Mode Selector (Push-to-Talk vs Wake-Word)
                        _VoiceModeSelector(
                          voiceState: voiceState,
                          onModeChanged: (mode) => controller.setVoiceMode(mode),
                        ),
                        const SizedBox(height: 28),

                        // Dynamic Waveform Feedback Visualizer
                        Center(
                          child: WaveformVisualizer(
                            rmsDbfs: voiceState.rmsDbfs,
                            peakDbfs: voiceState.peakDbfs,
                            isListening: isListening,
                          ),
                        ),
                        const SizedBox(height: 28),

                        // Push-to-Talk & Action Controls
                        Center(
                          child: _VoiceActionButton(
                            voiceState: voiceState,
                            onStart: () => controller.startVoiceListening(),
                            onStop: () => controller.stopVoiceListening(),
                          ),
                        ),
                        const SizedBox(height: 12),
                        Center(
                          child: Text(
                            isWakeMode
                                ? (isListening
                                    ? 'Listening for keywords...'
                                    : 'Tap microphone to start keyword standby')
                                : (isListening
                                    ? 'Listening... Release or tap to finish'
                                    : 'Hold or tap microphone to speak'),
                            style: theme.textTheme.bodyMedium?.copyWith(
                              color: theme.colorScheme.onSurfaceVariant,
                            ),
                          ),
                        ),
                        const SizedBox(height: 28),

                        // Wake-word keywords configuration
                        if (isWakeMode) ...<Widget>[
                          _KeywordSelector(
                            activeKeywords: voiceState.activeKeywords,
                            onKeywordsChanged: (kws) => controller.setVoiceKeywords(kws),
                          ),
                          const SizedBox(height: 24),
                        ],

                        // Live Streaming Transcript Card
                        _TranscriptCard(
                          voiceState: voiceState,
                          onClear: controller.clearTranscript,
                        ),
                      ],
                    ),
                  ),
                ),
              );
            },
          ),
        );
      },
    );
  }
}

/// Header card displaying real-time audio status and offline safety guarantees.
class _VoiceStatusHeader extends StatelessWidget {
  final VoiceState voiceState;
  final bool isListening;
  final bool isWakeMode;

  const _VoiceStatusHeader({
    required this.voiceState,
    required this.isListening,
    required this.isWakeMode,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);

    final Color statusColor;
    final String statusLabel;
    final IconData statusIcon;

    if (isListening) {
      statusColor = isWakeMode ? Colors.teal : theme.colorScheme.primary;
      statusLabel = isWakeMode ? 'Wake-Word Standby Active' : 'Recording & Transcribing';
      statusIcon = isWakeMode ? Icons.hearing : Icons.mic;
    } else {
      statusColor = theme.colorScheme.outline;
      statusLabel = 'Microphone Idle';
      statusIcon = Icons.mic_off_outlined;
    }

    return Card(
      elevation: 0,
      color: theme.colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(
          color: isListening ? statusColor.withValues(alpha: 0.5) : theme.colorScheme.outlineVariant,
          width: isListening ? 1.5 : 1.0,
        ),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Row(
          children: <Widget>[
            Container(
              padding: const EdgeInsets.all(10),
              decoration: BoxDecoration(
                color: statusColor.withValues(alpha: 0.15),
                shape: BoxShape.circle,
              ),
              child: Icon(statusIcon, color: statusColor, size: 24),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(
                    statusLabel,
                    style: theme.textTheme.titleSmall?.copyWith(
                      fontWeight: FontWeight.bold,
                      color: isListening ? statusColor : theme.colorScheme.onSurface,
                    ),
                  ),
                  const SizedBox(height: 2),
                  Text(
                    isWakeMode
                        ? 'Spotting on volatile audio frames (zero disk logging)'
                        : 'Local STT decoding directly on device CPU/NPU',
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                ],
              ),
            ),
            if (voiceState.lastSpottedKeyword != null) ...<Widget>[
              const SizedBox(width: 8),
              Chip(
                avatar: const Icon(Icons.bolt, size: 16, color: Colors.amber),
                label: Text(
                  voiceState.lastSpottedKeyword!,
                  style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 12),
                ),
                backgroundColor: theme.colorScheme.surfaceContainerHighest,
              ),
            ],
          ],
        ),
      ),
    );
  }
}

/// Segmented mode selector toggling between Push-to-Talk and Wake-Word modes.
class _VoiceModeSelector extends StatelessWidget {
  final VoiceState voiceState;
  final ValueChanged<VoiceMode> onModeChanged;

  const _VoiceModeSelector({
    required this.voiceState,
    required this.onModeChanged,
  });

  @override
  Widget build(BuildContext context) {
    return SegmentedButton<VoiceMode>(
      segments: const <ButtonSegment<VoiceMode>>[
        ButtonSegment<VoiceMode>(
          value: VoiceMode.pushToTalk,
          icon: Icon(Icons.touch_app_outlined),
          label: Text('Push-to-Talk'),
        ),
        ButtonSegment<VoiceMode>(
          value: VoiceMode.wakeWord,
          icon: Icon(Icons.record_voice_over_outlined),
          label: Text('Hands-Free Wake Word'),
        ),
      ],
      selected: <VoiceMode>{voiceState.mode},
      onSelectionChanged: (newSelection) {
        if (newSelection.isNotEmpty) {
          onModeChanged(newSelection.first);
        }
      },
    );
  }
}

/// Dynamic audio level waveform visualizer displaying live RMS and peak feedback.
class WaveformVisualizer extends StatelessWidget {
  final double rmsDbfs;
  final double peakDbfs;
  final bool isListening;

  const WaveformVisualizer({
    super.key,
    required this.rmsDbfs,
    required this.peakDbfs,
    required this.isListening,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final primary = theme.colorScheme.primary;
    final secondary = theme.colorScheme.secondary;

    // Normalize dBFS (-96 dBFS silence, 0 dBFS max) to [0.0, 1.0]
    final normalizedRms = isListening
        ? ((rmsDbfs + 96.0) / 96.0).clamp(0.08, 1.0)
        : 0.08;
    final normalizedPeak = isListening
        ? ((peakDbfs + 96.0) / 96.0).clamp(0.1, 1.0)
        : 0.1;

    return Container(
      width: 240,
      height: 96,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerLow,
        borderRadius: BorderRadius.circular(24),
        border: Border.all(
          color: isListening ? primary.withValues(alpha: 0.3) : theme.colorScheme.outlineVariant,
        ),
      ),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.spaceEvenly,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: List<Widget>.generate(9, (index) {
          // Compute variable bar heights symmetrically from center
          final distFromCenter = (index - 4).abs();
          final factor = 1.0 - (distFromCenter * 0.18);
          final barHeight = 12.0 + (56.0 * normalizedRms * factor);
          final isCenter = index == 4;

          return AnimatedContainer(
            duration: const Duration(milliseconds: 90),
            curve: Curves.easeOutCubic,
            width: 8,
            height: isListening ? barHeight : 8.0,
            decoration: BoxDecoration(
              color: isListening
                  ? (isCenter ? primary : (distFromCenter <= 2 ? secondary : primary.withValues(alpha: 0.6)))
                  : theme.colorScheme.outlineVariant,
              borderRadius: BorderRadius.circular(4),
              boxShadow: isListening && normalizedPeak > 0.4 && isCenter
                  ? <BoxShadow>[
                      BoxShadow(
                        color: primary.withValues(alpha: 0.4),
                        blurRadius: 8,
                        spreadRadius: 2,
                      ),
                    ]
                  : null,
            ),
          );
        }),
      ),
    );
  }
}

/// Large interactive microphone action button supporting push-to-talk hold and toggle taps.
class _VoiceActionButton extends StatefulWidget {
  final VoiceState voiceState;
  final VoidCallback onStart;
  final VoidCallback onStop;

  const _VoiceActionButton({
    required this.voiceState,
    required this.onStart,
    required this.onStop,
  });

  @override
  State<_VoiceActionButton> createState() => _VoiceActionButtonState();
}

class _VoiceActionButtonState extends State<_VoiceActionButton>
    with SingleTickerProviderStateMixin {
  late final AnimationController _pulseController;

  @override
  void initState() {
    super.initState();
    _pulseController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1400),
    );
    if (widget.voiceState.isListening) {
      _pulseController.repeat(reverse: true);
    }
  }

  @override
  void didUpdateWidget(covariant _VoiceActionButton oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.voiceState.isListening && !oldWidget.voiceState.isListening) {
      _pulseController.repeat(reverse: true);
    } else if (!widget.voiceState.isListening && oldWidget.voiceState.isListening) {
      _pulseController.stop();
      _pulseController.reset();
    }
  }

  @override
  void dispose() {
    _pulseController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final isListening = widget.voiceState.isListening;
    final isWakeMode = widget.voiceState.mode == VoiceMode.wakeWord;

    final primaryColor = isWakeMode ? Colors.teal : theme.colorScheme.primary;

    return GestureDetector(
      key: const Key('voice_action_button'),
      onTapDown: (_) {
        if (!isWakeMode) {
          widget.onStart();
        }
      },
      onTapUp: (_) {
        if (!isWakeMode) {
          widget.onStop();
        }
      },
      onTapCancel: () {
        if (!isWakeMode && isListening) {
          widget.onStop();
        }
      },
      onTap: () {
        if (isWakeMode) {
          if (isListening) {
            widget.onStop();
          } else {
            widget.onStart();
          }
        } else {
          // Toggle fallback for click / accessibility
          if (isListening) {
            widget.onStop();
          } else {
            widget.onStart();
          }
        }
      },
      child: AnimatedBuilder(
        animation: _pulseController,
        builder: (context, child) {
          final scale = isListening ? 1.0 + (_pulseController.value * 0.08) : 1.0;
          return Transform.scale(
            scale: scale,
            child: child,
          );
        },
        child: Container(
          width: 96,
          height: 96,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: isListening ? primaryColor : theme.colorScheme.surfaceContainerHighest,
            boxShadow: isListening
                ? <BoxShadow>[
                    BoxShadow(
                      color: primaryColor.withValues(alpha: 0.35),
                      blurRadius: 20,
                      spreadRadius: 6,
                    ),
                  ]
                : <BoxShadow>[
                    BoxShadow(
                      color: Colors.black.withValues(alpha: 0.08),
                      blurRadius: 10,
                      offset: const Offset(0, 4),
                    ),
                  ],
          ),
          child: Icon(
            isListening ? Icons.mic : Icons.mic_none,
            size: 44,
            color: isListening ? Colors.white : theme.colorScheme.onSurfaceVariant,
          ),
        ),
      ),
    );
  }
}

/// Selector chips for customizable offline wake-word triggers.
class _KeywordSelector extends StatelessWidget {
  final List<String> activeKeywords;
  final ValueChanged<List<String>> onKeywordsChanged;

  const _KeywordSelector({
    required this.activeKeywords,
    required this.onKeywordsChanged,
  });

  static const _availableKeywords = <String>[
    'hey assistant',
    'computer',
    'jarvis',
  ];

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card(
      elevation: 0,
      color: theme.colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: theme.colorScheme.outlineVariant),
      ),
      child: Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(
              'Active Wake Keywords',
              style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 8.0,
              runSpacing: 4.0,
              children: _availableKeywords.map((k) {
                final isSelected = activeKeywords.contains(k);
                return FilterChip(
                  label: Text(k),
                  selected: isSelected,
                  onSelected: (selected) {
                    final next = List<String>.from(activeKeywords);
                    if (selected) {
                      if (!next.contains(k)) next.add(k);
                    } else {
                      if (next.length > 1) next.remove(k);
                    }
                    onKeywordsChanged(next);
                  },
                );
              }).toList(),
            ),
          ],
        ),
      ),
    );
  }
}

/// Card showing real-time streaming speech-to-text transcript and confidence.
class _TranscriptCard extends StatelessWidget {
  final VoiceState voiceState;
  final VoidCallback onClear;

  const _TranscriptCard({
    required this.voiceState,
    required this.onClear,
  });

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final hasContent = voiceState.finalTranscript.isNotEmpty ||
        voiceState.partialTranscript.isNotEmpty;

    return Card(
      elevation: 0,
      color: theme.colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: theme.colorScheme.outlineVariant),
      ),
      child: Padding(
        padding: const EdgeInsets.all(20.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              mainAxisAlignment: MainAxisAlignment.spaceBetween,
              children: <Widget>[
                Row(
                  children: <Widget>[
                    Icon(
                      Icons.subtitles_outlined,
                      size: 20,
                      color: theme.colorScheme.primary,
                    ),
                    const SizedBox(width: 8),
                    Text(
                      'Live Transcript',
                      style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
                    ),
                  ],
                ),
                Row(
                  children: <Widget>[
                    Chip(
                      visualDensity: VisualDensity.compact,
                      label: Text(
                        voiceState.language.toUpperCase(),
                        style: const TextStyle(fontSize: 10, fontWeight: FontWeight.bold),
                      ),
                      backgroundColor: theme.colorScheme.surfaceContainerHighest,
                    ),
                    if (hasContent) ...<Widget>[
                      const SizedBox(width: 8),
                      IconButton(
                        icon: const Icon(Icons.clear, size: 18),
                        tooltip: 'Clear transcript',
                        onPressed: onClear,
                      ),
                    ],
                  ],
                ),
              ],
            ),
            const Divider(height: 24),
            Container(
              constraints: const BoxConstraints(minHeight: 120),
              alignment: Alignment.topLeft,
              child: hasContent
                  ? Text.rich(
                      TextSpan(
                        style: theme.textTheme.bodyLarge?.copyWith(height: 1.4),
                        children: <TextSpan>[
                          if (voiceState.finalTranscript.isNotEmpty)
                            TextSpan(
                              text: voiceState.finalTranscript,
                              style: TextStyle(
                                fontWeight: FontWeight.w600,
                                color: theme.colorScheme.onSurface,
                              ),
                            ),
                          if (voiceState.finalTranscript.isNotEmpty &&
                              voiceState.partialTranscript.isNotEmpty)
                            const TextSpan(text: ' '),
                          if (voiceState.partialTranscript.isNotEmpty)
                            TextSpan(
                              text: voiceState.partialTranscript,
                              style: TextStyle(
                                fontStyle: FontStyle.italic,
                                color: theme.colorScheme.primary,
                              ),
                            ),
                        ],
                      ),
                    )
                  : Center(
                      child: Padding(
                        padding: const EdgeInsets.symmetric(vertical: 32.0),
                        child: Text(
                          'No speech captured yet.\nHold the microphone or speak wake-word.',
                          textAlign: TextAlign.center,
                          style: theme.textTheme.bodyMedium?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant.withValues(alpha: 0.6),
                          ),
                        ),
                      ),
                    ),
            ),
          ],
        ),
      ),
    );
  }
}
