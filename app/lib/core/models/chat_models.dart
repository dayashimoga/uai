// Conversational chat session, turn, search, and confirmation models.

/// Represents a conversation session.
class ConversationItem {
  final String id;
  final String title;
  final DateTime createdAt;
  final DateTime updatedAt;

  const ConversationItem({
    required this.id,
    required this.title,
    required this.createdAt,
    required this.updatedAt,
  });

  factory ConversationItem.fromJson(Map<String, dynamic> json) {
    return ConversationItem(
      id: json['id'] as String? ?? '',
      title: json['title'] as String? ?? 'Untitled',
      createdAt: DateTime.fromMillisecondsSinceEpoch(
        (json['created_at_ms'] as num?)?.toInt() ?? 0,
      ),
      updatedAt: DateTime.fromMillisecondsSinceEpoch(
        (json['updated_at_ms'] as num?)?.toInt() ?? 0,
      ),
    );
  }

  Map<String, dynamic> toJson() => {
        'id': id,
        'title': title,
        'created_at_ms': createdAt.millisecondsSinceEpoch,
        'updated_at_ms': updatedAt.millisecondsSinceEpoch,
      };
}

/// Represents an individual interaction turn in a conversation.
class ChatTurnItem {
  final String id;
  final String conversationId;
  final int turnIndex;
  final String role; // "user", "assistant", "system"
  final String content;
  final DateTime timestamp;
  final String? toolCallsJson;
  final List<String> citations;

  const ChatTurnItem({
    required this.id,
    required this.conversationId,
    required this.turnIndex,
    required this.role,
    required this.content,
    required this.timestamp,
    this.toolCallsJson,
    this.citations = const [],
  });

  factory ChatTurnItem.fromJson(Map<String, dynamic> json) {
    return ChatTurnItem(
      id: json['id'] as String? ?? '',
      conversationId: json['conversation_id'] as String? ?? '',
      turnIndex: (json['turn_index'] as num?)?.toInt() ?? 0,
      role: json['role'] as String? ?? 'user',
      content: json['content'] as String? ?? '',
      timestamp: DateTime.fromMillisecondsSinceEpoch(
        (json['timestamp_ms'] as num?)?.toInt() ?? 0,
      ),
      toolCallsJson: json['tool_calls_json'] as String?,
      citations: (json['citations'] as List<dynamic>?)
              ?.map((e) => e.toString())
              .toList() ??
          const [],
    );
  }

  Map<String, dynamic> toJson() => {
        'id': id,
        'conversation_id': conversationId,
        'turn_index': turnIndex,
        'role': role,
        'content': content,
        'timestamp_ms': timestamp.millisecondsSinceEpoch,
        'tool_calls_json': toolCallsJson,
        'citations': citations,
      };
}

/// Represents an FTS5 search hit in conversation history.
class ChatSearchItem {
  final String turnId;
  final String conversationId;
  final int turnIndex;
  final String role;
  final String content;
  final String snippet;
  final double rank;

  const ChatSearchItem({
    required this.turnId,
    required this.conversationId,
    required this.turnIndex,
    required this.role,
    required this.content,
    required this.snippet,
    required this.rank,
  });

  factory ChatSearchItem.fromJson(Map<String, dynamic> json) {
    return ChatSearchItem(
      turnId: json['turn_id'] as String? ?? '',
      conversationId: json['conversation_id'] as String? ?? '',
      turnIndex: (json['turn_index'] as num?)?.toInt() ?? 0,
      role: json['role'] as String? ?? 'user',
      content: json['content'] as String? ?? '',
      snippet: json['snippet'] as String? ?? '',
      rank: (json['rank'] as num?)?.toDouble() ?? 0.0,
    );
  }
}

/// Represents an outstanding security confirmation request.
class PendingConfirmationItem {
  final String turnId;
  final String toolName;
  final String riskTier;
  final String paramsJson;
  final bool taintStatus;

  const PendingConfirmationItem({
    required this.turnId,
    required this.toolName,
    required this.riskTier,
    required this.paramsJson,
    required this.taintStatus,
  });

  factory PendingConfirmationItem.fromJson(Map<String, dynamic> json) {
    return PendingConfirmationItem(
      turnId: json['turn_id'] as String? ?? '',
      toolName: json['tool_name'] as String? ?? '',
      riskTier: json['risk_tier'] as String? ?? 'StateModifying',
      paramsJson: json['params_json'] as String? ?? '{}',
      taintStatus: json['taint_status'] as bool? ?? false,
    );
  }
}
