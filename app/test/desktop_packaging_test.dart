import 'dart:io';
import 'package:flutter_test/flutter_test.dart';

void main() {
  group('Desktop Release Packaging & Distribution Tests (S11.P1.1)', () {
    test('Windows packaging definitions contain required binaries, capabilities and metadata', () {
      final issFile = File('../packaging/desktop/windows/setup.iss');
      expect(issFile.existsSync(), isTrue, reason: 'setup.iss must exist');
      final iss = issFile.readAsStringSync();

      expect(iss.contains('Universal AI Assistant'), isTrue);
      expect(iss.contains('aissistant.exe'), isTrue);
      expect(iss.contains('aissistant_ffi.dll'), isTrue);
      expect(iss.contains('flutter_windows.dll'), isTrue);
      expect(iss.contains('{app}\\data'), isTrue);
      expect(iss.contains('Run'), isTrue);
      expect(iss.contains('desktopicon'), isTrue);

      final manifestFile = File('../packaging/desktop/windows/AppxManifest.xml');
      expect(manifestFile.existsSync(), isTrue, reason: 'AppxManifest.xml must exist');
      final manifest = manifestFile.readAsStringSync();

      expect(manifest.contains('Identity Name="UniversalAIAssistant"'), isTrue);
      expect(manifest.contains('runFullTrust'), isTrue);
      expect(manifest.contains('microphone'), isTrue);
      expect(manifest.contains('privateNetworkClientServer'), isTrue);
      expect(manifest.contains('Executable="aissistant.exe"'), isTrue);

      final psScript = File('../packaging/desktop/windows/package_windows.ps1');
      expect(psScript.existsSync(), isTrue, reason: 'package_windows.ps1 must exist');
      final ps = psScript.readAsStringSync();

      expect(ps.contains('cargo build'), isTrue);
      expect(ps.contains('aissistant-ffi'), isTrue);
      expect(ps.contains('flutter build windows --release'), isTrue);
      expect(ps.contains('Get-FileHash'), isTrue);
      expect(ps.contains('SHA256'), isTrue);
    });

    test('Linux packaging definitions conform to XDG and Debian/AppImage specifications', () {
      final desktopFile = File('../packaging/desktop/linux/aissistant.desktop');
      expect(desktopFile.existsSync(), isTrue, reason: 'aissistant.desktop must exist');
      final desktop = desktopFile.readAsStringSync();

      expect(desktop.contains('[Desktop Entry]'), isTrue);
      expect(desktop.contains('Type=Application'), isTrue);
      expect(desktop.contains('Name=Universal AI Assistant'), isTrue);
      expect(desktop.contains('Exec=aissistant %u'), isTrue);
      expect(desktop.contains('Icon=aissistant'), isTrue);
      expect(desktop.contains('StartupWMClass=aissistant'), isTrue);
      expect(desktop.contains('ArtificialIntelligence'), isTrue);

      final debScript = File('../packaging/desktop/linux/package_deb.sh');
      expect(debScript.existsSync(), isTrue, reason: 'package_deb.sh must exist');
      final deb = debScript.readAsStringSync();

      expect(deb.contains('Package: aissistant'), isTrue);
      expect(deb.contains('libgtk-3-0'), isTrue);
      expect(deb.contains('libpipewire-0.3-0'), isTrue);
      expect(deb.contains('libasound2'), isTrue);
      expect(deb.contains('libaissistant_ffi.so'), isTrue);
      expect(deb.contains('dpkg-deb'), isTrue);

      final appRun = File('../packaging/desktop/linux/AppRun');
      expect(appRun.existsSync(), isTrue, reason: 'AppRun must exist');
      final appRunContent = appRun.readAsStringSync();

      expect(appRunContent.contains('APPDIR'), isTrue);
      expect(appRunContent.contains('LD_LIBRARY_PATH'), isTrue);
      expect(appRunContent.contains('exec "\${APPDIR}/usr/bin/aissistant"'), isTrue);

      final appImageScript = File('../packaging/desktop/linux/package_appimage.sh');
      expect(appImageScript.existsSync(), isTrue, reason: 'package_appimage.sh must exist');
      final appImage = appImageScript.readAsStringSync();

      expect(appImage.contains('aissistant.AppDir'), isTrue);
      expect(appImage.contains('appimagetool'), isTrue);
      expect(appImage.contains('libaissistant_ffi.so'), isTrue);
    });

    test('macOS packaging definitions contain dylib embedding, code signing, and DMG generation', () {
      final macScript = File('../packaging/desktop/macos/package_macos.sh');
      expect(macScript.existsSync(), isTrue, reason: 'package_macos.sh must exist');
      final mac = macScript.readAsStringSync();

      expect(mac.contains('libaissistant_ffi.dylib'), isTrue);
      expect(mac.contains('flutter build macos --release'), isTrue);
      expect(mac.contains('Contents/Frameworks'), isTrue);
      expect(mac.contains('create_dmg.sh'), isTrue);

      final dmgScript = File('../packaging/desktop/macos/create_dmg.sh');
      expect(dmgScript.existsSync(), isTrue, reason: 'create_dmg.sh must exist');
      final dmg = dmgScript.readAsStringSync();

      expect(dmg.contains('hdiutil create'), isTrue);
      expect(dmg.contains('Universal AI Assistant'), isTrue);
      expect(dmg.contains('/Applications'), isTrue);
      expect(dmg.contains('shasum -a 256'), isTrue);
    });

    test('GitHub Actions desktop packaging workflow is valid and covers all three platforms', () {
      final workflowFile = File('../.github/workflows/desktop_packaging.yml');
      expect(workflowFile.existsSync(), isTrue, reason: 'desktop_packaging.yml must exist');
      final workflow = workflowFile.readAsStringSync();

      expect(workflow.contains('name: Desktop Release Packaging'), isTrue);
      expect(workflow.contains('runs-on: windows-latest'), isTrue);
      expect(workflow.contains('runs-on: ubuntu-latest'), isTrue);
      expect(workflow.contains('runs-on: macos-latest'), isTrue);
      expect(workflow.contains('package_windows.ps1'), isTrue);
      expect(workflow.contains('package_deb.sh'), isTrue);
      expect(workflow.contains('package_appimage.sh'), isTrue);
      expect(workflow.contains('package_macos.sh'), isTrue);
      expect(workflow.contains('SHA256SUMS_DESKTOP.txt'), isTrue);
    });
  });
}
