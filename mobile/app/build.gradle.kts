plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.lenovo.lpasistant"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.lenovo.lpasistant"
        minSdk = 26
        targetSdk = 34
        versionCode = 1
        versionName = "1.0"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // 自用发布:复用 debug 密钥签名(免密钥管理;日后对外分发再换正式 keystore)。
            // release 变体不含 debugImplementation 的 ui-tooling,PreviewActivity 等调试组件随之消失。
            signingConfig = signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
    buildFeatures {
        compose = true
    }
    packaging {
        jniLibs {
            // Rust 产物 liblpasistant_core.so 由 cargo-ndk 输出到 src/main/jniLibs/<abi>/
        }
    }
    // UniFFI 生成的 Kotlin 绑定就在 src/main/java 下(uniffi/lpasistant_core/…),默认已包含
}

dependencies {
    implementation(platform("androidx.compose:compose-bom:2024.09.03"))
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.activity:activity-compose:1.9.2")
    implementation("androidx.core:core-ktx:1.13.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.6")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.8.1")
    // UniFFI 生成的绑定依赖 JNA
    implementation("net.java.dev.jna:jna:5.13.0@aar")
    // 本地 LLM 兜底:llama.cpp(NDK 静态编译,GGUF 直读;自建 JNI 桥,无第三方依赖)
    // 产物 libllama_jni.so 由 scripts/llama_jni_build.sh 编译并放入 jniLibs
    // 电台播放(Media3/ExoPlayer,HLS 与 Icecast 均支持)
    // media3-session:MediaSessionService 常驻后台播放 + 系统通知栏媒体控件
    implementation("androidx.media3:media3-exoplayer:1.4.1")
    implementation("androidx.media3:media3-exoplayer-hls:1.4.1")
    implementation("androidx.media3:media3-session:1.4.1")
    debugImplementation("androidx.compose.ui:ui-tooling")
}
