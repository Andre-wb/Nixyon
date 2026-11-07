# Библиотека языковой модели для протокола встречи: своя прослойка JNI поверх llama.cpp.
# Исходники llama.cpp кладёт scripts/fetch-llama-cpp.sh, путь к ним передаёт Gradle.
cmake_minimum_required(VERSION 3.25)
project(nixyon_llm C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT EXISTS "${NIXYON_LLAMA_SRC}/CMakeLists.txt")
    message(FATAL_ERROR "нет исходников llama.cpp в ${NIXYON_LLAMA_SRC}, запустите scripts/fetch-llama-cpp.sh")
endif()

# Видеокарта (Vulkan) для языковой модели: только с -DNIXYON_LLM_VULKAN=ON (свойство Gradle
# nixyonLlmVulkan), в обычной сборке её нет. Заголовки Vulkan с vulkan.hpp и SPIRV-Headers
# кладёт scripts/fetch-llama-cpp.sh --vulkan, компилятор шейдеров glslc берётся из NDK.
# ggml-vulkan ещё собирает генератор шейдеров для самого компьютера сборки: ему нужен
# компилятор C++ этого компьютера в PATH (cl, g++ или clang++).
option(NIXYON_LLM_VULKAN "llama.cpp с видеокартой через Vulkan" OFF)
if(NIXYON_LLM_VULKAN)
    foreach(headers NIXYON_VULKAN_HEADERS NIXYON_SPIRV_HEADERS)
        if(NOT EXISTS "${${headers}}/include")
            message(FATAL_ERROR "нет ${headers} (${${headers}}), запустите scripts/fetch-llama-cpp.sh --vulkan")
        endif()
    endforeach()
    set(GGML_VULKAN ON CACHE BOOL "" FORCE)
    # В NDK есть vulkan.h, но нет vulkan.hpp: заголовки берутся из Vulkan-Headers.
    set(Vulkan_INCLUDE_DIR "${NIXYON_VULKAN_HEADERS}/include" CACHE PATH "" FORCE)
    # ggml-vulkan ищет SPIRV-Headers пакетом CMake. Пакет из одних заголовков: хватает
    # своего описания с путём к ним, без установки SPIRV-Headers.
    set(spirv_config "${CMAKE_CURRENT_BINARY_DIR}/spirv-headers")
    file(WRITE "${spirv_config}/SPIRV-HeadersConfig.cmake"
        "if(NOT TARGET SPIRV-Headers::SPIRV-Headers)\n"
        "    add_library(SPIRV-Headers::SPIRV-Headers INTERFACE IMPORTED)\n"
        "    set_target_properties(SPIRV-Headers::SPIRV-Headers PROPERTIES\n"
        "        INTERFACE_INCLUDE_DIRECTORIES \"${NIXYON_SPIRV_HEADERS}/include\")\n"
        "endif()\n")
    set(SPIRV-Headers_DIR "${spirv_config}" CACHE PATH "" FORCE)
    include_directories(SYSTEM "${NIXYON_SPIRV_HEADERS}/include")
    # glslc лежит в NDK (shader-tools/<компьютер сборки>/), другой можно передать
    # -DVulkan_GLSLC_EXECUTABLE.
    if(NOT Vulkan_GLSLC_EXECUTABLE)
        foreach(ndk IN ITEMS "${CMAKE_ANDROID_NDK}" "${ANDROID_NDK}")
            if(ndk)
                file(GLOB glslc_found "${ndk}/shader-tools/*/glslc" "${ndk}/shader-tools/*/glslc.exe")
                if(glslc_found)
                    list(GET glslc_found 0 glslc_found)
                    set(Vulkan_GLSLC_EXECUTABLE "${glslc_found}" CACHE FILEPATH "" FORCE)
                    break()
                endif()
            endif()
        endforeach()
    endif()
    if(NOT Vulkan_GLSLC_EXECUTABLE)
        message(FATAL_ERROR "нет glslc: в NDK (${CMAKE_ANDROID_NDK}${ANDROID_NDK}/shader-tools) его не нашлось, передайте -DVulkan_GLSLC_EXECUTABLE")
    endif()
endif()

# SYSTEM: предупреждения из заголовков llama.cpp не валят сборку прослойки с -Werror.
add_subdirectory(${NIXYON_LLAMA_SRC} build-llama SYSTEM)

add_library(nixyon_llm SHARED meeting_llm.cpp llm_devices.cpp)
target_compile_options(nixyon_llm PRIVATE -Wall -Wextra -Werror)
target_link_libraries(nixyon_llm llama llama-common android log)
