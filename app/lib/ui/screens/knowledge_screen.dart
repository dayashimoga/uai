import 'package:flutter/material.dart';

import '../../core/assistant_controller.dart';
import '../../core/models/knowledge_models.dart';

/// Responsive Knowledge Explorer screen providing offline hybrid RAG search,
/// grounded citation cards with highlight snippets, and local document catalog management.
class KnowledgeScreen extends StatefulWidget {
  final AssistantController controller;

  const KnowledgeScreen({super.key, required this.controller});

  @override
  State<KnowledgeScreen> createState() => _KnowledgeScreenState();
}

class _KnowledgeScreenState extends State<KnowledgeScreen> with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  final TextEditingController _searchController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
  }

  @override
  void dispose() {
    _tabController.dispose();
    _searchController.dispose();
    super.dispose();
  }

  void _onSearchSubmitted(String query) {
    widget.controller.searchKnowledge(query);
    if (query.trim().isNotEmpty && _tabController.index != 0) {
      _tabController.animateTo(0);
    }
  }

  void _onClearSearch() {
    _searchController.clear();
    widget.controller.clearKnowledgeSearch();
  }

  void _showIndexDialog(BuildContext context) {
    final pathController = TextEditingController(text: 'notes/local_note.md');
    final nameController = TextEditingController(text: 'local_note.md');
    final mimeController = TextEditingController(text: 'text/markdown');
    final contentController = TextEditingController();

    showDialog<void>(
      context: context,
      builder: (dialogCtx) => AlertDialog(
        title: const Row(
          children: <Widget>[
            Icon(Icons.note_add_outlined),
            SizedBox(width: 8),
            Text('Index Local Document'),
          ],
        ),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              TextField(
                key: const Key('index_path_field'),
                controller: pathController,
                decoration: const InputDecoration(
                  labelText: 'Document Path',
                  hintText: 'e.g. docs/notes.md',
                  border: OutlineInputBorder(),
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                key: const Key('index_name_field'),
                controller: nameController,
                decoration: const InputDecoration(
                  labelText: 'File Name',
                  hintText: 'e.g. notes.md',
                  border: OutlineInputBorder(),
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                key: const Key('index_mime_field'),
                controller: mimeController,
                decoration: const InputDecoration(
                  labelText: 'MIME Type',
                  hintText: 'text/markdown or text/plain',
                  border: OutlineInputBorder(),
                ),
              ),
              const SizedBox(height: 12),
              TextField(
                key: const Key('index_content_field'),
                controller: contentController,
                maxLines: 5,
                decoration: const InputDecoration(
                  labelText: 'Document Content',
                  hintText: 'Enter text or markdown to ingest and chunk...',
                  border: OutlineInputBorder(),
                ),
              ),
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogCtx).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton.icon(
            key: const Key('submit_index_button'),
            icon: const Icon(Icons.check),
            label: const Text('Ingest & Chunk'),
            onPressed: () async {
              final path = pathController.text.trim();
              final name = nameController.text.trim();
              final mime = mimeController.text.trim();
              final content = contentController.text.trim();

              if (path.isEmpty || name.isEmpty || content.isEmpty) {
                return;
              }

              Navigator.of(dialogCtx).pop();
              final docId = await widget.controller.indexKnowledgeDocument(
                path: path,
                fileName: name,
                mimeType: mime.isEmpty ? 'text/plain' : mime,
                content: content,
              );

              if (context.mounted && docId != null) {
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text('Document indexed successfully ($docId)')),
                );
              }
            },
          ),
        ],
      ),
    );
  }

  void _showDeleteConfirmDialog(BuildContext context, DocumentRecordItem doc) {
    showDialog<void>(
      context: context,
      builder: (dialogCtx) => AlertDialog(
        title: const Text('Delete Ingested Document?'),
        content: Text(
          'Are you sure you want to delete "${doc.fileName}" (${doc.path})?\n\n'
          'All ${doc.chunkCount} semantic chunks, FTS5 index rows, and dense vector embeddings will be permanently removed.',
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogCtx).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            key: Key('confirm_delete_${doc.id}'),
            style: FilledButton.styleFrom(
              backgroundColor: Theme.of(context).colorScheme.error,
              foregroundColor: Theme.of(context).colorScheme.onError,
            ),
            onPressed: () async {
              Navigator.of(dialogCtx).pop();
              final deleted = await widget.controller.deleteKnowledgeDocument(doc.id);
              if (context.mounted) {
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(
                    content: Text(
                      deleted ? 'Document "${doc.fileName}" deleted' : 'Failed to delete document',
                    ),
                  ),
                );
              }
            },
            child: const Text('Delete'),
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
        final docs = widget.controller.documents;
        final citations = widget.controller.citationSearchResults;
        final isSearching = widget.controller.isSearchingKnowledge;
        final searchQuery = widget.controller.knowledgeSearchQuery;

        return Scaffold(
          appBar: AppBar(
            title: const Text('Knowledge Base & Offline RAG'),
            bottom: TabBar(
              controller: _tabController,
              tabs: <Widget>[
                Tab(
                  icon: const Icon(Icons.search),
                  text: 'Citations (${citations.length})',
                ),
                Tab(
                  icon: const Icon(Icons.folder_outlined),
                  text: 'Documents (${docs.length})',
                ),
              ],
            ),
          ),
          floatingActionButton: FloatingActionButton.extended(
            key: const Key('add_document_fab'),
            icon: const Icon(Icons.add),
            label: const Text('Index Document'),
            onPressed: () => _showIndexDialog(context),
          ),
          body: Column(
            children: <Widget>[
              Padding(
                padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
                child: TextField(
                  key: const Key('knowledge_search_field'),
                  controller: _searchController,
                  onSubmitted: _onSearchSubmitted,
                  decoration: InputDecoration(
                    hintText: 'Search offline knowledge (e.g. Flutter, Rust, architecture)...',
                    prefixIcon: const Icon(Icons.search),
                    suffixIcon: _searchController.text.isNotEmpty || searchQuery != null
                        ? IconButton(
                            key: const Key('clear_search_button'),
                            icon: const Icon(Icons.clear),
                            onPressed: _onClearSearch,
                          )
                        : IconButton(
                            key: const Key('run_search_button'),
                            icon: const Icon(Icons.arrow_forward),
                            onPressed: () => _onSearchSubmitted(_searchController.text),
                          ),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(28),
                    ),
                    filled: true,
                  ),
                ),
              ),
              if (isSearching)
                const LinearProgressIndicator()
              else if (searchQuery != null && searchQuery.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16.0, vertical: 4.0),
                  child: Row(
                    children: <Widget>[
                      Chip(
                        avatar: const Icon(Icons.filter_list, size: 16),
                        label: Text('Query: "$searchQuery"'),
                        onDeleted: _onClearSearch,
                      ),
                      const SizedBox(width: 8),
                      Text(
                        '${citations.length} grounded citation(s) found',
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ],
                  ),
                ),
              Expanded(
                child: TabBarView(
                  controller: _tabController,
                  children: <Widget>[
                    _buildCitationsView(citations, searchQuery),
                    _buildDocumentsView(docs),
                  ],
                ),
              ),
            ],
          ),
        );
      },
    );
  }

  Widget _buildCitationsView(List<KnowledgeCitationItem> citations, String? query) {
    if (citations.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24.0),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Icon(
                Icons.menu_book_outlined,
                size: 64,
                color: Theme.of(context).colorScheme.outline,
              ),
              const SizedBox(height: 16),
              Text(
                query == null || query.isEmpty
                    ? 'Offline Knowledge Search'
                    : 'No citations matched "$query"',
                style: Theme.of(context).textTheme.titleMedium,
              ),
              const SizedBox(height: 8),
              Text(
                query == null || query.isEmpty
                    ? 'Enter keywords above to search local documents using hybrid BM25 and vector fusion.'
                    : 'Try broader search terms or index additional documents into the knowledge base.',
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      color: Theme.of(context).colorScheme.outline,
                    ),
              ),
            ],
          ),
        ),
      );
    }

    return ListView.builder(
      padding: const EdgeInsets.all(16),
      itemCount: citations.length,
      itemBuilder: (context, index) {
        final citation = citations[index];
        return _buildCitationCard(citation, index + 1);
      },
    );
  }

  Widget _buildCitationCard(KnowledgeCitationItem citation, int rankNumber) {
    final theme = Theme.of(context);
    final colorScheme = theme.colorScheme;

    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      elevation: 1,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                CircleAvatar(
                  radius: 14,
                  backgroundColor: colorScheme.primaryContainer,
                  foregroundColor: colorScheme.onPrimaryContainer,
                  child: Text(
                    '$rankNumber',
                    style: const TextStyle(fontSize: 12, fontWeight: FontWeight.bold),
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(
                    citation.fileName,
                    style: theme.textTheme.titleSmall?.copyWith(fontWeight: FontWeight.bold),
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
                Container(
                  padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
                  decoration: BoxDecoration(
                    color: colorScheme.secondaryContainer,
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    'RRF: ${citation.rrfScore.toStringAsFixed(4)}',
                    style: theme.textTheme.labelSmall?.copyWith(
                      color: colorScheme.onSecondaryContainer,
                      fontWeight: FontWeight.bold,
                    ),
                  ),
                ),
              ],
            ),
            const SizedBox(height: 4),
            Text(
              citation.path,
              style: theme.textTheme.bodySmall?.copyWith(color: colorScheme.outline),
              overflow: TextOverflow.ellipsis,
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 6,
              runSpacing: 4,
              children: <Widget>[
                Chip(
                  visualDensity: VisualDensity.compact,
                  materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                  label: Text('Chunk #${citation.chunkIndex}'),
                ),
                if (citation.bm25Rank != null)
                  Chip(
                    visualDensity: VisualDensity.compact,
                    materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    avatar: const Icon(Icons.text_fields, size: 14),
                    label: Text('BM25: #${citation.bm25Rank}'),
                  ),
                if (citation.vectorRank != null)
                  Chip(
                    visualDensity: VisualDensity.compact,
                    materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    avatar: const Icon(Icons.grain, size: 14),
                    label: Text('Vector: #${citation.vectorRank}'),
                  ),
                if (citation.cosineSimilarity != null)
                  Chip(
                    visualDensity: VisualDensity.compact,
                    materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                    label: Text('Cos: ${citation.cosineSimilarity!.toStringAsFixed(2)}'),
                  ),
              ],
            ),
            const Divider(height: 16),
            RichText(
              text: TextSpan(
                style: theme.textTheme.bodyMedium?.copyWith(height: 1.4),
                children: _parseSnippetSpans(citation.snippet, context),
              ),
            ),
            const SizedBox(height: 8),
            Theme(
              data: theme.copyWith(dividerColor: Colors.transparent),
              child: ExpansionTile(
                tilePadding: EdgeInsets.zero,
                childrenPadding: const EdgeInsets.only(top: 8),
                title: Text(
                  'Full Chunk Content',
                  style: theme.textTheme.labelMedium?.copyWith(color: colorScheme.primary),
                ),
                children: <Widget>[
                  Container(
                    width: double.infinity,
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                      color: colorScheme.surfaceContainerHighest.withValues(alpha: 0.5),
                      borderRadius: BorderRadius.circular(8),
                    ),
                    child: SelectableText(
                      citation.content,
                      style: theme.textTheme.bodySmall?.copyWith(fontFamily: 'monospace'),
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

  Widget _buildDocumentsView(List<DocumentRecordItem> docs) {
    if (docs.isEmpty) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(24.0),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Icon(
                Icons.folder_open_outlined,
                size: 64,
                color: Theme.of(context).colorScheme.outline,
              ),
              const SizedBox(height: 16),
              Text(
                'No Ingested Documents',
                style: Theme.of(context).textTheme.titleMedium,
              ),
              const SizedBox(height: 8),
              Text(
                'Add local documents using the "Index Document" button below to enable offline semantic knowledge retrieval.',
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.bodyMedium?.copyWith(
                      color: Theme.of(context).colorScheme.outline,
                    ),
              ),
            ],
          ),
        ),
      );
    }

    return ListView.builder(
      padding: const EdgeInsets.all(16),
      itemCount: docs.length,
      itemBuilder: (context, index) {
        final doc = docs[index];
        return Card(
          key: Key('doc_card_${doc.id}'),
          margin: const EdgeInsets.only(bottom: 12),
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
          child: ListTile(
            contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            leading: CircleAvatar(
              backgroundColor: Theme.of(context).colorScheme.primaryContainer,
              foregroundColor: Theme.of(context).colorScheme.onPrimaryContainer,
              child: const Icon(Icons.description),
            ),
            title: Text(
              doc.fileName,
              style: const TextStyle(fontWeight: FontWeight.bold),
            ),
            subtitle: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(doc.path, overflow: TextOverflow.ellipsis),
                const SizedBox(height: 4),
                Text(
                  '${doc.chunkCount} chunk(s) • ${_formatBytes(doc.fileSizeBytes)} • ${doc.mimeType}',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ],
            ),
            trailing: IconButton(
              key: Key('delete_doc_${doc.id}'),
              icon: Icon(
                Icons.delete_outline,
                color: Theme.of(context).colorScheme.error,
              ),
              tooltip: 'Delete document and chunks',
              onPressed: () => _showDeleteConfirmDialog(context, doc),
            ),
          ),
        );
      },
    );
  }

  List<TextSpan> _parseSnippetSpans(String text, BuildContext context) {
    final spans = <TextSpan>[];
    final colorScheme = Theme.of(context).colorScheme;
    final markRegex = RegExp(r'<mark>(.*?)</mark>');
    int lastEnd = 0;

    for (final match in markRegex.allMatches(text)) {
      if (match.start > lastEnd) {
        spans.add(TextSpan(text: text.substring(lastEnd, match.start)));
      }
      final highlightedText = match.group(1) ?? '';
      spans.add(TextSpan(
        text: highlightedText,
        style: TextStyle(
          backgroundColor: colorScheme.primaryContainer,
          color: colorScheme.onPrimaryContainer,
          fontWeight: FontWeight.bold,
        ),
      ));
      lastEnd = match.end;
    }
    if (lastEnd < text.length) {
      spans.add(TextSpan(text: text.substring(lastEnd)));
    }
    return spans;
  }

  String _formatBytes(int bytes) {
    if (bytes < 1024) return '$bytes B';
    if (bytes < 1024 * 1024) return '${(bytes / 1024).toStringAsFixed(1)} KB';
    return '${(bytes / (1024 * 1024)).toStringAsFixed(1)} MB';
  }
}
