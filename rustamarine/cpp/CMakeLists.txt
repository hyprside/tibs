cmake_minimum_required(VERSION 3.16)
project(rustamarine-cpp LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 23)

# Include dirs (podem vir do Rust depois via ENV vars se necessário)

add_library(rustamarine-cpp STATIC
    rustamarine.cpp
    headers/rustamarine/internal/rustamarine.hpp
    screen.cpp
    headers/rustamarine/internal/screen.hpp
    opengl.cpp
    renderbuffer.cpp
    egl_sync.cpp
    segfault-handler.cpp
    headers/rustamarine/internal/opengl.hpp
    headers/rustamarine/internal/utils.hpp
    headers/rustamarine.h
    vendor/glad/src/glad_egl.c
    vendor/glad/src/glad.c
)

# Se precisares de pkg-config, podes usar isso
find_package(PkgConfig REQUIRED)
pkg_check_modules(AQUAMARINE REQUIRED aquamarine)
pkg_check_modules(HYPRUTILS REQUIRED hyprutils)
pkg_check_modules(LIBDRM REQUIRED libdrm)
pkg_check_modules(LIBGBM REQUIRED gbm)
pkg_check_modules(LIBUNWIND REQUIRED libunwind)

target_compile_options(rustamarine-cpp PUBLIC
  ${AQUAMARINE_CFLAGS}
  ${HYPRUTILS_CFLAGS}
  ${LIBDRM_CFLAGS}
  ${LIBUNWIND_CFLAGS}
)

target_include_directories(rustamarine-cpp PUBLIC
  headers
  vendor/glad/include
)

target_link_libraries(rustamarine-cpp
	PUBLIC
  ${AQUAMARINE_LIBRARIES}
  ${HYPRUTILS_LIBRARIES}
  ${LIBDRM_LIBRARIES}
  ${LIBGBM_LIBRARIES}
  ${LIBUNWIND_LIBRARIES}
  EGL GLESv2
)


# Instalar biblioteca
install(TARGETS rustamarine-cpp
    ARCHIVE DESTINATION lib
    LIBRARY DESTINATION lib
    RUNTIME DESTINATION bin
)
