import 'package:flutter/foundation.dart';

/// Normalized 2D bounding box representing an object or text region in unit range `[0.0, 1.0]`.
@immutable
class OcrBoundingBoxItem {
  final double x;
  final double y;
  final double width;
  final double height;

  const OcrBoundingBoxItem({
    required this.x,
    required this.y,
    required this.width,
    required this.height,
  });

  factory OcrBoundingBoxItem.fromJson(Map<String, dynamic> json) {
    return OcrBoundingBoxItem(
      x: (json['x'] as num?)?.toDouble() ?? 0.0,
      y: (json['y'] as num?)?.toDouble() ?? 0.0,
      width: (json['width'] as num?)?.toDouble() ?? 0.0,
      height: (json['height'] as num?)?.toDouble() ?? 0.0,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'x': x,
        'y': y,
        'width': width,
        'height': height,
      };
}

/// An individual word recognized by OCR with its bounding box and confidence score.
@immutable
class OcrWordItem {
  final String text;
  final OcrBoundingBoxItem bbox;
  final double confidence;

  const OcrWordItem({
    required this.text,
    required this.bbox,
    required this.confidence,
  });

  factory OcrWordItem.fromJson(Map<String, dynamic> json) {
    return OcrWordItem(
      text: json['text'] as String? ?? '',
      bbox: OcrBoundingBoxItem.fromJson(
        json['bbox'] as Map<String, dynamic>? ?? <String, dynamic>{},
      ),
      confidence: (json['confidence'] as num?)?.toDouble() ?? 0.0,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'text': text,
        'bbox': bbox.toJson(),
        'confidence': confidence,
      };
}

/// A line of recognized text with bounding box and word components.
@immutable
class OcrTextLineItem {
  final int lineNumber;
  final String text;
  final OcrBoundingBoxItem bbox;
  final double confidence;
  final List<OcrWordItem> words;

  const OcrTextLineItem({
    required this.lineNumber,
    required this.text,
    required this.bbox,
    required this.confidence,
    required this.words,
  });

  factory OcrTextLineItem.fromJson(Map<String, dynamic> json) {
    final wordsList = json['words'] as List<dynamic>? ?? <dynamic>[];
    return OcrTextLineItem(
      lineNumber: (json['line_number'] as num?)?.toInt() ?? 0,
      text: json['text'] as String? ?? '',
      bbox: OcrBoundingBoxItem.fromJson(
        json['bbox'] as Map<String, dynamic>? ?? <String, dynamic>{},
      ),
      confidence: (json['confidence'] as num?)?.toDouble() ?? 0.0,
      words: wordsList
          .map((w) => OcrWordItem.fromJson(w as Map<String, dynamic>))
          .toList(),
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'line_number': lineNumber,
        'text': text,
        'bbox': bbox.toJson(),
        'confidence': confidence,
        'words': words.map((w) => w.toJson()).toList(),
      };
}

/// Complete OCR extraction output for an image.
@immutable
class OcrDocumentItem {
  final List<OcrTextLineItem> lines;
  final String fullText;
  final double averageConfidence;
  final int imageWidth;
  final int imageHeight;
  final int processingTimeMs;

  const OcrDocumentItem({
    required this.lines,
    required this.fullText,
    required this.averageConfidence,
    required this.imageWidth,
    required this.imageHeight,
    required this.processingTimeMs,
  });

  factory OcrDocumentItem.fromJson(Map<String, dynamic> json) {
    final linesList = json['lines'] as List<dynamic>? ?? <dynamic>[];
    return OcrDocumentItem(
      lines: linesList
          .map((l) => OcrTextLineItem.fromJson(l as Map<String, dynamic>))
          .toList(),
      fullText: json['full_text'] as String? ?? '',
      averageConfidence: (json['average_confidence'] as num?)?.toDouble() ?? 0.0,
      imageWidth: (json['image_width'] as num?)?.toInt() ?? 0,
      imageHeight: (json['image_height'] as num?)?.toInt() ?? 0,
      processingTimeMs: (json['processing_time_ms'] as num?)?.toInt() ?? 0,
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
        'lines': lines.map((l) => l.toJson()).toList(),
        'full_text': fullText,
        'average_confidence': averageConfidence,
        'image_width': imageWidth,
        'image_height': imageHeight,
        'processing_time_ms': processingTimeMs,
      };
}
