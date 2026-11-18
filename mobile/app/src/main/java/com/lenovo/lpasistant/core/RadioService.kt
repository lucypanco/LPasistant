package com.lenovo.lpasistant.core

import android.content.Intent
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * 电台常驻播放服务(Media3 MediaSessionService):
 * - 点 ▶ 后进入前台服务,App 退后台/锁屏仍继续播;
 * - 系统通知栏媒体控件(播放/暂停/停止)由 media3 自动生成;
 * - 隐私边界不变:仅拉取用户点播的电台流,无采集、无上报。
 */
class RadioService : MediaSessionService() {

    private var mediaSession: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        val player = ExoPlayer.Builder(this)
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
                    .build(),
                true // 音频焦点:抢占/被抢都按媒体语义处理
            )
            .setHandleAudioBecomingNoisy(true) // 拔耳机自动暂停
            .build()
        mediaSession = MediaSession.Builder(this, player).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        mediaSession

    @Deprecated("Deprecated in Java")
    override fun onTaskRemoved(rootIntent: Intent?) {
        // 从最近任务划掉 = 用户明确要停:释放会话结束服务
        val p = mediaSession?.player
        if (p == null || !p.playWhenReady || p.mediaItemCount == 0) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        mediaSession?.run {
            player.release()
            release()
        }
        mediaSession = null
        super.onDestroy()
    }
}
