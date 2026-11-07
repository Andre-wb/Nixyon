import java.util.Properties
import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("rust")
}

val tauriProperties = Properties().apply {
    val propFile = file("tauri.properties")
    if (propFile.exists()) {
        propFile.inputStream().use { load(it) }
    }
}

// Демо-сборка подписывается отладочным ключом: свойство nixyonDebugSigned ставит
// scripts/build-demo-apk.sh. Полная демо-сборка несёт модели внутри APK: папку с
// подпапками models/ и llm/ задаёт свойство nixyonModelsAssets. Лёгкая сборка и сборки для
// разработки моделей не содержат, телефон качает их с зеркала.
val debugSigned: Boolean = providers.gradleProperty("nixyonDebugSigned").orNull == "true"
val modelsAssets: String? = providers.gradleProperty("nixyonModelsAssets").orNull

android {
    compileSdk = 37
    // Та же NDK, что у плагина и в docker/android/Dockerfile: ею же выпускная сборка
    // обрезает отладочные данные нативных библиотек.
    ndkVersion = "30.0.16248370"
    namespace = "dev.demo.nixyon"
    defaultConfig {
        manifestPlaceholders["usesCleartextTraffic"] = "false"
        applicationId = "dev.demo.nixyon"
        minSdk = 24
        targetSdk = 37
        versionCode = tauriProperties.getProperty("tauri.android.versionCode", "1").toInt()
        versionName = tauriProperties.getProperty("tauri.android.versionName", "1.0")
        // Только arm64: так меньше APK, а sherpa-onnx тянет свои .so под каждую ABI.
        ndk {
            abiFilters += listOf("arm64-v8a")
        }
    }
    buildTypes {
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
        }
        getByName("release") {
            optimization {
               enable = true
            }
            // Демо ставится поверх отладочных сборок, поэтому подписано тем же отладочным
            // ключом. Сборка для магазина, если появится, получит свой ключ.
            if (debugSigned) {
                signingConfig = signingConfigs.getByName("debug")
            }
            proguardFiles(
                *fileTree(".") {
                  include("**/*.pro")
                  exclude("build/**")
                }.files.toTypedArray()
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    buildFeatures {
        buildConfig = true
    }
    sourceSets {
        getByName("main") {
            if (modelsAssets != null) {
                assets.srcDir(modelsAssets)
            }
        }
    }
    // sherpa-onnx читает модели прямо из APK через AssetManager: сжатые файлы пришлось бы
    // распаковывать в память целиком. Языковую модель (gguf) llama.cpp читает из файла APK
    // по смещению, это тоже возможно только без сжатия.
    androidResources {
        noCompress += listOf("onnx", "txt", "json", "gguf")
    }
    // Kotlin-код плагина работает только через libsherpa-onnx-jni.so,
    // C и C++ API sherpa-onnx в APK не нужны.
    packaging {
        jniLibs {
            excludes += listOf("**/libsherpa-onnx-c-api.so", "**/libsherpa-onnx-cxx-api.so")
        }
    }
}

// Отладочные данные нативных библиотек остаются только в отладочной сборке. Блок packaging
// внутри buildTypes действует на все сборки сразу, и выпускной APK вёз их тоже.
androidComponents {
    onVariants(selector().withBuildType("debug")) { variant ->
        variant.packaging.jniLibs.keepDebugSymbols.add("**/*.so")
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

rust {
    rootDirRel = "../../../"
}

dependencies {
    implementation("androidx.webkit:webkit:1.14.0")
    implementation("androidx.appcompat:appcompat:1.7.1")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("com.google.android.material:material:1.12.0")
    implementation("androidx.lifecycle:lifecycle-process:2.10.0")
    // sherpa-onnx: Kotlin API и .so (одна копия libonnxruntime.so). Скачивает scripts/fetch-sherpa.sh.
    implementation(files("../../../../../third_party/sherpa-onnx/sherpa-onnx-2.0.1.aar"))
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.1.4")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.0")
}

apply(from = file("tauri.build.gradle.kts"))
