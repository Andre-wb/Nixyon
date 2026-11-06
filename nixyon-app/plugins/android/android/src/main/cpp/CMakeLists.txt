# Библиотека языковой модели для протокола встречи: своя прослойка JNI поверх llama.cpp.
# Исходники llama.cpp кладёт scripts/fetch-llama-cpp.sh, путь к ним передаёт Gradle.
cmake_minimum_required(VERSION 3.25)
project(nixyon_llm C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

if(NOT EXISTS "${NIXYON_LLAMA_SRC}/CMakeLists.txt")
    message(FATAL_ERROR "нет исходников llama.cpp в ${NIXYON_LLAMA_SRC}, запустите scripts/fetch-llama-cpp.sh")
endif()

# SYSTEM: предупреждения из заголовков llama.cpp не валят сборку прослойки с -Werror.
add_subdirectory(${NIXYON_LLAMA_SRC} build-llama SYSTEM)

add_library(nixyon_llm SHARED meeting_llm.cpp)
target_compile_options(nixyon_llm PRIVATE -Wall -Wextra -Werror)
target_link_libraries(nixyon_llm llama llama-common android log)
