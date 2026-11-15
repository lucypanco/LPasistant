// LlamaBridge JNI:llama.cpp(GGUF)最小推理桥
// 接口: nativeInit(模型路径) / nativeInfer(prompt) / nativeIsLoaded / nativeFree
// 设计: 单回合无状态——每次 infer 清空 KV,由 Rust harness 拼完整 prompt;
//       贪心采样(路由 JSON 需要确定性);EOG(<|im_end|>/<|endoftext|>)停止;上限 128 token。

#include <jni.h>
#include <string>
#include <vector>
#include <mutex>
#include <cstring>

#include "llama.h"
#include <android/log.h>

#define LOGI(...) __android_log_print(ANDROID_LOG_INFO,  "LlamaBridge", __VA_ARGS__)
#define LOGE(...) __android_log_print(ANDROID_LOG_ERROR, "LlamaBridge", __VA_ARGS__)

static llama_model *  g_model = nullptr;
static llama_context *g_ctx   = nullptr;
static const llama_vocab *g_vocab = nullptr;
static std::mutex     g_mutex;

static constexpr int N_CTX     = 2048;
static constexpr int N_THREADS = 4;
static constexpr int MAX_NEW_TOKENS = 128;

extern "C" JNIEXPORT jboolean JNICALL
Java_com_lenovo_lpasistant_core_LlamaBridge_nativeInit(JNIEnv *env, jclass, jstring jpath) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_model) return JNI_TRUE;

    const char *path = env->GetStringUTFChars(jpath, nullptr);

    llama_model_params mp = llama_model_default_params();
    mp.n_gpu_layers = 0;  // 纯 CPU(手机 NPU/GPU 后端不在本桥范围)
    g_model = llama_model_load_from_file(path, mp);
    env->ReleaseStringUTFChars(jpath, path);
    if (!g_model) return JNI_FALSE;

    llama_context_params cp = llama_context_default_params();
    cp.n_ctx            = N_CTX;
    cp.n_batch          = 256;
    cp.n_ubatch         = 256;
    cp.n_threads        = N_THREADS;
    cp.n_threads_batch  = N_THREADS;
    cp.no_perf          = true;
    g_ctx = llama_init_from_model(g_model, cp);
    if (!g_ctx) { llama_model_free(g_model); g_model = nullptr; return JNI_FALSE; }
    g_vocab = llama_model_get_vocab(g_model);
    return JNI_TRUE;
}

extern "C" JNIEXPORT jboolean JNICALL
Java_com_lenovo_lpasistant_core_LlamaBridge_nativeIsLoaded(JNIEnv *, jclass) {
    return g_model && g_ctx ? JNI_TRUE : JNI_FALSE;
}

extern "C" JNIEXPORT void JNICALL
Java_com_lenovo_lpasistant_core_LlamaBridge_nativeFree(JNIEnv *, jclass) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (g_ctx)   { llama_free(g_ctx);          g_ctx = nullptr; }
    if (g_model) { llama_model_free(g_model);  g_model = nullptr; g_vocab = nullptr; }
}

extern "C" JNIEXPORT jstring JNICALL
Java_com_lenovo_lpasistant_core_LlamaBridge_nativeInfer(JNIEnv *env, jclass, jstring jprompt) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (!g_ctx || !g_model || !g_vocab) return env->NewStringUTF("");

    const char *prompt = env->GetStringUTFChars(jprompt, nullptr);

    // 1) 应用模型自带聊天模板(GGUF metadata 里的 chat_template,Qwen2.5=ChatML)
    llama_chat_message msg = { const_cast<char *>("user"), const_cast<char *>(prompt) };
    std::vector<char> formatted(N_CTX * 4, 0);
    int fitted = llama_chat_apply_template(
        nullptr,          // nullptr = 使用模型内置模板
        &msg, 1,
        true,             // 末尾追加 assistant 起始(生成从这里续)
        formatted.data(), (int)formatted.size());
    env->ReleaseStringUTFChars(jprompt, prompt);
    LOGI("template fitted=%d", fitted);
    if (fitted < 0) { LOGE("chat template 应用失败"); return env->NewStringUTF(""); }
    std::string text(formatted.data(), (size_t)std::min((long)fitted, (long)formatted.size() - 1));

    // 2) 分词
    int n_prompt = llama_tokenize(g_vocab, text.c_str(), (int32_t)text.size(),
                                  nullptr, 0, true, true);
    std::vector<llama_token> tokens;
    if (n_prompt < 0) {
        // llama.cpp 约定:负返回值 = 所需 token 数,按此分配后重试
        tokens.resize((size_t)(-n_prompt));
        n_prompt = llama_tokenize(g_vocab, text.c_str(), (int32_t)text.size(),
                                  tokens.data(), (int32_t)tokens.size(), true, true);
        if (n_prompt < 0) { LOGE("tokenize 二次调用失败"); return env->NewStringUTF(""); }
    }
    LOGI("tokenize %d token", n_prompt);
    if ((int)tokens.size() > N_CTX - MAX_NEW_TOKENS - 8) {
        size_t keep = (size_t)(N_CTX - MAX_NEW_TOKENS - 8);
        tokens.assign(tokens.end() - (long)keep, tokens.end());
        n_prompt = (int)tokens.size();
    }
    if ((int)tokens.size() > N_CTX - MAX_NEW_TOKENS - 8) {
        // 超长截断:保留末尾(指令在尾部)
        size_t keep = (size_t)(N_CTX - MAX_NEW_TOKENS - 8);
        tokens.assign(tokens.end() - (long)keep, tokens.end());
        n_prompt = (int)tokens.size();
    }

    // 3) 每次推理清空 KV(无状态单回合)
    llama_memory_clear(llama_get_memory(g_ctx), true);

    llama_batch batch = llama_batch_get_one(tokens.data(), n_prompt);
    llama_sampler_chain_params sp = llama_sampler_chain_default_params();
    llama_sampler *smpl = llama_sampler_chain_init(sp);
    llama_sampler_chain_add(smpl, llama_sampler_init_greedy());

    std::string out;
    llama_token new_id = 0;
    LOGI("prefill %d token 开始", n_prompt);
    for (int i = 0; i < MAX_NEW_TOKENS; i++) {
        if (llama_decode(g_ctx, batch) != 0) { LOGE("decode 失败 @%d", i); break; }
        new_id = llama_sampler_sample(smpl, g_ctx, -1);
        if (i == 0) LOGI("首 token=%d", new_id);
        if (llama_vocab_is_eog(g_vocab, new_id)) { LOGI("EOG @%d token=%d", i, new_id); break; }
        char piece[256];
        int n = llama_token_to_piece(g_vocab, new_id, piece, sizeof(piece), 0, false);
        if (n > 0) out.append(piece, (size_t)n);
        batch = llama_batch_get_one(&new_id, 1);
    }
    LOGI("生成结束,输出 %d 字符", (int)out.size());
    llama_sampler_free(smpl);
    return env->NewStringUTF(out.c_str());
}
