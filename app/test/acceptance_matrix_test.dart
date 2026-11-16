import 'dart:convert';
import 'dart:io';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('Full Acceptance Matrix & SBOM Verification Tests (S11.P2.1)', () {
    test('ACCEPTANCE_MATRIX.md contains complete traceability for all 30 requirements', () {
      final matrixFile = File('../docs/ACCEPTANCE_MATRIX.md');
      expect(matrixFile.existsSync(), isTrue, reason: 'docs/ACCEPTANCE_MATRIX.md must exist');
      final content = matrixFile.readAsStringSync();

      // Functional Requirements (FR-1 to FR-5)
      final expectedFr = [
        'FR-1.1', 'FR-1.2', 'FR-1.3', 'FR-1.4', 'FR-1.5',
        'FR-2.1', 'FR-2.2', 'FR-2.3', 'FR-2.4',
        'FR-3.1', 'FR-3.2', 'FR-3.3', 'FR-3.4',
        'FR-4.1', 'FR-4.2', 'FR-4.3', 'FR-4.4',
        'FR-5.1', 'FR-5.2', 'FR-5.3', 'FR-5.4', 'FR-5.5', 'FR-5.6',
      ];
      for (final fr in expectedFr) {
        expect(content.contains(fr), isTrue, reason: 'Requirement $fr must be present in matrix');
      }

      // Non-Functional Requirements (NFR-1 to NFR-8)
      final expectedNfr = [
        'NFR-1', 'NFR-2', 'NFR-3', 'NFR-4', 'NFR-5', 'NFR-6', 'NFR-7', 'NFR-8',
      ];
      for (final nfr in expectedNfr) {
        expect(content.contains(nfr), isTrue, reason: 'Requirement $nfr must be present in matrix');
      }

      // Security Requirements (SEC-1 to SEC-6)
      final expectedSec = ['SEC-1', 'SEC-2', 'SEC-3', 'SEC-4', 'SEC-5', 'SEC-6'];
      for (final sec in expectedSec) {
        expect(content.contains(sec), isTrue, reason: 'Security requirement $sec must be present in matrix');
      }

      // Privacy Requirements (PRIV-1 to PRIV-4)
      final expectedPriv = ['PRIV-1', 'PRIV-2', 'PRIV-3', 'PRIV-4'];
      for (final priv in expectedPriv) {
        expect(content.contains(priv), isTrue, reason: 'Privacy requirement $priv must be present in matrix');
      }

      expect(content.contains('CERTIFIED_FOR_PRODUCTION'), isTrue);
      expect(content.contains('**Requirements Verified:** 30 (100.0%)'), isTrue);
    });

    test('generate_sbom.py produces valid SPDX 2.3 JSON documents for Rust, Flutter and Unified stack', () {
      final scriptFile = File('../scripts/generate_sbom.py');
      expect(scriptFile.existsSync(), isTrue, reason: 'scripts/generate_sbom.py must exist');

      final result = Process.runSync('python', [
        '../scripts/generate_sbom.py',
        '--output-dir', '../dist/sbom',
        '--cargo-lock', '../core/Cargo.lock',
        '--pubspec-lock', 'pubspec.lock',
      ]);
      expect(result.exitCode, 0, reason: 'generate_sbom.py must execute cleanly: ${result.stderr}');

      final sbomFiles = [
        '../dist/sbom/rust-sbom.spdx.json',
        '../dist/sbom/flutter-sbom.spdx.json',
        '../dist/sbom/unified-sbom.spdx.json',
      ];

      for (final filePath in sbomFiles) {
        final f = File(filePath);
        expect(f.existsSync(), isTrue, reason: '$filePath must be generated');

        final doc = jsonDecode(f.readAsStringSync()) as Map<String, dynamic>;
        expect(doc['spdxVersion'], 'SPDX-2.3');
        expect(doc['dataLicense'], 'CC0-1.0');
        expect(doc['SPDXID'], 'SPDXRef-DOCUMENT');
        expect(doc['creationInfo'], isNotNull);
        expect(doc['creationInfo']['creators'], isList);

        final packages = doc['packages'] as List<dynamic>;
        expect(packages.isNotEmpty, isTrue, reason: 'Packages list must not be empty');

        final relationships = doc['relationships'] as List<dynamic>;
        expect(relationships.isNotEmpty, isTrue, reason: 'Relationships list must not be empty');
      }
    });

    test('verify_release_checksums.py generates unified SHA256SUMS.txt and passes verification', () {
      final verifierScript = File('../scripts/verify_release_checksums.py');
      expect(verifierScript.existsSync(), isTrue, reason: 'verify_release_checksums.py must exist');

      final result = Process.runSync('python', [
        '../scripts/verify_release_checksums.py',
        '--dist-dir', '../dist',
        '--verify',
      ]);
      expect(result.exitCode, 0, reason: 'verify_release_checksums.py must verify successfully: ${result.stderr}');

      final manifestFile = File('../dist/SHA256SUMS.txt');
      expect(manifestFile.existsSync(), isTrue, reason: 'SHA256SUMS.txt must be generated');

      final manifestText = manifestFile.readAsStringSync();
      expect(manifestText.contains('rust-sbom.spdx.json'), isTrue);
      expect(manifestText.contains('flutter-sbom.spdx.json'), isTrue);
      expect(manifestText.contains('unified-sbom.spdx.json'), isTrue);
    });
  });
}
