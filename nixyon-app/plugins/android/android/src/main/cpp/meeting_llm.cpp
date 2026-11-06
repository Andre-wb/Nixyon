// Языковая модель для протокола встречи: загрузка GGUF, ответ на один запрос из
// системной инструкции и текста встречи, отмена, выгрузка.
//
// Граница JNI (MeetingLlmNative.kt):
// - nativeOpen получает открытый дескриптор файла и смещение модели в нём: модель лежит
//   отдельным файлом (смещение 0) или несжатой внутри APK. Дескриптор копируется через
//   dup, вызывающий может закрыть свой сразу после вызова.
// - nativeOpen возвращает указатель на Session как long; 0 не возвращается никогда,
//   ошибка приходит исключением LlmException. Владелец указателя Kotlin: ровно один
//   вызов nativeClose освобождает его, после этого указатель не используется.
// - nativeGenerate и nativeClose для одной сессии зовутся из одного потока и не
//   пересекаются; nativeCancel можно звать из любого потока в любой момент до
//   nativeClose: он только выставляет атомарный флаг.
// - Текст встречи в лог не пишется: в лог Android уходят только ошибки llama.cpp.

#include <android/log.h>
#include <dlfcn.h>
#include <jni.h>
#include <unistd.h>

#include <cstdio>

#include <algorithm>
#include <atomic>
#include <memory>
#include <mutex>
#include <string>
#include <vector>

#include "chat.h"
#include "common.h"
#include "ggml-backend.h"
#include "llama.h"

namespace {

constexpr const char *TAG = "nixyon-llm";
constexpr const char *EXCEPTION_CLASS = "dev/demo/nixyon/speech/LlmException";
constexpr const char *PROGRESS_CLASS = "dev/demo/nixyon/speech/LlmProgress";

// Коды совпадают с LlmException.kt.
constexpr const char *CODE_LOAD_FAILED = "llm_load_failed";
constexpr const char *CODE_TOO_LONG = "llm_too_long";
constexpr const char *CODE_CANCELLED = "llm_cancelled";
constexpr const char *CODE_FAILED = "llm_failed";

constexpr int PHASE_READING = 0;
constexpr int PHASE_WRITING = 1;
// Прогресс ответа шлётся не на каждый токен, чтобы не забивать канал в интерфейс.
constexpr int WRITING_PROGRESS_STEP = 8;

void log_errors_only(ggml_log_level level, const char *text, void * /*unused*/) {
    if (level == GGML_LOG_LEVEL_ERROR) {
        __android_log_write(ANDROID_LOG_ERROR, TAG, text);
    }
}

std::once_flag backend_once;

// Варианты процессорной части llama.cpp, собранные с GGML_CPU_ALL_VARIANTS под Android
// (ggml/src/CMakeLists.txt). Библиотеки лежат внутри APK несжатыми, папки с ними на диске
// нет, поэтому ggml_backend_load_all_from_path их не находит. Загрузчик Android находит
// их по имени, а выбор тот же, что у llama.cpp: наибольшая оценка ggml_backend_score.
constexpr const char *CPU_VARIANTS[] = {
    "libggml-cpu-android_armv8.0_1.so", "libggml-cpu-android_armv8.2_1.so", "libggml-cpu-android_armv8.2_2.so",
    "libggml-cpu-android_armv8.6_1.so", "libggml-cpu-android_armv9.0_1.so", "libggml-cpu-android_armv9.2_1.so",
    "libggml-cpu-android_armv9.2_2.so",
};

bool load_best_cpu_backend() {
    using score_fn = int (*)();
    const char *best = nullptr;
    int best_score = 0;
    for (const char *name : CPU_VARIANTS) {
        void *handle = dlopen(name, RTLD_NOW | RTLD_LOCAL);
        if (handle == nullptr) {
            continue;
        }
        auto score = reinterpret_cast<score_fn>(dlsym(handle, "ggml_backend_score"));
        const int value = score == nullptr ? 0 : score();
        dlclose(handle);
        if (value > best_score) {
            best_score = value;
            best = name;
        }
    }
    return best != nullptr && ggml_backend_load(best) != nullptr;
}

struct FileCloser {
    void operator()(FILE *file) const { std::fclose(file); }
};
struct ModelDeleter {
    void operator()(llama_model *model) const { llama_model_free(model); }
};
struct ContextDeleter {
    void operator()(llama_context *context) const { llama_free(context); }
};
struct SamplerDeleter {
    void operator()(llama_sampler *sampler) const { llama_sampler_free(sampler); }
};

// Файл объявлен первым: он закрывается последним, после модели, которая читает из него.
struct Session {
    std::unique_ptr<FILE, FileCloser> file;
    std::unique_ptr<llama_model, ModelDeleter> model;
    std::unique_ptr<llama_context, ContextDeleter> context;
    common_chat_templates_ptr templates;
    std::atomic<bool> cancel{false};
};

class GenerationError {
  public:
    GenerationError(const char *code, std::string message) : code_(code), message_(std::move(message)) {}
    [[nodiscard]] const char *code() const { return code_; }
    [[nodiscard]] const std::string &message() const { return message_; }

