package com.lenovo.lpasistant.core

import android.content.ComponentName
import android.content.Context
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import com.google.common.util.concurrent.MoreExecutors

/**
 * 电台播放控制(连接 [RadioService] 的 MediaController):
 * - 播放走前台服务,App 退后台/锁屏继续播,通知栏有系统媒体控件;
 * - 对 UI 保持原 API(play/stop/toggle/nowPlaying),调用方零改动。
 */
object RadioController {

    private var controller: MediaController? = null

    /** 当前播放中的台名(null = 未播放);Compose 可观察 */
    var nowPlaying by mutableStateOf<String?>(null)
        private set

    /** 连接 service(幂等);就绪后回调 */
    private fun ensure(ctx: Context, onReady: (MediaController) -> Unit) {
        controller?.let { c -> if (c.isConnected) { onReady(c); return } }
        val token = SessionToken(
            ctx.applicationContext,
            ComponentName(ctx.applicationContext, RadioService::class.java)
        )
        val future = MediaController.Builder(ctx.applicationContext, token).buildAsync()
        future.addListener({
            try {
                val c = future.get()
                c.addListener(object : Player.Listener {
                    override fun onIsPlayingChanged(isPlaying: Boolean) {
                        // 以服务实际状态为准(通知栏直接操作也能同步回 UI)
                        nowPlaying = if (isPlaying) {
                            c.currentMediaItem?.mediaMetadata?.artist?.toString()
                        } else null
                    }
                })
                controller = c
                onReady(c)
            } catch (_: Exception) {
                // 连接失败:保持原状(UI 会感知 nowPlaying 未变化)
            }
        }, MoreExecutors.directExecutor())
    }

    fun play(ctx: Context, name: String, url: String) {
        ensure(ctx) { c ->
            val item = MediaItem.Builder()
                .setUri(url)
                .setMediaId(name)
                .setMediaMetadata(MediaMetadata.Builder().setTitle(name).setArtist(name).build())
                .build()
            c.setMediaItem(item)
            c.prepare()
            c.play()
        }
    }

    fun stop() {
        controller?.let { c ->
            c.stop()
            c.clearMediaItems()
        }
        nowPlaying = null
    }

    fun toggle(ctx: Context, name: String, url: String) {
        if (nowPlaying == name) stop() else play(ctx, name, url)
    }
}
