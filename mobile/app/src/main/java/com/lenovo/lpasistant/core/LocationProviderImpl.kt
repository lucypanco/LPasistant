package com.lenovo.lpasistant.core

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.location.Location
import android.location.LocationManager
import androidx.core.content.ContextCompat
import uniffi.lpasistant_core.LocationProvider

/**
 * 定位回调(仅用于天气):返回 "lat,lon";未授权/失败返回空串。
 *
 * 隐私设计:
 * - 只用系统 LocationManager 的最后已知位置(不主动持续定位、不引入第三方定位SDK);
 * - 坐标只发给 Open-Meteo(天气计算所必需),坐标直供、不经逆地理编码,
 *   不存历史、不发给任何其他方;
 * - 是否启用由用户在「设置」页决定,默认关闭;权限为运行时授权,可随时撤销。
 */
class LocationProviderImpl(private val ctx: Context) : LocationProvider {

    override fun currentCoords(): String {
        return try {
            val granted = ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED ||
                ContextCompat.checkSelfPermission(ctx, Manifest.permission.ACCESS_COARSE_LOCATION) == PackageManager.PERMISSION_GRANTED
            if (!granted) return ""
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
}