  private:
    const char *code_;
    std::string message_;
};

void throw_java(JNIEnv *env, const char *code, const std::string &message) {
    jclass cls = env->FindClass(EXCEPTION_CLASS);
    if (cls == nullptr) {
        return;
    }
    jmethodID ctor = env->GetMethodID(cls, "<init>", "(Ljava/lang/String;Ljava/lang/String;)V");
    if (ctor == nullptr) {
        return;
    }
    jstring jcode = env->NewStringUTF(code);
    jstring jmessage = env->NewStringUTF(message.c_str());
    auto exception = static_cast<jthrowable>(env->NewObject(cls, ctor, jcode, jmessage));
    if (exception != nullptr) {
        env->Throw(exception);
    }
}

std::string utf8_of(JNIEnv *env, jstring text) {
    const char *chars = env->GetStringUTFChars(text, nullptr);
    if (chars == nullptr) {
        throw GenerationError(CODE_FAILED, "строка из Java не прочитана");
    }
    std::string result(chars);
    env->ReleaseStringUTFChars(text, chars);
    return result;
}

Session *session_of(jlong handle) {
    // По контракту JNI выше handle получен из nativeOpen и ещё не закрыт.
    return reinterpret_cast<Session *>(handle);
}

bool cancel_requested(void *data) {
    return static_cast<std::atomic<bool> *>(data)->load(std::memory_order_relaxed);
}

class Progress {
  public:
    Progress(JNIEnv *env, jobject target) : env_(env), target_(target) {
        jclass cls = env->FindClass(PROGRESS_CLASS);
        method_ = cls == nullptr ? nullptr : env->GetMethodID(cls, "onProgress", "(III)V");
        if (method_ == nullptr) {
            throw GenerationError(CODE_FAILED, "LlmProgress.onProgress не найден");
        }
    }

    void send(int phase, int done, int total) {
        env_->CallVoidMethod(target_, method_, phase, done, total);
        if (env_->ExceptionCheck()) {
            env_->ExceptionClear();
            throw GenerationError(CODE_FAILED, "прогресс не отправлен");
        }
    }

  private:
    JNIEnv *env_;
    jobject target_;
    jmethodID method_;
};

// Части шаблона разбираются со служебными токенами, части с текстом пользователя без
// них: служебная разметка внутри расшифровки остаётся простым текстом.
std::vector<llama_token> tokenize_prompt(const llama_vocab *vocab, const common_chat_prompt &prompt) {
    std::vector<llama_token> tokens;
    bool first = true;
    for (const auto &part : prompt.parts) {
        auto piece = common_tokenize(vocab, part.text, first, !part.is_input);
        tokens.insert(tokens.end(), piece.begin(), piece.end());
        first = false;
    }
    return tokens;
}

void check_decode(int32_t result, const Session &session) {
    if (result == 0) {
        return;
    }
    if (result == 2 || session.cancel.load()) {
        throw GenerationError(CODE_CANCELLED, "отменено");
    }
    throw GenerationError(CODE_FAILED, "llama_decode вернул " + std::to_string(result));
}

std::string generate(Session &session, const std::string &system, const std::string &user, int max_tokens,
                     Progress &progress) {
    const llama_vocab *vocab = llama_model_get_vocab(session.model.get());
    llama_context *context = session.context.get();

    common_chat_templates_inputs inputs;
    common_chat_msg system_msg;
    system_msg.role = "system";
    system_msg.content = system;
    common_chat_msg user_msg;
    user_msg.role = "user";
    user_msg.content = user;
    inputs.messages = {system_msg, user_msg};
    inputs.add_generation_prompt = true;
    inputs.use_jinja = true;
    inputs.enable_thinking = false;
    inputs.chat_template_kwargs["enable_thinking"] = "false";
    const auto applied = common_chat_templates_apply(session.templates.get(), inputs);

    auto tokens = tokenize_prompt(vocab, applied.prompt);
    const auto n_ctx = static_cast<int64_t>(llama_n_ctx(context));
    if (tokens.empty()) {
        throw GenerationError(CODE_FAILED, "пустой запрос");
    }
    if (static_cast<int64_t>(tokens.size()) + max_tokens > n_ctx) {
        throw GenerationError(CODE_TOO_LONG, "токенов запроса " + std::to_string(tokens.size()) + ", контекст " +
                                                 std::to_string(n_ctx));
    }

    llama_memory_clear(llama_get_memory(context), true);
    const auto total = static_cast<int>(tokens.size());
    const int batch = static_cast<int>(llama_n_batch(context));
    progress.send(PHASE_READING, 0, total);
    for (int start = 0; start < total; start += batch) {
        const int count = std::min(batch, total - start);
        check_decode(llama_decode(context, llama_batch_get_one(tokens.data() + start, count)), session);
        progress.send(PHASE_READING, start + count, total);
    }

    std::unique_ptr<llama_sampler, SamplerDeleter> sampler(llama_sampler_chain_init(llama_sampler_chain_default_params()));
    llama_sampler_chain_add(sampler.get(), llama_sampler_init_greedy());

    std::string answer;
    progress.send(PHASE_WRITING, 0, max_tokens);
    for (int produced = 0; produced < max_tokens; ++produced) {
        if (session.cancel.load()) {
            throw GenerationError(CODE_CANCELLED, "отменено");
        }
        llama_token token = llama_sampler_sample(sampler.get(), context, -1);
        if (llama_vocab_is_eog(vocab, token)) {
            progress.send(PHASE_WRITING, max_tokens, max_tokens);
            return answer;
        }
        answer += common_token_to_piece(vocab, token, false);
        check_decode(llama_decode(context, llama_batch_get_one(&token, 1)), session);
        if ((produced + 1) % WRITING_PROGRESS_STEP == 0) {
            progress.send(PHASE_WRITING, produced + 1, max_tokens);
        }
    }
    throw GenerationError(CODE_FAILED, "ответ не уложился в " + std::to_string(max_tokens) + " токенов");
}

jbyteArray to_java_bytes(JNIEnv *env, const std::string &text) {
    const auto size = static_cast<jsize>(text.size());
    jbyteArray bytes = env->NewByteArray(size);
    if (bytes != nullptr) {
        env->SetByteArrayRegion(bytes, 0, size, reinterpret_cast<const jbyte *>(text.data()));
    }
    return bytes;
}

}  // namespace

