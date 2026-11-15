package com.aissistant.aissistant

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.PowerManager
import android.provider.Settings
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {

    private val channelName = "com.aissistant/mobile_bridge"
    private var methodChannel: MethodChannel? = null
    private var mediaSessionManager: MediaSessionManager? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)

        val channel = MethodChannel(flutterEngine.dartExecutor.binaryMessenger, channelName)
        methodChannel = channel

        mediaSessionManager = MediaSessionManager(
            context = this,
            onMediaCommand = { cmd ->
                channel.invokeMethod("onMediaCommand", mapOf("command" to cmd))
            },
            onAudioFocusLost = {
                channel.invokeMethod("onAudioFocusLost", null)
            }
        ).apply {
            initialize()
        }

        channel.setMethodCallHandler { call, result ->
            when (call.method) {
                "startVoiceService" -> {
                    val intent = Intent(this, AssistantForegroundService::class.java).apply {
                        action = AssistantForegroundService.ACTION_START
                    }
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                        startForegroundService(intent)
                    } else {
                        startService(intent)
                    }
                    result.success(true)
                }

                "stopVoiceService" -> {
                    val intent = Intent(this, AssistantForegroundService::class.java).apply {
                        action = AssistantForegroundService.ACTION_STOP
                    }
                    startService(intent)
                    result.success(true)
                }

                "isVoiceServiceRunning" -> {
                    result.success(AssistantForegroundService.isServiceRunning)
                }

                "isAccessibilityEnabled" -> {
                    result.success(AssistantAccessibilityService.isRunning)
                }

                "openAccessibilitySettings" -> {
                    val intent = Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS).apply {
                        flags = Intent.FLAG_ACTIVITY_NEW_TASK
                    }
                    startActivity(intent)
                    result.success(true)
                }

                "isBatteryOptimizationIgnored" -> {
                    val pm = getSystemService(Context.POWER_SERVICE) as? PowerManager
                    val isIgnored = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                        pm?.isIgnoringBatteryOptimizations(packageName) ?: false
                    } else {
                        true
                    }
                    result.success(isIgnored)
                }

                "requestIgnoreBatteryOptimizations" -> {
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                        try {
                            val intent =
                                Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS).apply {
                                    data = Uri.parse("package:$packageName")
                                    flags = Intent.FLAG_ACTIVITY_NEW_TASK
                                }
                            startActivity(intent)
                            result.success(true)
                        } catch (e: Exception) {
                            result.error("INTENT_ERROR", e.message, null)
                        }
                    } else {
                        result.success(true)
                    }
                }

                "getInitialIntentPayload" -> {
                    result.success(extractIntentPayload(intent))
                }

                "getAccessibilityActiveWindow" -> {
                    val hierarchy =
                        AssistantAccessibilityService.instance?.getActiveWindowHierarchy()
                    result.success(hierarchy)
                }

                "clickAccessibilityNode" -> {
                    val text = call.argument<String>("text")
                    if (text.isNullOrBlank()) {
                        result.error("INVALID_ARGUMENT", "Missing text argument", null)
                    } else {
                        val clicked =
                            AssistantAccessibilityService.instance?.clickNodeByText(text) ?: false
                        result.success(clicked)
                    }
                }

                "requestAudioFocus" -> {
                    val granted = mediaSessionManager?.requestAudioFocus() ?: false
                    result.success(granted)
                }

                "abandonAudioFocus" -> {
                    mediaSessionManager?.abandonAudioFocus()
                    result.success(true)
                }

                else -> {
                    result.notImplemented()
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        val payload = extractIntentPayload(intent)
        if (payload.isNotEmpty()) {
            methodChannel?.invokeMethod("onIntentReceived", payload)
        }
    }

    override fun onDestroy() {
        mediaSessionManager?.release()
        mediaSessionManager = null
        methodChannel?.setMethodCallHandler(null)
        methodChannel = null
        super.onDestroy()
    }

    private fun extractIntentPayload(intent: Intent?): Map<String, Any?> {
        val payload = mutableMapOf<String, Any?>()
        if (intent == null) return payload

        payload["action"] = intent.action
        payload["data"] = intent.dataString
        payload["type"] = intent.type

        if (Intent.ACTION_SEND == intent.action && "text/plain" == intent.type) {
            payload["text"] = intent.getStringExtra(Intent.EXTRA_TEXT)
        } else if (Intent.ACTION_ASSIST == intent.action) {
            payload["isAssist"] = true
        }

        return payload
    }
}
