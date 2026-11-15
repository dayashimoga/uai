import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/persona_models.dart';

/// Responsive User Persona, Voice Profile & Explicit Preference Manager Screen.
class PersonaScreen extends StatefulWidget {
  final AssistantController controller;

  const PersonaScreen({super.key, required this.controller});

  @override
  State<PersonaScreen> createState() => _PersonaScreenState();
}

class _PersonaScreenState extends State<PersonaScreen> {
  String? _selectedCategory;

  @override
  void initState() {
    super.initState();
    widget.controller.loadPersonas();
    widget.controller.loadPreferences();
  }

  void _showAddEditPersonaDialog([UserPersonaItem? persona]) {
    final isEditing = persona != null;
    final nameCtrl = TextEditingController(text: persona?.name ?? '');
    final overrideCtrl =
        TextEditingController(text: persona?.systemPromptOverride ?? '');
    var selectedTone = persona?.tone ?? 'balanced';
    var selectedVerbosity = persona?.verbosity ?? 'normal';
    var selectedVoice = persona?.voiceId ?? 'vits_piper_en_ryan';
    var speakingRate = persona?.speakingRate ?? 1.0;
    var pitch = persona?.pitch ?? 1.0;

    showDialog<void>(
      context: context,
      builder: (ctx) {
        return StatefulBuilder(
          builder: (dialogCtx, setDialogState) {
            return AlertDialog(
              title: Text(isEditing ? 'Edit Persona' : 'Create New Persona'),
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
                          labelText: 'Persona Name',
                          hintText: 'e.g. Code Expert or Research Tutor',
                          border: OutlineInputBorder(),
                        ),
                      ),
                      const SizedBox(height: 16),
                      DropdownButtonFormField<String>(
                        initialValue: selectedTone,
                        isExpanded: true,
                        decoration: const InputDecoration(
                          labelText: 'Conversation Tone',
                          border: OutlineInputBorder(),
                        ),
                        items: PersonaTonePreset.values.map((p) {
                          return DropdownMenuItem(
                            value: p.value,
                            child: Text(p.label),
                          );
                        }).toList(),
                        onChanged: (val) {
                          if (val != null) {
                            setDialogState(() => selectedTone = val);
                          }
                        },
                      ),
                      const SizedBox(height: 16),
                      DropdownButtonFormField<String>(
                        initialValue: selectedVerbosity,
                        isExpanded: true,
                        decoration: const InputDecoration(
                          labelText: 'Response Verbosity',
                          border: OutlineInputBorder(),
                        ),
                        items: ResponseVerbosityLevel.values.map((v) {
                          return DropdownMenuItem(
                            value: v.value,
                            child: Text(v.label),
                          );
                        }).toList(),
                        onChanged: (val) {
                          if (val != null) {
                            setDialogState(() => selectedVerbosity = val);
                          }
                        },
                      ),
                      const SizedBox(height: 16),
                      DropdownButtonFormField<String>(
                        initialValue: selectedVoice,
                        isExpanded: true,
                        decoration: const InputDecoration(
                          labelText: 'Voice Synthesizer Profile',
                          border: OutlineInputBorder(),
                        ),
                        items: const [
                          DropdownMenuItem(
                            value: 'vits_piper_en_ryan',
                            child: Text('Ryan (English - Neutral)'),
                          ),
                          DropdownMenuItem(
                            value: 'vits_piper_en_lessac',
                            child: Text('Lessac (English - Expressive)'),
                          ),
                          DropdownMenuItem(
                            value: 'vits_piper_es_davefx',
                            child: Text('Dave (Spanish - Natural)'),
                          ),
                          DropdownMenuItem(
                            value: 'vits_piper_fr_siwis',
                            child: Text('Siwis (French - Clear)'),
                          ),
                        ],
                        onChanged: (val) {
                          if (val != null) {
                            setDialogState(() => selectedVoice = val);
                          }
                        },
                      ),
                      const SizedBox(height: 16),
                      Text('Speaking Rate: ${speakingRate.toStringAsFixed(2)}x'),
                      Slider(
                        value: speakingRate,
                        min: 0.5,
                        max: 2.0,
                        divisions: 15,
                        label: '${speakingRate.toStringAsFixed(2)}x',
                        onChanged: (val) {
                          setDialogState(() => speakingRate = val);
                        },
                      ),
                      const SizedBox(height: 8),
                      Text('Voice Pitch: ${pitch.toStringAsFixed(2)}x'),
                      Slider(
                        value: pitch,
                        min: 0.5,
                        max: 1.5,
                        divisions: 10,
                        label: '${pitch.toStringAsFixed(2)}x',
                        onChanged: (val) {
                          setDialogState(() => pitch = val);
                        },
                      ),
                      const SizedBox(height: 16),
                      TextField(
                        controller: overrideCtrl,
                        maxLines: 3,
                        decoration: const InputDecoration(
                          labelText: 'System Prompt Custom Directives (Optional)',
                          hintText:
                              'e.g. Always generate typed code examples and cite documentation.',
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

                    final item = UserPersonaItem(
                      id: persona?.id ??
                          'persona_${DateTime.now().millisecondsSinceEpoch}',
                      name: name,
                      tone: selectedTone,
                      verbosity: selectedVerbosity,
                      voiceId: selectedVoice,
                      speakingRate: speakingRate,
                      pitch: pitch,
                      systemPromptOverride: overrideCtrl.text.trim().isEmpty
                          ? null
                          : overrideCtrl.text.trim(),
                      isActive: persona?.isActive ?? false,
                      createdAtMs: persona?.createdAtMs ?? 0,
                      updatedAtMs: DateTime.now().millisecondsSinceEpoch,
                    );

                    widget.controller.savePersona(item);
                    Navigator.of(ctx).pop();
                  },
                  child: Text(isEditing ? 'Save Changes' : 'Create'),
                ),
              ],
            );
          },
        );
      },
    );
  }

  void _showAddPreferenceDialog() {
    final keyCtrl = TextEditingController();
    final valCtrl = TextEditingController();
    final catCtrl = TextEditingController(text: 'general');

    showDialog<void>(
      context: context,
      builder: (ctx) {
        return AlertDialog(
          title: const Text('Add Explicit User Preference'),
          content: SizedBox(
            width: 400,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                TextField(
                  controller: keyCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Preference Key',
                    hintText: 'e.g. editor.theme or code.style',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: valCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Value (JSON formatted)',
                    hintText: 'e.g. "dark" or 4 or true',
                    border: OutlineInputBorder(),
                  ),
                ),
                const SizedBox(height: 12),
                TextField(
                  controller: catCtrl,
                  decoration: const InputDecoration(
                    labelText: 'Category',
                    hintText: 'e.g. appearance, developer, localization',
                    border: OutlineInputBorder(),
                  ),
                ),
              ],
            ),
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.of(ctx).pop(),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () {
                final k = keyCtrl.text.trim();
                final v = valCtrl.text.trim();
                final c = catCtrl.text.trim();
                if (k.isNotEmpty && v.isNotEmpty) {
                  widget.controller.setPreference(k, v, c.isEmpty ? 'general' : c);
                  Navigator.of(ctx).pop();
                }
              },
              child: const Text('Add Preference'),
            ),
          ],
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;

    return AnimatedBuilder(
      animation: widget.controller,
      builder: (context, _) {
        final active = widget.controller.activePersona;
        final personas = widget.controller.personas;
        final preferences = widget.controller.preferences;

        // Distinct categories for filtering
        final categories = <String>{
          'All',
          ...preferences.map((p) => p.category),
        }.toList();

        final filteredPrefs = (_selectedCategory == null ||
                _selectedCategory == 'All')
            ? preferences
            : preferences
                .where((p) => p.category == _selectedCategory)
                .toList();

        return Scaffold(
          body: widget.controller.isPersonaLoading && personas.isEmpty
              ? const Center(child: CircularProgressIndicator())
              : RefreshIndicator(
                  onRefresh: () async {
                    await widget.controller.loadPersonas();
                    await widget.controller.loadPreferences();
                  },
                  child: ListView(
                    padding: const EdgeInsets.all(20),
                    children: [
                      // Active Persona Hero Card
                      if (active != null) ...[
                        _buildActivePersonaHeroCard(context, active),
                        const SizedBox(height: 24),
                      ],

                      // Saved Personas Section
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Text(
                            'Saved Personas (${personas.length})',
                            style: theme.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                          FilledButton.tonalIcon(
                            onPressed: () => _showAddEditPersonaDialog(),
                            icon: const Icon(Icons.add),
                            label: const Text('New Persona'),
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),
                      ...personas.map((p) => _buildPersonaTile(context, p)),
                      const SizedBox(height: 28),

                      // Explicit Preferences Section
                      Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Text(
                            'Explicit Preferences (${preferences.length})',
                            style: theme.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                          OutlinedButton.icon(
                            onPressed: _showAddPreferenceDialog,
                            icon: const Icon(Icons.tune),
                            label: const Text('Add Preference'),
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),

                      // Category filter chips
                      if (categories.length > 1) ...[
                        SingleChildScrollView(
                          scrollDirection: Axis.horizontal,
                          child: Row(
                            children: categories.map((cat) {
                              final isSelected = (_selectedCategory ?? 'All') == cat;
                              return Padding(
                                padding: const EdgeInsets.only(right: 8),
                                child: FilterChip(
                                  label: Text(cat),
                                  selected: isSelected,
                                  onSelected: (selected) {
                                    setState(() {
                                      _selectedCategory = selected ? cat : 'All';
                                    });
                                  },
                                ),
                              );
                            }).toList(),
                          ),
                        ),
                        const SizedBox(height: 12),
                      ],

                      if (filteredPrefs.isEmpty)
                        Card(
                          elevation: 0,
                          color: colorScheme.surfaceContainerLow,
                          child: const Padding(
                            padding: EdgeInsets.all(16),
                            child: Center(
                              child: Text('No preferences stored in this category.'),
                            ),
                          ),
                        )
                      else
                        Card(
                          elevation: 1,
                          clipBehavior: Clip.antiAlias,
                          child: ListView.separated(
                            shrinkWrap: true,
                            physics: const NeverScrollableScrollPhysics(),
                            itemCount: filteredPrefs.length,
                            separatorBuilder: (_, __) => const Divider(height: 1),
                            itemBuilder: (context, index) {
                              final pref = filteredPrefs[index];
                              return ListTile(
                                leading: CircleAvatar(
                                  backgroundColor: colorScheme.primaryContainer,
                                  child: Icon(
                                    Icons.settings,
                                    size: 18,
                                    color: colorScheme.onPrimaryContainer,
                                  ),
                                ),
                                title: Text(
                                  pref.key,
                                  style: const TextStyle(fontWeight: FontWeight.w600),
                                ),
                                subtitle: Text(
                                  'Value: ${pref.valueJson} • Category: ${pref.category}',
                                  style: theme.textTheme.bodySmall,
                                ),
                                trailing: IconButton(
                                  icon: const Icon(Icons.delete_outline),
                                  tooltip: 'Delete preference',
                                  onPressed: () {
                                    widget.controller.deletePreference(pref.key);
                                  },
                                ),
                              );
                            },
                          ),
                        ),
                    ],
                  ),
                ),
        );
      },
    );
  }

  Widget _buildActivePersonaHeroCard(BuildContext context, UserPersonaItem active) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    final tonePreset = PersonaTonePreset.fromString(active.tone);
    final verbosityLevel = ResponseVerbosityLevel.fromString(active.verbosity);

    return Card(
      elevation: 2,
      color: colorScheme.primaryContainer.withAlpha(80),
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: BorderSide(color: colorScheme.primary.withAlpha(120), width: 1.5),
      ),
      child: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                CircleAvatar(
                  radius: 24,
                  backgroundColor: colorScheme.primary,
                  child: Icon(Icons.record_voice_over, color: colorScheme.onPrimary),
                ),
                const SizedBox(width: 14),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Row(
                        children: [
                          Text(
                            active.name,
                            style: theme.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.bold,
                            ),
                          ),
                          const SizedBox(width: 8),
                          Chip(
                            avatar: const Icon(Icons.check_circle, size: 16),
                            label: const Text('Active'),
                            visualDensity: VisualDensity.compact,
                            backgroundColor: colorScheme.primary.withAlpha(40),
                          ),
                        ],
                      ),
                      Text(
                        'Voice: ${active.voiceId}',
                        style: theme.textTheme.bodyMedium?.copyWith(
                          color: colorScheme.onSurfaceVariant,
                        ),
                      ),
                    ],
                  ),
                ),
                IconButton.filledTonal(
                  icon: const Icon(Icons.edit),
                  tooltip: 'Edit active persona',
                  onPressed: () => _showAddEditPersonaDialog(active),
                ),
              ],
            ),
            const SizedBox(height: 16),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              children: [
                Chip(
                  avatar: const Icon(Icons.psychology, size: 16),
                  label: Text('Tone: ${tonePreset.label}'),
                ),
                Chip(
                  avatar: const Icon(Icons.short_text, size: 16),
                  label: Text('Depth: ${verbosityLevel.label}'),
                ),
                Chip(
                  avatar: const Icon(Icons.speed, size: 16),
                  label: Text('Rate: ${active.speakingRate.toStringAsFixed(2)}x'),
                ),
                Chip(
                  avatar: const Icon(Icons.graphic_eq, size: 16),
                  label: Text('Pitch: ${active.pitch.toStringAsFixed(2)}x'),
                ),
              ],
            ),
            if (active.systemPromptOverride != null &&
                active.systemPromptOverride!.isNotEmpty) ...[
              const SizedBox(height: 14),
              Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: colorScheme.surface.withAlpha(200),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(Icons.info_outline, size: 18, color: colorScheme.primary),
                    const SizedBox(width: 8),
                    Expanded(
                      child: Text(
                        active.systemPromptOverride!,
                        style: theme.textTheme.bodySmall?.copyWith(fontStyle: FontStyle.italic),
                      ),
                    ),
                  ],
                ),
              ),
            ],
            const SizedBox(height: 16),
            const Divider(),
            const SizedBox(height: 8),
            Text(
              'Live Voice Tuning',
              style: theme.textTheme.labelLarge?.copyWith(fontWeight: FontWeight.bold),
            ),
            Row(
              children: [
                const SizedBox(width: 100, child: Text('Speed:')),
                Expanded(
                  child: Slider(
                    value: active.speakingRate,
                    min: 0.5,
                    max: 2.0,
                    divisions: 15,
                    label: '${active.speakingRate.toStringAsFixed(2)}x',
                    onChanged: (val) {
                      widget.controller.savePersona(
                        active.copyWith(speakingRate: val),
                      );
                    },
                  ),
                ),
                Text('${active.speakingRate.toStringAsFixed(2)}x'),
              ],
            ),
            Row(
              children: [
                const SizedBox(width: 100, child: Text('Pitch:')),
                Expanded(
                  child: Slider(
                    value: active.pitch,
                    min: 0.5,
                    max: 1.5,
                    divisions: 10,
                    label: '${active.pitch.toStringAsFixed(2)}x',
                    onChanged: (val) {
                      widget.controller.savePersona(
                        active.copyWith(pitch: val),
                      );
                    },
                  ),
                ),
                Text('${active.pitch.toStringAsFixed(2)}x'),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildPersonaTile(BuildContext context, UserPersonaItem p) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;
    final isDefault = p.id == 'persona_default';

    return Card(
      elevation: 0,
      margin: const EdgeInsets.only(bottom: 8),
      color: p.isActive
          ? colorScheme.primaryContainer.withAlpha(40)
          : colorScheme.surfaceContainerLow,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(12),
        side: BorderSide(
          color: p.isActive ? colorScheme.primary : colorScheme.outlineVariant,
        ),
      ),
      child: ListTile(
        leading: CircleAvatar(
          backgroundColor: p.isActive
              ? colorScheme.primary
              : colorScheme.surfaceContainerHighest,
          child: Icon(
            p.isActive ? Icons.check : Icons.person_outline,
            color: p.isActive
                ? colorScheme.onPrimary
                : colorScheme.onSurfaceVariant,
          ),
        ),
        title: Text(
          p.name,
          style: TextStyle(
            fontWeight: p.isActive ? FontWeight.bold : FontWeight.w500,
          ),
        ),
        subtitle: Text(
          'Tone: ${p.tone} • Depth: ${p.verbosity} • Voice: ${p.voiceId}',
          style: theme.textTheme.bodySmall,
        ),
        trailing: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (!p.isActive)
              FilledButton.tonal(
                onPressed: () => widget.controller.setActivePersona(p.id),
                child: const Text('Activate'),
              ),
            IconButton(
              icon: const Icon(Icons.edit_outlined),
              tooltip: 'Edit persona',
              onPressed: () => _showAddEditPersonaDialog(p),
            ),
            IconButton(
              icon: const Icon(Icons.delete_outline),
              tooltip: isDefault
                  ? 'Default persona cannot be deleted'
                  : p.isActive
                      ? 'Active persona cannot be deleted'
                      : 'Delete persona',
              onPressed: (isDefault || p.isActive)
                  ? null
                  : () => widget.controller.deletePersona(p.id),
            ),
          ],
        ),
      ),
    );
  }
}
