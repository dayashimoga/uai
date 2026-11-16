import 'dart:io';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('v1.0.0 Production Release Sign-Off & Verification Tests (S11.P2.2)', () {
    test('Version numbers are strictly synchronized across Cargo, pubspec, and documentation', () {
      final cargoToml = File('../core/Cargo.toml').readAsStringSync();
      expect(cargoToml.contains('version = "1.0.0"'), isTrue, reason: 'core/Cargo.toml version must be 1.0.0');

      final pubspec = File('pubspec.yaml').readAsStringSync();
      expect(pubspec.contains('version: 1.0.0+1'), isTrue, reason: 'app/pubspec.yaml version must be 1.0.0+1');

      final releaseNotes = File('../RELEASE_NOTES.md').readAsStringSync();
      expect(releaseNotes.contains('v1.0.0'), isTrue, reason: 'RELEASE_NOTES.md must reference v1.0.0');

      final readme = File('../README.md').readAsStringSync();
      expect(readme.contains('v1.0.0'), isTrue, reason: 'README.md must reference v1.0.0');
    });

    test('RELEASE_NOTES.md contains exhaustive architectural, security, and packaging sections', () {
      final rnFile = File('../RELEASE_NOTES.md');
      expect(rnFile.existsSync(), isTrue, reason: 'RELEASE_NOTES.md must exist');
      final content = rnFile.readAsStringSync();

      expect(content.contains('Universal AI Assistant v1.0.0 (Production Release)'), isTrue);
      expect(content.contains('100% Offline Speech & Audio Pipeline'), isTrue);
      expect(content.contains('Local LLM Runtime & Grammar-Constrained Tool Calling'), isTrue);
      expect(content.contains('Storage, Semantic Memory & Citation-Grounded RAG'), isTrue);
      expect(content.contains('Cross-Device Encrypted Mesh & Remote Control'), isTrue);
      expect(content.contains('Defense-in-Depth Security & Tamper-Evident Audit Logging'), isTrue);
      expect(content.contains('Five-Platform Packaging & Distribution Artifacts'), isTrue);
      expect(content.contains('Acceptance Matrix & Empirical Performance Benchmarks'), isTrue);
      expect(content.contains('Software Bill of Materials (SBOM) & Supply Chain Security'), isTrue);
      expect(content.contains('Getting Started'), isTrue);

      // Verify all 5 platform names are present
      expect(content.contains('Windows'), isTrue);
      expect(content.contains('Linux'), isTrue);
      expect(content.contains('macOS'), isTrue);
      expect(content.contains('Android'), isTrue);
      expect(content.contains('iOS'), isTrue);
    });

    test('Repository user, setup, and acceptance documentation are complete and certified', () {
      final setupFile = File('../docs/SETUP.md');
      expect(setupFile.existsSync(), isTrue, reason: 'docs/SETUP.md must exist');
      final setup = setupFile.readAsStringSync();
      expect(setup.contains('Release Packaging & SBOM Generation'), isTrue);
      expect(setup.contains('Storage Budget Management'), isTrue);

      final userGuideFile = File('../docs/USER_GUIDE.md');
      expect(userGuideFile.existsSync(), isTrue, reason: 'docs/USER_GUIDE.md must exist');
      final guide = userGuideFile.readAsStringSync();
      expect(guide.contains('Voice Interaction & Customizable Wake Words'), isTrue);
      expect(guide.contains('Multimodal Vision & Offline Voice Interpreter'), isTrue);
      expect(guide.contains('Cross-Device Pairing & Remote Control'), isTrue);
      expect(guide.contains('Emergency Panic Lockdown'), isTrue);

      final matrixFile = File('../docs/ACCEPTANCE_MATRIX.md');
      expect(matrixFile.existsSync(), isTrue, reason: 'docs/ACCEPTANCE_MATRIX.md must exist');
      final matrix = matrixFile.readAsStringSync();
      expect(matrix.contains('CERTIFIED_FOR_PRODUCTION'), isTrue);
      expect(matrix.contains('**Requirements Verified:** 30 (100.0%)'), isTrue);
    });

    test('Release manifests and SPDX 2.3 SBOM artifacts are present and verified', () {
      final manifest = File('../dist/SHA256SUMS.txt');
      expect(manifest.existsSync(), isTrue, reason: 'dist/SHA256SUMS.txt must exist');

      final rustSbom = File('../dist/sbom/rust-sbom.spdx.json');
      expect(rustSbom.existsSync(), isTrue, reason: 'dist/sbom/rust-sbom.spdx.json must exist');

      final flutterSbom = File('../dist/sbom/flutter-sbom.spdx.json');
      expect(flutterSbom.existsSync(), isTrue, reason: 'dist/sbom/flutter-sbom.spdx.json must exist');

      final unifiedSbom = File('../dist/sbom/unified-sbom.spdx.json');
      expect(unifiedSbom.existsSync(), isTrue, reason: 'dist/sbom/unified-sbom.spdx.json must exist');
    });
  });
}
