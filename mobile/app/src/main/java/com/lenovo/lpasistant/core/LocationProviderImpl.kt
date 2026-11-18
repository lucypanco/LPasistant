package com.lenovo.lpasistant.core

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import android.os.Looper
import androidx.core.content.ContextCompat
import uniffi.lpasistant_core.LocationProvider
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * 定位回调(仅用于天气):返回 "lat,lon";未授权/失败返回空串。
 *
 * 隐私设计:
 * - 系统 LocationManager:缓存位置即时读;「刷新」时主动请求一次单发定位(不持续、
 *   不引入第三方定位SDK),20 秒超时,任一 provider 回调即取用;
 * - 坐标只发给 Open-Meteo(天气计算所必需),坐标直供、不经逆地理编码,
 *   不存历史、不发给任何其他方;
 * - 是否启用由用户在「设置」页决定,默认关闭;权限为运行时授权,可随时撤销。
 */
class LocationProviderImpl(private val ctx: Context) : LocationProvider {

    private fun granted(): Boolean =
        ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED ||
            ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_COARSE_LOCATION) == PackageManager.PERMISSION_GRANTED

    override fun currentCoords(): String {
        return try {
            if (!granted()) return ""
            val lm = ctx.getSystemService(Context.LOCATION_SERVICE) as LocationManager
            var best: Location? = lm.getLastKnownLocation(LocationManager.GPS_PROVIDER)
            for (p in listOf(LocationManager.NETWORK_PROVIDER, LocationManager.PASSIVE_PROVIDER)) {
                val l = try { lm.getLastKnownLocation(p) } catch (_: Exception) { null }
                if (l != null && (best == null || l.time > best.time)) best = l
            }
            best?.let { "${it.latitude},${it.longitude}" } ?: ""
        } catch (_: Exception) {
            ""
        }
    }

    /**
     * 主动单发定位:requestSingleUpdate 逐 provider 注册,任一回调即返回;
     * 超时返回空串(调用方自行回退 lastKnown)。阻塞调用,须在 IO 线程。
     */
    fun requestOnce(timeoutMs: Long = 20000L): String {
        if (!granted()) return ""
        val lm = ctx.getSystemService(Context.LOCATION_SERVICE) as LocationManager
        val providers = listOf("fused", LocationManager.GPS_PROVIDER, LocationManager.NETWORK_PROVIDER)
            .filter { p -> try { lm.isProviderEnabled(p) } catch (_: Exception) { false } }
        if (providers.isEmpty()) return ""
        var result: Location? = null
        val latch = CountDownLatch(1)
        val listener = LocationListener { loc ->
            if (result == null) {
                result = loc
                latch.countDown()
            }
        }
        try {
            for (p in providers) {
                try { lm.requestSingleUpdate(p, listener, Looper.getMainLooper()) } catch (_: Exception) { }
            }
            latch.await(timeoutMs, TimeUnit.MILLISECONDS)
        } catch (_: Exception) {
        } finally {
            try { lm.removeUpdates(listener) } catch (_: Exception) { }
        }
        val loc = result ?: return ""
        return "${loc.latitude},${loc.longitude}"
    }
}
