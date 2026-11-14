package com.lenovo.lpasistant.core

import android.content.Context
import android.util.Log
import com.google.mediapipe.tasks.genai.llminference.LlmInference
import uniffi.lpasistant_core.LlmInfer
import java.io.File

/**
 * AI兜底(仅本地模型,纯离线):规则解析失败时由 Rust 核心发起一次推理调用。
 *
 * 模型文件:GGUF(Qwen2.5-0.5B-Instruct Q4_K_M,约400MB)放到
 *   /sdcard/Android/data/com.lenovo.lpasistant/files/model.gguf
 * 开关在「设置」页;未开启时 Rust 侧不会调用本回调,零数据外流。
 *
 * 职责边界:模型只把句子"翻译"成受限 JSON 字段;字段校验、日期计算、
 * 二步确认、落库全部在 Rust 侧本地代码完成。
 * 诊断:全链路打 Logcat(tag=LlmController),便于真机排查模型加载/输出。
 */
class LlmInferImpl(private val ctx: Context) : LlmInfer {

    override fun infer(prompt: String): String = try {
        Log.i(TAG, "infer 收到prompt: ${prompt.take(150)}")
        if (LlmController.ensureEngine(ctx)) {
            val out = LlmController.inferLocal(prompt)
            Log.i(TAG, "infer 返回: ${out?.take(300) ?: "(null)"}")
            out ?: ""
        } else {
            Log.w(TAG, "engine 未就绪,返回空串")
            ""
        }
    } catch (e: Exception) {
        Log.e(TAG, "infer 异常", e)
        ""
    }

    companion object {
        const val TAG = "LlmController"
    }
}

object LlmController {

    private const val TAG = "LlmController"

    @Volatile
    private var engine: LlmInference? = null

    fun modelFile(ctx: Context): File {
        val dir = ctx.getExternalFilesDir(null) ?: ctx.filesDir
        return File(dir, "model.gguf")
    }

    fun modelStatus(ctx: Context): String {
        val f = modelFile(ctx)
        return when {
            !f.exists() -> "未就绪:未找到模型文件"
            f.length() < 10_000_000L -> "未就绪:文件过小(${f.length() / 1024}KB),不是有效模型"
            engine != null -> "就绪(已加载,${f.length() / 1024 / 1024}MB)"
            else -> "文件就绪(${f.length() / 1024 / 1024}MB),首次使用时自动加载"
        }
    }

    /** 模型文件存在则惰性加载;成功返回 true(文件可随时补放) */
    @Synchronized
    fun ensureEngine(ctx: Context): Boolean {
        engine?.let { return true }
        val f = modelFile(ctx)
        if (!f.exists() || f.length() < 10_000_000L) return false
        return try {
            val opts = LlmInference.LlmInferenceOptions.builder()
                .setModelPath(f.absolutePath)
                .setMaxTokens(128)
                .build()
            engine = LlmInference.createFromOptions(ctx, opts)
            Log.i(TAG, "engine 创建成功: ${f.length() / 1024 / 1024}MB")
            true
        } catch (e: Exception) {
            Log.e(TAG, "engine 创建失败: ${e.javaClass.simpleName}: ${e.message}", e)
            false
        }
    }

    fun inferLocal(prompt: String): String? = try {
        engine?.generateResponse(prompt)
    } catch (e: Exception) {
        Log.e(TAG, "generateResponse 异常: ${e.javaClass.simpleName}: ${e.message}", e)
        null
    }
}