extern "C" JNIEXPORT jlong JNICALL Java_dev_demo_nixyon_speech_MeetingLlmNative_nativeOpen(
    JNIEnv *env, jobject /*unused*/, jint fd, jlong offset, jint n_ctx, jint n_threads) {
    try {
        static bool backend_ready = false;
        std::call_once(backend_once, [] {
            llama_log_set(log_errors_only, nullptr);
            backend_ready = load_best_cpu_backend();
            llama_backend_init();
        });
        if (!backend_ready) {
            throw GenerationError(CODE_LOAD_FAILED, "процессорная часть llama.cpp не загрузилась");
        }

        auto session = std::make_unique<Session>();
        const int own_fd = dup(fd);
        if (own_fd < 0) {
            throw GenerationError(CODE_LOAD_FAILED, "дескриптор модели не скопирован");
        }
        session->file.reset(fdopen(own_fd, "rb"));
        if (!session->file) {
            close(own_fd);
            throw GenerationError(CODE_LOAD_FAILED, "файл модели не открылся");
        }
        // llama.cpp читает GGUF с текущего места файла.
        if (fseeko(session->file.get(), static_cast<off_t>(offset), SEEK_SET) != 0) {
            throw GenerationError(CODE_LOAD_FAILED, "смещение модели недоступно");
        }
        llama_model_params model_params = llama_model_default_params();
        model_params.n_gpu_layers = 0;
        session->model.reset(llama_model_load_from_file_ptr(session->file.get(), model_params));
        if (!session->model) {
            throw GenerationError(CODE_LOAD_FAILED, "модель не загрузилась");
        }

        llama_context_params context_params = llama_context_default_params();
        context_params.n_ctx = static_cast<uint32_t>(n_ctx);
        context_params.n_batch = 512;
        context_params.n_ubatch = 512;
        context_params.n_threads = n_threads;
        context_params.n_threads_batch = n_threads;
        context_params.abort_callback = cancel_requested;
        context_params.abort_callback_data = &session->cancel;
        session->context.reset(llama_init_from_model(session->model.get(), context_params));
        if (!session->context) {
            throw GenerationError(CODE_LOAD_FAILED, "контекст не создан");
        }
        session->templates = common_chat_templates_init(session->model.get(), "");
        return reinterpret_cast<jlong>(session.release());
    } catch (const GenerationError &e) {
        throw_java(env, e.code(), e.message());
    } catch (const std::exception &e) {
        throw_java(env, CODE_LOAD_FAILED, e.what());
    }
    return 0;
}

extern "C" JNIEXPORT jbyteArray JNICALL Java_dev_demo_nixyon_speech_MeetingLlmNative_nativeGenerate(
    JNIEnv *env, jobject /*unused*/, jlong handle, jstring jsystem, jstring juser, jint max_tokens,
    jobject jprogress) {
    Session *session = session_of(handle);
    try {
        Progress progress(env, jprogress);
        const std::string answer =
            generate(*session, utf8_of(env, jsystem), utf8_of(env, juser), max_tokens, progress);
        return to_java_bytes(env, answer);
    } catch (const GenerationError &e) {
        throw_java(env, e.code(), e.message());
    } catch (const std::exception &e) {
        throw_java(env, CODE_FAILED, e.what());
    }
    return nullptr;
}

extern "C" JNIEXPORT void JNICALL Java_dev_demo_nixyon_speech_MeetingLlmNative_nativeCancel(
    JNIEnv * /*unused*/, jobject /*unused*/, jlong handle) {
    session_of(handle)->cancel.store(true);
}

extern "C" JNIEXPORT void JNICALL Java_dev_demo_nixyon_speech_MeetingLlmNative_nativeClose(
    JNIEnv * /*unused*/, jobject /*unused*/, jlong handle) {
    delete session_of(handle);
}
