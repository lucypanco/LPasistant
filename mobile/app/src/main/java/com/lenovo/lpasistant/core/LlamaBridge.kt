package com.lenovo.lpasistant.core

/**
 * llama.cpp(GGUF)JNI 桥。
 *
 * - nativeInit: 加载 GGUF 并创建推理上下文(单例,进程内一次)
 * - nativeInfer: 应用模型自带 ChatML 模板,贪心解码,上限 128 token,EOG 停止
 * - 每次推理清空 KV(无状态单回合;Rust harness 负责拼完整 prompt 与校验)
 */
object LlamaBridge {
    init {
        System.loadLibrary("llama_jni")
    }

    external fun nativeInit(modelPath: String): Boolean
    external fun nativeIsLoaded(): Boolean
    external fun nativeInfer(prompt: String): String
    external fun nativeFree()
}
