import AVFoundation
import Foundation

/// Manages iOS AVAudioSession lifecycle, audio ducking, route transitions,
/// and interruptions (incoming cellular / FaceTime calls, Siri, alarms).
class AudioSessionManager {
    private let audioSession = AVAudioSession.sharedInstance()
    private var isConfigured = false

    var onInterruption: ((String) -> Void)?
    var onRouteChange: ((String) -> Void)?

    func configureSession() throws {
        guard !isConfigured else { return }

        var options: AVAudioSession.CategoryOptions = [
            .duckOthers,
            .defaultToSpeaker,
            .allowBluetooth
        ]

        if #available(iOS 10.0, *) {
            options.insert(.allowBluetoothA2DP)
        }

        try audioSession.setCategory(
            .playAndRecord,
            mode: .spokenAudio,
            options: options
        )

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleInterruption),
            name: AVAudioSession.interruptionNotification,
            object: audioSession
        )

        NotificationCenter.default.addObserver(
            self,
            selector: #selector(handleRouteChange),
            name: AVAudioSession.routeChangeNotification,
            object: audioSession
        )

        isConfigured = true
    }

    func setActive(_ active: Bool) throws {
        if active {
            try audioSession.setActive(true)
        } else {
            try audioSession.setActive(false, options: .notifyOthersOnDeactivation)
        }
    }

    func isOtherAudioPlaying() -> Bool {
        return audioSession.isOtherAudioPlaying
    }

    @objc private func handleInterruption(notification: Notification) {
        guard let userInfo = notification.userInfo,
              let typeValue = userInfo[AVAudioSessionInterruptionTypeKey] as? UInt,
              let type = AVAudioSession.InterruptionType(rawValue: typeValue) else {
            return
        }

        switch type {
        case .began:
            onInterruption?("began")
        case .ended:
            if let optionsValue = userInfo[AVAudioSessionInterruptionOptionKey] as? UInt {
                let options = AVAudioSession.InterruptionOptions(rawValue: optionsValue)
                if options.contains(.shouldResume) {
                    onInterruption?("ended_resume")
                } else {
                    onInterruption?("ended")
                }
            } else {
                onInterruption?("ended")
            }
        @unknown default:
            break
        }
    }

    @objc private func handleRouteChange(notification: Notification) {
        guard let userInfo = notification.userInfo,
              let reasonValue = userInfo[AVAudioSessionRouteChangeReasonKey] as? UInt,
              let reason = AVAudioSession.RouteChangeReason(rawValue: reasonValue) else {
            return
        }

        switch reason {
        case .oldDeviceUnavailable:
            // e.g. Headphones unplugged
            onRouteChange?("headset_disconnected")
        case .newDeviceAvailable:
            // e.g. Headphones or Bluetooth connected
            onRouteChange?("headset_connected")
        default:
            onRouteChange?("route_changed")
        }
    }

    deinit {
        NotificationCenter.default.removeObserver(self)
    }
}
