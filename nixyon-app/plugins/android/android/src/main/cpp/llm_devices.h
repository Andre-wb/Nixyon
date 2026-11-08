// Где считает языковая модель: процессор или видеокарта через Vulkan.
//
// Видеокарта есть только в сборке с ggml-vulkan (свойство Gradle nixyonLlmVulkan):
// библиотека libggml-vulkan.so грузится по имени при первой просьбе о видеокарте, один раз
// за процесс. Нет библиотеки, нет Vulkan на телефоне или он не нашёл видеокарту: модель
// считает на процессоре. Код без JNI и логов Android, чтобы его можно было проверить на
// ноутбуке с программным Vulkan.

#pragma once

#include <string>

#include "ggml-backend.h"
#include "llama.h"

namespace nixyon_llm {

// Первая видеокарта (GPU или встроенная IGPU) из библиотеки library, или nullptr.
// Повторные вызовы возвращают то, что нашёл первый, с любым library.
ggml_backend_dev_t vulkan_device(const char *library);

// Список устройств для llama_model_params::devices. Живёт, пока грузится модель.
struct DeviceList {
    ggml_backend_dev_t items[2] = {nullptr, nullptr};
};

// Параметры модели под устройство. gpu != nullptr: все слои на видеокарте, перекладка
// весов для процессора не нужна. gpu == nullptr: список устройств пустой, чтобы llama.cpp
// не взял видеокарту сам, если её библиотека уже загружена; repack как просили.
void use_device(llama_model_params &params, DeviceList &list, ggml_backend_dev_t gpu, bool repack);

// Имя для лога и интерфейса: «Vulkan: <описание видеокарты>» или «CPU».
std::string device_name(ggml_backend_dev_t gpu);

}  // namespace nixyon_llm
