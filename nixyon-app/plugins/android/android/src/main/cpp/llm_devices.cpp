#include "llm_devices.h"

#include <mutex>

namespace nixyon_llm {

namespace {

// Все слои модели: llama.cpp ограничивает число их настоящим количеством.
constexpr int ALL_LAYERS = 999;

std::once_flag vulkan_once;
ggml_backend_dev_t vulkan = nullptr;

bool is_gpu(ggml_backend_dev_t dev) {
    const enum ggml_backend_dev_type type = ggml_backend_dev_type(dev);
    return type == GGML_BACKEND_DEVICE_TYPE_GPU || type == GGML_BACKEND_DEVICE_TYPE_IGPU;
}

}  // namespace

ggml_backend_dev_t vulkan_device(const char *library) {
    std::call_once(vulkan_once, [library] {
        ggml_backend_reg_t reg = ggml_backend_load(library);
        if (reg == nullptr) {
            return;
        }
        for (size_t i = 0; i < ggml_backend_reg_dev_count(reg); ++i) {
            ggml_backend_dev_t dev = ggml_backend_reg_dev_get(reg, i);
            if (is_gpu(dev)) {
                vulkan = dev;
                return;
            }
        }
    });
    return vulkan;
}

void use_device(llama_model_params &params, DeviceList &list, ggml_backend_dev_t gpu, bool repack) {
    list.items[0] = gpu;
    list.items[1] = nullptr;
    params.devices = list.items;
    params.n_gpu_layers = gpu != nullptr ? ALL_LAYERS : 0;
    params.use_extra_bufts = gpu == nullptr && repack;
}

std::string device_name(ggml_backend_dev_t gpu) {
    if (gpu == nullptr) {
        return "CPU";
    }
    const char *description = ggml_backend_dev_description(gpu);
    return std::string("Vulkan: ") + (description != nullptr ? description : ggml_backend_dev_name(gpu));
}

}  // namespace nixyon_llm
