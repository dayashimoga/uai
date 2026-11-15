package com.aissistant.aissistant

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.media.session.MediaSession
import android.media.session.PlaybackState
import android.os.Build

/**
 * Android MediaSession and AudioFocus manager.
 * Handles lockscreen media controls, Bluetooth headset hardware buttons,
 * and audio focus ducking/pauses during incoming phone calls or alarms.
 */
class MediaSessionManager(
    private val context: Context,
    private val onMediaCommand: (String) -> Unit,
    private val onAudioFocusLost: () -> Unit
) {

    private val audioManager = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
    private var mediaSession: MediaSession? = null
    private var audioFocusRequest: AudioFocusRequest? = null

    private val focusChangeListener = AudioManager.OnAudioFocusChangeListener { focusChange ->
        when (focusChange) {
            AudioManager.AUDIOFOCUS_LOSS,
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT,
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK -> {
                onAudioFocusLost()
            }
            AudioManager.AUDIOFOCUS_GAIN -> {
                // Audio focus recovered
            }
        }
    }

    fun initialize() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            val session = MediaSession(context, "AIssistantMediaSession").apply {
                setCallback(object : MediaSession.Callback() {
                    override fun onPlay() {
                        onMediaCommand("play")
                        updatePlaybackState(PlaybackState.STATE_PLAYING)
                    }

                    override fun onPause() {
                        onMediaCommand("pause")
                        updatePlaybackState(PlaybackState.STATE_PAUSED)
                    }

                    override fun onSkipToNext() {
                        onMediaCommand("next")
                    }

                    override fun onSkipToPrevious() {
                        onMediaCommand("previous")
                    }

                    override fun onStop() {
                        onMediaCommand("stop")
                        updatePlaybackState(PlaybackState.STATE_STOPPED)
                    }
                })

                setFlags(
                    MediaSession.FLAG_HANDLES_MEDIA_BUTTONS or
                        MediaSession.FLAG_HANDLES_TRANSPORT_CONTROLS
                )
                isActive = true
            }
            mediaSession = session
            updatePlaybackState(PlaybackState.STATE_PAUSED)
        }
    }

    fun requestAudioFocus(): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val attributes = AudioAttributes.Builder()
                .setUsage(AudioAttributes.USAGE_ASSISTANCE_NAVIGATION_GUIDANCE)
                .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                .build()

            val request = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_MAY_DUCK)
                .setAudioAttributes(attributes)
                .setOnAudioFocusChangeListener(focusChangeListener)
                .build()

            audioFocusRequest = request
            audioManager.requestAudioFocus(request) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
        } else {
            @Suppress("DEPRECATION")
            audioManager.requestAudioFocus(
                focusChangeListener,
                AudioManager.STREAM_MUSIC,
                AudioManager.AUDIOFOCUS_GAIN_TRANSIENT_MAY_DUCK
            ) == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
        }
    }

    fun abandonAudioFocus() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            audioFocusRequest?.let {
                audioManager.abandonAudioFocusRequest(it)
                audioFocusRequest = null
            }
        } else {
            @Suppress("DEPRECATION")
            audioManager.abandonAudioFocus(focusChangeListener)
        }
    }

    private fun updatePlaybackState(state: Int) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            val playbackState = PlaybackState.Builder()
                .setActions(
                    PlaybackState.ACTION_PLAY or
                        PlaybackState.ACTION_PAUSE or
                        PlaybackState.ACTION_PLAY_PAUSE or
                        PlaybackState.ACTION_SKIP_TO_NEXT or
                        PlaybackState.ACTION_SKIP_TO_PREVIOUS or
                        PlaybackState.ACTION_STOP
                )
                .setState(state, PlaybackState.PLAYBACK_POSITION_UNKNOWN, 1.0f)
                .build()

            mediaSession?.setPlaybackState(playbackState)
        }
    }

    fun release() {
        abandonAudioFocus()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            mediaSession?.let {
                it.isActive = false
                it.release()
            }
            mediaSession = null
        }
    }
}
