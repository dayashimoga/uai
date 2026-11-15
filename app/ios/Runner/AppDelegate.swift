import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {

  private let channelName = "com.aissistant/ios_bridge"
  private var methodChannel: FlutterMethodChannel?
  private let audioSessionManager = AudioSessionManager()
  private var initialShortcutPayload: [String: Any]?

  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    GeneratedPluginRegistrant.register(with: self)

    if let url = launchOptions?[.url] as? URL {
      initialShortcutPayload = extractUrlPayload(url)
    }

    let controller = window?.rootViewController as? FlutterViewController
    if let messenger = controller?.binaryMessenger {
      setupMethodChannel(with: messenger)
    }

    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    setupMethodChannel(with: engineBridge.applicationEngine.binaryMessenger)
  }

  private func setupMethodChannel(with messenger: FlutterBinaryMessenger) {
    let channel = FlutterMethodChannel(name: channelName, binaryMessenger: messenger)
    methodChannel = channel

    audioSessionManager.onInterruption = { [weak channel] type in
      channel?.invokeMethod("onAudioInterruption", ["type": type])
    }

    audioSessionManager.onRouteChange = { [weak channel] reason in
      channel?.invokeMethod("onAudioRouteChange", ["reason": reason])
    }

    channel.setMethodCallHandler { [weak self] (call, result) in
      guard let self = self else { return }

      switch call.method {
      case "configureAudioSession":
        do {
          try self.audioSessionManager.configureSession()
          result(true)
        } catch {
          result(FlutterError(code: "AUDIO_SESSION_ERROR", message: error.localizedDescription, details: nil))
        }

      case "activateAudioSession":
        let args = call.arguments as? [String: Any]
        let active = args?["active"] as? Bool ?? true
        do {
          try self.audioSessionManager.setActive(active)
          result(true)
        } catch {
          result(FlutterError(code: "AUDIO_SESSION_ERROR", message: error.localizedDescription, details: nil))
        }

      case "isOtherAudioPlaying":
        result(self.audioSessionManager.isOtherAudioPlaying())

      case "startAccessingSecurityScopedResource":
        let args = call.arguments as? [String: Any]
        guard let urlString = args?["url"] as? String,
              let url = URL(string: urlString) else {
          result(false)
          return
        }
        let success = url.startAccessingSecurityScopedResource()
        result(success)

      case "stopAccessingSecurityScopedResource":
        let args = call.arguments as? [String: Any]
        if let urlString = args?["url"] as? String,
           let url = URL(string: urlString) {
          url.stopAccessingSecurityScopedResource()
        }
        result(true)

      case "getInitialShortcutInvocation":
        result(self.initialShortcutPayload)

      default:
        result(FlutterMethodNotImplemented)
      }
    }
  }

  override func application(
    _ app: UIApplication,
    open url: URL,
    options: [UIApplication.OpenURLOptionsKey: Any] = [:]
  ) -> Bool {
    let payload = extractUrlPayload(url)
    if let channel = methodChannel {
      channel.invokeMethod("onShortcutTriggered", payload)
    } else {
      initialShortcutPayload = payload
    }
    return true
  }

  override func application(
    _ application: UIApplication,
    continue userActivity: NSUserActivity,
    restorationHandler: @escaping ([UIUserActivityRestoring]?) -> Void
  ) -> Bool {
    var payload: [String: Any] = [
      "activityType": userActivity.activityType
    ]
    if let userInfo = userActivity.userInfo {
      payload["userInfo"] = userInfo
    }

    if let channel = methodChannel {
      channel.invokeMethod("onShortcutTriggered", payload)
    } else {
      initialShortcutPayload = payload
    }
    return true
  }

  private func extractUrlPayload(_ url: URL) -> [String: Any] {
    var payload: [String: Any] = [
      "url": url.absoluteString,
      "scheme": url.scheme ?? "",
      "host": url.host ?? ""
    ]

    if let components = URLComponents(url: url, resolvingAgainstBaseURL: false),
       let queryItems = components.queryItems {
      for item in queryItems {
        payload[item.name] = item.value
      }
    }

    return payload
  }
}
