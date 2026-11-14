package com.lenovo.lpasistant.core

import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.media3.common.MediaItem
import androidx.media3.exoplayer.ExoPlayer

/**
 * 电台播放(Media3/ExoPlayer):支持 HLS(m3u8)与 Icecast(mp3/aac)流。
 *
 * MVP 边界:App 前台可听;退到后台系统可能回收音频(没有前台服务)。
 * 常驻后台播放需要 MediaSessionService,列为后续增强。
 */
object RadioController {

    private var player: ExoPlayer? = null

    /** 当前播放中的台名(null = 未播放);Compose 可观察 */
    var nowPlaying by mutableStateOf<String?>(null)
        private set

    fun play(ctx: Context, name: String, url: String) {
        val p = player ?: ExoPlayer.Builder(ctx.applicationContext).build()
            .also { player = it }
        p.setMediaItem(MediaItem.fromUri(url))
        p.prepare()
        p.playWhenReady = true
        nowPlaying = name
    }

    fun stop() {
        player?.stop()
        nowPlaying = null
    }

    fun toggle(ctx: Context, name: String, url: String) {
        if (nowPlaying == name) stop() else play(ctx, name, url)
    }
}
