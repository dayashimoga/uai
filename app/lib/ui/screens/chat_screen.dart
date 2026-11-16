import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/chat_models.dart';

/// Primary conversational chat screen providing full conversational turn loop,
/// SQLite FTS5 search, RAG citation chips, and 5-tier confirmation gate.
class ChatScreen extends StatefulWidget {
  final AssistantController controller;

  const ChatScreen({super.key, required this.controller});

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final TextEditingController _inputController = TextEditingController();
  final TextEditingController _searchController = TextEditingController();
  final ScrollController _scrollController = ScrollController();
  final FocusNode _inputFocus = FocusNode();

  bool _isHistoryDrawerOpen = true;
  bool _isSearching = false;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_onControllerChanged);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (widget.controller.conversations.isEmpty) {
        widget.controller.loadConversations();
      }
    });
  }

  @override
  void dispose() {
    widget.controller.removeListener(_onControllerChanged);
    _inputController.dispose();
    _searchController.dispose();
    _scrollController.dispose();
    _inputFocus.dispose();
    super.dispose();
  }

  void _onControllerChanged() {
    if (mounted) {
      if (widget.controller.pendingChatPrompt != null) {
        _inputController.text = widget.controller.pendingChatPrompt!;
        widget.controller.clearPendingChatPrompt();
      }
      setState(() {});
      _scrollToBottom();
    }
  }

  void _scrollToBottom() {
    if (_scrollController.hasClients) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (_scrollController.hasClients) {
          _scrollController.animateTo(
            _scrollController.position.maxScrollExtent,
            duration: const Duration(milliseconds: 200),
            curve: Curves.easeOut,
          );
        }
      });
    }
  }

  void _handleSend() {
    final text = _inputController.text.trim();
    if (text.isEmpty || widget.controller.isChatStreaming) return;
    _inputController.clear();
    widget.controller.sendChatMessage(text);
    _scrollToBottom();
  }

  void _showAttachFileDialog() {
    final pathController = TextEditingController();
    showDialog<void>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: const Text('Attach Context File'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            const Text('Enter local file path to include in prompt context:'),
            const SizedBox(height: 12),
            TextField(
              controller: pathController,
              decoration: const InputDecoration(
                hintText: 'e.g. docs/architecture.md',
                border: OutlineInputBorder(),
                prefixIcon: Icon(Icons.attach_file),
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
              final path = pathController.text.trim();
              if (path.isNotEmpty) {
                Navigator.of(ctx).pop();
                _inputController.text =
                    '${_inputController.text}\n[Attached Context: $path]\n';
              }
            },
            child: const Text('Attach'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final isDesktop = MediaQuery.of(context).size.width >= 720;
    final activeConv = widget.controller.activeConversation;
    final activeModel = widget.controller.activeModel;

    return Scaffold(
      appBar: AppBar(
        title: Row(
          children: <Widget>[
            IconButton(
              icon: Icon(
                _isHistoryDrawerOpen ? Icons.menu_open : Icons.menu,
                semanticLabel: 'Toggle Chat History',
              ),
              onPressed: () => setState(() => _isHistoryDrawerOpen = !_isHistoryDrawerOpen),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: Text(
                activeConv?.title ?? 'Chat',
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 18, fontWeight: FontWeight.w600),
              ),
            ),
          ],
        ),
        actions: <Widget>[
          Chip(
            avatar: Icon(
              activeModel != null ? Icons.memory : Icons.warning_amber_rounded,
              size: 16,
              color: activeModel != null ? Colors.green : Colors.orange,
            ),
            label: Text(
              activeModel?.name ?? 'No Model Loaded',
              style: TextStyle(
                fontSize: 12,
                fontWeight: FontWeight.w500,
                color: activeModel != null ? Colors.green.shade800 : Colors.orange.shade800,
              ),
            ),
            backgroundColor: (activeModel != null ? Colors.green : Colors.orange).withValues(alpha: 0.12),
            side: BorderSide.none,
            padding: EdgeInsets.zero,
          ),
          const SizedBox(width: 8),
          IconButton(
            icon: const Icon(Icons.add_comment_outlined),
            tooltip: 'New Conversation',
            onPressed: () => widget.controller.createNewConversation(),
          ),
          const SizedBox(width: 8),
        ],
      ),
      body: Row(
        children: <Widget>[
          // Left History Drawer / Search Pane
          if (_isHistoryDrawerOpen)
            Container(
              width: isDesktop ? 280 : 220,
              decoration: BoxDecoration(
                border: Border(
                  right: BorderSide(color: theme.dividerColor.withValues(alpha: 0.4)),
                ),
              ),
              child: Material(
                color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.25),
                child: _buildHistorySidebar(context),
              ),
            ),

          // Main Conversation Viewport
          Expanded(
            child: Column(
              children: <Widget>[
                if (activeModel == null) _buildNoModelBanner(context),
                if (widget.controller.chatError != null)
                  _buildErrorBanner(widget.controller.chatError!),
                Expanded(
                  child: _buildMessagesViewport(context),
                ),
                if (widget.controller.pendingConfirmation != null)
                  _buildConfirmationCard(context, widget.controller.pendingConfirmation!),
                _buildComposer(context),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildNoModelBanner(BuildContext context) {
    return Container(
      width: double.infinity,
      color: Colors.amber.shade900.withValues(alpha: 0.15),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Row(
        children: <Widget>[
          const Icon(Icons.info_outline, size: 18, color: Colors.amber),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              'No active GGUF model loaded in RAM. Generation will be handled by local fallback runtime.',
              style: TextStyle(fontSize: 12, color: Colors.amber.shade900),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildErrorBanner(String error) {
    return Container(
      width: double.infinity,
      color: Colors.red.shade900.withValues(alpha: 0.15),
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: Row(
        children: <Widget>[
          const Icon(Icons.error_outline, size: 18, color: Colors.red),
          const SizedBox(width: 10),
          Expanded(
            child: Text(
              error,
              style: TextStyle(fontSize: 12, color: Colors.red.shade800),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildHistorySidebar(BuildContext context) {
    final theme = Theme.of(context);
    final convs = widget.controller.conversations;
    final activeId = widget.controller.activeConversationId;

    return Column(
      children: <Widget>[
        // New Chat Button
        Padding(
          padding: const EdgeInsets.all(12.0),
          child: FilledButton.tonalIcon(
            onPressed: () => widget.controller.createNewConversation(),
            icon: const Icon(Icons.add, size: 18),
            label: const Text('New Chat'),
            style: FilledButton.styleFrom(
              minimumSize: const Size.fromHeight(42),
            ),
          ),
        ),

        // Instant FTS5 Search Box
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 12.0),
          child: TextField(
            controller: _searchController,
            decoration: InputDecoration(
              hintText: 'Search chats (FTS5)...',
              prefixIcon: const Icon(Icons.search, size: 18),
              suffixIcon: _searchController.text.isNotEmpty
                  ? IconButton(
                      icon: const Icon(Icons.clear, size: 16),
                      onPressed: () {
                        _searchController.clear();
                        widget.controller.clearChatSearch();
                        setState(() => _isSearching = false);
                      },
                    )
                  : null,
              isDense: true,
              border: OutlineInputBorder(
                borderRadius: BorderRadius.circular(8),
              ),
              contentPadding: const EdgeInsets.symmetric(vertical: 8, horizontal: 8),
            ),
            onChanged: (val) {
              if (val.trim().isNotEmpty) {
                setState(() => _isSearching = true);
                widget.controller.searchChatTurns(val);
              } else {
                setState(() => _isSearching = false);
                widget.controller.clearChatSearch();
              }
            },
          ),
        ),
        const SizedBox(height: 8),

        // Search Results or Conversation Thread List
        Expanded(
          child: _isSearching
              ? _buildSearchResultsList(context)
              : ListView.builder(
                  itemCount: convs.length,
                  itemBuilder: (ctx, idx) {
                    final conv = convs[idx];
                    final isSelected = conv.id == activeId;
                    return ListTile(
                      dense: true,
                      selected: isSelected,
                      selectedTileColor: theme.colorScheme.primary.withValues(alpha: 0.12),
                      leading: Icon(
                        isSelected ? Icons.chat_bubble : Icons.chat_bubble_outline,
                        size: 18,
                        color: isSelected ? theme.colorScheme.primary : theme.hintColor,
                      ),
                      title: Text(
                        conv.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontWeight: isSelected ? FontWeight.bold : FontWeight.normal,
                          fontSize: 13,
                        ),
                      ),
                      trailing: IconButton(
                        icon: const Icon(Icons.delete_outline, size: 16),
                        tooltip: 'Delete thread',
                        onPressed: () => widget.controller.deleteConversation(conv.id),
                      ),
                      onTap: () => widget.controller.selectConversation(conv.id),
                    );
                  },
                ),
        ),
      ],
    );
  }

  Widget _buildSearchResultsList(BuildContext context) {
    final results = widget.controller.chatSearchResults;
    if (widget.controller.isSearchingChat) {
      return const Center(child: CircularProgressIndicator());
    }
    if (results.isEmpty) {
      return const Center(
        child: Padding(
          padding: EdgeInsets.all(16.0),
          child: Text('No matching messages found.', style: TextStyle(fontSize: 12)),
        ),
      );
    }
    return ListView.builder(
      itemCount: results.length,
      itemBuilder: (ctx, idx) {
        final hit = results[idx];
        return ListTile(
          dense: true,
          title: Text(
            hit.snippet,
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: const TextStyle(fontSize: 12),
          ),
          subtitle: Text(
            '${hit.role.toUpperCase()} • BM25: ${hit.rank.toStringAsFixed(2)}',
            style: const TextStyle(fontSize: 10, color: Colors.grey),
          ),
          onTap: () {
            widget.controller.selectConversation(hit.conversationId);
            setState(() {
              _isSearching = false;
              _searchController.clear();
            });
          },
        );
      },
    );
  }

  Widget _buildMessagesViewport(BuildContext context) {
    final turns = widget.controller.currentTurns;
    final isStreaming = widget.controller.isChatStreaming;
    final streamingText = widget.controller.streamingAssistantText;

    if (turns.isEmpty && !isStreaming) {
      return _buildEmptyState(context);
    }

    return ListView.builder(
      controller: _scrollController,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      itemCount: turns.length + (isStreaming ? 1 : 0),
      itemBuilder: (ctx, idx) {
        if (idx == turns.length && isStreaming) {
          return _buildStreamingBubble(context, streamingText);
        }
        final turn = turns[idx];
        return _buildTurnBubble(context, turn);
      },
    );
  }

  Widget _buildEmptyState(BuildContext context) {
    final theme = Theme.of(context);
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.all(24.0),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: <Widget>[
            Icon(
              Icons.auto_awesome,
              size: 54,
              color: theme.colorScheme.primary.withValues(alpha: 0.8),
            ),
            const SizedBox(height: 16),
            Text(
              'How can I help you today?',
              style: theme.textTheme.headlineSmall?.copyWith(fontWeight: FontWeight.bold),
            ),
            const SizedBox(height: 8),
            Text(
              'Local AI Assistant with hardware-accelerated privacy and verifiable BLAKE3 audit trail.',
              textAlign: TextAlign.center,
              style: theme.textTheme.bodyMedium?.copyWith(color: theme.hintColor),
            ),
            const SizedBox(height: 24),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              alignment: WrapAlignment.center,
              children: <Widget>[
                ActionChip(
                  avatar: const Icon(Icons.speed, size: 16),
                  label: const Text('Show live performance snapshot'),
                  onPressed: () {
                    _inputController.text = 'Show current hardware accelerator metrics and memory usage.';
                    _handleSend();
                  },
                ),
                ActionChip(
                  avatar: const Icon(Icons.shield_outlined, size: 16),
                  label: const Text('Explain local security policies'),
                  onPressed: () {
                    _inputController.text = 'Explain the 5-tier confirmation gate and prompt-injection defenses.';
                    _handleSend();
                  },
                ),
                ActionChip(
                  avatar: const Icon(Icons.devices, size: 16),
                  label: const Text('Device pairing protocol'),
                  onPressed: () {
                    _inputController.text = 'How does peer-to-peer device pairing work over mDNS?';
                    _handleSend();
                  },
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildTurnBubble(BuildContext context, ChatTurnItem turn) {
    final theme = Theme.of(context);
    final isUser = turn.role.toLowerCase() == 'user';

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8.0),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisAlignment: isUser ? MainAxisAlignment.end : MainAxisAlignment.start,
        children: <Widget>[
          if (!isUser) ...<Widget>[
            CircleAvatar(
              radius: 16,
              backgroundColor: theme.colorScheme.primary.withValues(alpha: 0.15),
              child: Icon(Icons.smart_toy_outlined, size: 18, color: theme.colorScheme.primary),
            ),
            const SizedBox(width: 10),
          ],
          Flexible(
            child: Container(
              padding: const EdgeInsets.all(14.0),
              decoration: BoxDecoration(
                color: isUser
                    ? theme.colorScheme.primary
                    : theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.4),
                borderRadius: BorderRadius.only(
                  topLeft: const Radius.circular(16),
                  topRight: const Radius.circular(16),
                  bottomLeft: isUser ? const Radius.circular(16) : const Radius.circular(4),
                  bottomRight: isUser ? const Radius.circular(4) : const Radius.circular(16),
                ),
                border: Border.all(
                  color: isUser
                      ? theme.colorScheme.primary
                      : theme.dividerColor.withValues(alpha: 0.2),
                ),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  SelectableText(
                    turn.content,
                    style: TextStyle(
                      fontSize: 14,
                      color: isUser
                          ? theme.colorScheme.onPrimary
                          : theme.colorScheme.onSurface,
                    ),
                  ),
                  if (turn.citations.isNotEmpty) ...<Widget>[
                    const SizedBox(height: 10),
                    const Divider(height: 1),
                    const SizedBox(height: 6),
                    Wrap(
                      spacing: 6,
                      runSpacing: 4,
                      children: turn.citations.map((c) {
                        return Tooltip(
                          message: c,
                          child: Chip(
                            avatar: const Icon(Icons.library_books, size: 14),
                            label: Text(
                              c.length > 25 ? '${c.substring(0, 25)}...' : c,
                              style: const TextStyle(fontSize: 11),
                            ),
                            padding: EdgeInsets.zero,
                            materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                          ),
                        );
                      }).toList(),
                    ),
                  ],
                ],
              ),
            ),
          ),
          if (isUser) ...<Widget>[
            const SizedBox(width: 10),
            CircleAvatar(
              radius: 16,
              backgroundColor: theme.colorScheme.secondary.withValues(alpha: 0.2),
              child: Icon(Icons.person, size: 18, color: theme.colorScheme.secondary),
            ),
          ],
        ],
      ),
    );
  }

  Widget _buildStreamingBubble(BuildContext context, String text) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 8.0),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          CircleAvatar(
            radius: 16,
            backgroundColor: theme.colorScheme.primary.withValues(alpha: 0.15),
            child: Icon(Icons.smart_toy_outlined, size: 18, color: theme.colorScheme.primary),
          ),
          const SizedBox(width: 10),
          Flexible(
            child: Container(
              padding: const EdgeInsets.all(14.0),
              decoration: BoxDecoration(
                color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.4),
                borderRadius: const BorderRadius.only(
                  topLeft: Radius.circular(16),
                  topRight: Radius.circular(16),
                  bottomLeft: Radius.circular(4),
                  bottomRight: Radius.circular(16),
                ),
                border: Border.all(color: theme.dividerColor.withValues(alpha: 0.2)),
              ),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  SelectableText(
                    text.isEmpty ? 'Thinking...' : '$text ▌',
                    style: TextStyle(
                      fontSize: 14,
                      color: theme.colorScheme.onSurface,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildConfirmationCard(BuildContext context, PendingConfirmationItem confirmation) {
    final theme = Theme.of(context);

    return Container(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: Colors.amber.shade900.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: Colors.amber.shade700, width: 1.5),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              const Icon(Icons.shield_outlined, color: Colors.amber, size: 24),
              const SizedBox(width: 8),
              const Expanded(
                child: Text(
                  'Security Policy Confirmation Required',
                  style: TextStyle(fontWeight: FontWeight.bold, fontSize: 14),
                  overflow: TextOverflow.ellipsis,
                ),
              ),
              const SizedBox(width: 8),
              Chip(
                label: Text(
                  confirmation.riskTier,
                  style: const TextStyle(fontSize: 11, fontWeight: FontWeight.bold),
                ),
                backgroundColor: Colors.amber.shade700.withValues(alpha: 0.2),
                side: BorderSide.none,
                padding: EdgeInsets.zero,
              ),
            ],
          ),
          const SizedBox(height: 8),
          Text(
            'The assistant requests permission to invoke tool: "${confirmation.toolName}"',
            style: const TextStyle(fontSize: 13),
          ),
          if (confirmation.taintStatus) ...<Widget>[
            const SizedBox(height: 6),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
              decoration: BoxDecoration(
                color: Colors.red.shade900.withValues(alpha: 0.12),
                borderRadius: BorderRadius.circular(4),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  const Icon(Icons.warning, size: 14, color: Colors.red),
                  const SizedBox(width: 6),
                  Text(
                    'Turn contains untrusted external input (Tainted)',
                    style: TextStyle(fontSize: 11, color: Colors.red.shade900, fontWeight: FontWeight.w600),
                  ),
                ],
              ),
            ),
          ],
          const SizedBox(height: 8),
          Container(
            padding: const EdgeInsets.all(8),
            width: double.infinity,
            decoration: BoxDecoration(
              color: theme.colorScheme.surface,
              borderRadius: BorderRadius.circular(6),
              border: Border.all(color: theme.dividerColor.withValues(alpha: 0.3)),
            ),
            child: SelectableText(
              confirmation.paramsJson,
              style: const TextStyle(fontFamily: 'monospace', fontSize: 11),
            ),
          ),
          const SizedBox(height: 12),
          Row(
            mainAxisAlignment: MainAxisAlignment.end,
            children: <Widget>[
              OutlinedButton.icon(
                onPressed: () => widget.controller.resolveConfirmation(confirmation.turnId, false),
                icon: const Icon(Icons.close, size: 16),
                label: const Text('Deny'),
                style: OutlinedButton.styleFrom(
                  foregroundColor: Colors.red,
                  side: const BorderSide(color: Colors.red),
                ),
              ),
              const SizedBox(width: 12),
              FilledButton.icon(
                onPressed: () => widget.controller.resolveConfirmation(confirmation.turnId, true),
                icon: const Icon(Icons.check, size: 16),
                label: const Text('Approve'),
                style: FilledButton.styleFrom(
                  backgroundColor: Colors.green.shade700,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Widget _buildComposer(BuildContext context) {
    final theme = Theme.of(context);
    final isStreaming = widget.controller.isChatStreaming;

    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
      decoration: BoxDecoration(
        color: theme.colorScheme.surface,
        border: Border(
          top: BorderSide(color: theme.dividerColor.withValues(alpha: 0.4)),
        ),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: <Widget>[
          IconButton(
            icon: const Icon(Icons.attach_file),
            tooltip: 'Attach File Context',
            onPressed: isStreaming ? null : _showAttachFileDialog,
          ),
          Expanded(
            child: CallbackShortcuts(
              bindings: <ShortcutActivator, VoidCallback>{
                const SingleActivator(LogicalKeyboardKey.enter, control: true): _handleSend,
                const SingleActivator(LogicalKeyboardKey.enter): _handleSend,
              },
              child: TextField(
                controller: _inputController,
                focusNode: _inputFocus,
                maxLines: 5,
                minLines: 1,
                textInputAction: TextInputAction.send,
                onSubmitted: (_) => _handleSend(),
                decoration: InputDecoration(
                  hintText: isStreaming
                      ? 'Assistant is responding...'
                      : 'Ask a question or enter a command... (Enter to send)',
                  isDense: true,
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(16),
                  ),
                  contentPadding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
                ),
              ),
            ),
          ),
          const SizedBox(width: 8),
          if (isStreaming)
            IconButton.filled(
              icon: const Icon(Icons.stop),
              tooltip: 'Stop Generation',
              style: IconButton.styleFrom(backgroundColor: Colors.red.shade700),
              onPressed: () => widget.controller.cancelChatTurn(),
            )
          else
            IconButton.filled(
              icon: const Icon(Icons.arrow_upward),
              tooltip: 'Send Message',
              onPressed: _handleSend,
            ),
        ],
      ),
    );
  }
}
