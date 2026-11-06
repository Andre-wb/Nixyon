import org.jetbrains.kotlin.gradle.dsl.JvmTarget

plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "dev.demo.nixyon.speech"
    compileSdk = 37
    ndkVersion = "30.0.16248370"

    defaultConfig {
        // Как у приложения; llama.cpp нужен API 23 и выше (posix_madvise).
        minSdk = 24

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        consumerProguardFiles("consumer-rules.pro")

        ndk {
            abiFilters += listOf("arm64-v8a")
        }
        // llama.cpp из исходников, которые кладёт scripts/fetch-llama-cpp.sh. Процессорная
        // часть собирается в нескольких вариантах под разные ядра arm64, нужный выбирается
        // при запуске. KleidiAI выключен: при сборке он качает код из сети.
        externalNativeBuild {
            cmake {
                arguments += listOf(
                    "-DNIXYON_LLAMA_SRC=${projectDir}/../../../../third_party/llama.cpp/llama.cpp-b12005",
                    "-DCMAKE_BUILD_TYPE=Release",
                    // Библиотек C++ несколько, исключения ходят между ними: общая libc++.
                    "-DANDROID_STL=c++_shared",
                    "-DBUILD_SHARED_LIBS=ON",
                    "-DLLAMA_BUILD_COMMON=ON",
                    "-DLLAMA_OPENSSL=OFF",
                    "-DGGML_NATIVE=OFF",
                    "-DGGML_BACKEND_DL=ON",
                    "-DGGML_CPU_ALL_VARIANTS=ON",
                    "-DGGML_LLAMAFILE=OFF",
                    "-DGGML_OPENMP=OFF",
                    "-DGGML_CPU_KLEIDIAI=OFF",
                )
                // Без отладочных данных: выпускная сборка их и так обрезает, а в отладочной
                // они добавляли около 100 МБ к APK, который ставится на каждой проверке.
                cFlags += "-g0"
                cppFlags += "-g0"
            }
        }
    }

    externalNativeBuild {
        cmake {
            path("src/main/cpp/CMakeLists.txt")
            version = "4.1.2"
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
}

kotlin {
    compilerOptions {
        jvmTarget = JvmTarget.JVM_1_8
    }
}

dependencies {

    implementation("androidx.core:core-ktx:1.9.0")
    implementation("androidx.appcompat:appcompat:1.6.0")
    implementation("com.google.android.material:material:1.7.0")
    testImplementation("junit:junit:4.13.2")
    // В android.jar для модульных тестов org.json только заглушка, она бросает "not mocked".
    // Тесты манифеста моделей и файлов результатов разбирают JSON, им нужна настоящая библиотека.
    testImplementation("org.json:json:20250517")
    androidTestImplementation("androidx.test.ext:junit:1.1.5")
    androidTestImplementation("androidx.test.espresso:espresso-core:3.5.1")
    implementation(project(":tauri-android"))
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")
    // Библиотека Android не может включать локальный .aar, поэтому здесь он только
    // для компиляции, а в APK его кладёт модуль приложения. Файл скачивает
    // scripts/fetch-sherpa.sh с проверкой SHA-256.
    compileOnly(files("../../../../third_party/sherpa-onnx/sherpa-onnx-2.0.1.aar"))
}
