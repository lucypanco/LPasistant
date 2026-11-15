cmake_minimum_required(VERSION 3.22)
project(llama_jni CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
# 强制静态:llama.cpp 的 option(BUILD_SHARED_LIBS 默认 ON)会尊重此值
set(BUILD_SHARED_LIBS OFF)
# Android NDK 无 libomp:必须关闭 OpenMP,否则 dlopen 阶段缺 libomp.so
set(GGML_OPENMP OFF)

# LLAMA_DIR 由命令行传入(指向 llama.cpp 源码根)
add_subdirectory(${LLAMA_DIR} llama.cpp-build)

add_library(llama_jni SHARED llama_jni.cpp)
target_include_directories(llama_jni PRIVATE ${LLAMA_DIR}/include)
target_link_libraries(llama_jni PRIVATE llama log)
