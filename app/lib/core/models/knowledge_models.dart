import 'package:flutter/foundation.dart';

/// Ingested document record representation in Flutter.
@immutable
class DocumentRecordItem {
  final String id;
  final String path;
  final String fileName;
  final String mimeType;
  final String contentHash;
  final int fileSizeBytes;
  final int chunkCount;
  final int indexedAtMs;
  final int updatedAtMs;

  const DocumentRecordItem({
    required this.id,
    required this.path,
    required this.fileName,
    required this.mimeType,
    required this.contentHash,
    required this.fileSizeBytes,
    required this.chunkCount,
    required this.indexedAtMs,
    required this.updatedAtMs,
  });

  factory DocumentRecordItem.fromJson(Map<String, dynamic> json) {
    return DocumentRecordItem(
      id: json['id'] as String? ?? '',
      path: json['path'] as String? ?? '',
      fileName: json['file_name'] as String? ?? '',
      mimeType: json['mime_type'] as String? ?? 'text/plain',
      contentHash: json['content_hash'] as String? ?? '',
      fileSizeBytes: (json['file_size_bytes'] as num?)?.toInt() ?? 0,
      chunkCount: (json['chunk_count'] as num?)?.toInt() ?? 0,
      indexedAtMs: (json['indexed_at_ms'] as num?)?.toInt() ?? 0,
      updatedAtMs: (json['updated_at_ms'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'id': id,
        'path': path,
        'file_name': fileName,
        'mime_type': mimeType,
        'content_hash': contentHash,
        'file_size_bytes': fileSizeBytes,
        'chunk_count': chunkCount,
        'indexed_at_ms': indexedAtMs,
        'updated_at_ms': updatedAtMs,
      };
}

/// Grounded knowledge citation resulting from hybrid search.
@immutable
class KnowledgeCitationItem {
  final String chunkId;
  final String documentId;
  final String path;
  final String fileName;
  final int chunkIndex;
  final double rrfScore;
  final int? bm25Rank;
  final int? vectorRank;
  final double? cosineSimilarity;
  final String content;
  final String snippet;

  const KnowledgeCitationItem({
    required this.chunkId,
    required this.documentId,
    required this.path,
    required this.fileName,
    required this.chunkIndex,
    required this.rrfScore,
    this.bm25Rank,
    this.vectorRank,
    this.cosineSimilarity,
    required this.content,
    required this.snippet,
  });

  factory KnowledgeCitationItem.fromJson(Map<String, dynamic> json) {
    return KnowledgeCitationItem(
      chunkId: json['chunk_id'] as String? ?? '',
      documentId: json['document_id'] as String? ?? '',
      path: json['path'] as String? ?? '',
      fileName: json['file_name'] as String? ?? '',
      chunkIndex: (json['chunk_index'] as num?)?.toInt() ?? 0,
      rrfScore: (json['rrf_score'] as num?)?.toDouble() ?? 0.0,
      bm25Rank: (json['bm25_rank'] as num?)?.toInt(),
      vectorRank: (json['vector_rank'] as num?)?.toInt(),
      cosineSimilarity: (json['cosine_similarity'] as num?)?.toDouble(),
      content: json['content'] as String? ?? '',
      snippet: json['snippet'] as String? ?? '',
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'chunk_id': chunkId,
        'document_id': documentId,
        'path': path,
        'file_name': fileName,
        'chunk_index': chunkIndex,
        'rrf_score': rrfScore,
        'bm25_rank': bm25Rank,
        'vector_rank': vectorRank,
        'cosine_similarity': cosineSimilarity,
        'content': content,
        'snippet': snippet,
      };
}
