package com.lenovo.lpasistant.core

import android.content.Context
import android.util.Log
import uniffi.lpasistant_core.LlmInfer
import java.io.File

/**
 * AI兜底(仅本地模型,纯离线):规则解析失败时由 Rust 核心发起一次推理调用。
 *
 * 模型文件:GGUF(Qwen2.5-0.5B-Instruct Q4_K_M,约468MB)放到
 *   /sdcard/Android/data/com.lenovo.lpasistant/files/model.gguf
 * 开关在「设置」页;未开启时 Rust 侧不会调用本回调,零数据外流。
 *
 * 推理后端 = llama.cpp(NDK 静态编译,直接吃 GGUF;2026-11-15 弃 MediaPipe
 * tasks-genai——其 odml 格式与第三方模型转换链不兼容,详见 audit 记录)。
 * 职责边界:模型只把句子"翻译"成受限 JSON 字段;字段校验、日期计算、
 * 二步确认、落库全部在 Rust 侧本地代码完成。
 * 诊断:全链路打 Logcat(tag=LlmController),便于真机排查模型加载/输出。
 */
class LlmInferImpl(private val ctx: Context) : LlmInfer {

    override fun infer(prompt: String): String = try {
        Log.i(LlmController.TAG, "infer 收到prompt: ${prompt.take(150)}")
        if (LlmController.ensureEngine(ctx)) {
            val out = LlmController.inferLocal(prompt)
            Log.i(LlmController.TAG, "infer 返回: ${out?.take(300) ?: "(null)"}")
            out ?: ""
        } else {
            Log.w(LlmController.TAG, "engine 未就绪,返回空串")
            ""
        }
    } catch (e: Exception) {
        Log.e(LlmController.TAG, "infer 异常", e)
        ""
    }

    companion object {
        const val TAG = "LlmController"
    }
}

object LlmController {

    const val TAG = "LlmController"

    @Volatile
    private var ready = false

    fun modelFile(ctx: Context): File {
        val dir = ctx.getExternalFilesDir(null) ?: ctx.filesDir
        return File(dir, "model.gguf")
    }

    fun modelStatus(ctx: Context): String {
        val f = modelFile(ctx)
        return when {
            !f.exists() -> "未就绪:未找到模型文件"
            f.length() < 10_000_000L -> "未就绪:文件过小(${f.length() / 1024}KB),不是有效模型"
            ready -> "就绪(已加载,${f.length() / 1024 / 1024}MB)"
            else -> "文件就绪(${f.length() / 1024 / 1024}MB),首次使用时自动加载"
        }
    }

    /** 模型文件存在则惰性加载;成功返回 true(文件可随时补放) */
    @Synchronized
    fun ensureEngine(ctx: Context): Boolean {
        if (ready) return true
        val f = modelFile(ctx)
        if (!f.exists() || f.length() < 10_000_000L) {
            Log.w(TAG, "模型文件不存在/过小: ${f.absolutePath}")
            return false
        }
        ready = try {
            val ok = LlamaBridge.nativeInit(f.absolutePath)
            Log.i(TAG, if (ok) "engine 创建成功(${f.length() / 1024 / 1024}MB, llama.cpp)" else "engine 创建失败(llama.cpp 拒载)")
            ok
        } catch (t: Throwable) {
            Log.e(TAG, "engine init 异常", t)
            false
        }
        return ready
    }

    @Synchronized
    fun inferLocal(prompt: String): String? = try {
        val out = LlamaBridge.nativeInfer(prompt)
        Log.i(TAG, "infer 返回: ${out.take(300)}")
        out
    } catch (t: Throwable) {
        Log.e(TAG, "infer 异常: ${t.message}", t)
        null
    }
}
