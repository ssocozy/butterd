cmake_minimum_required(VERSION 3.22)
project(butterd LANGUAGES CXX)
add_library(butterd_compat SHARED
    native/compatibility/module.cpp
    native/compatibility/pif_config.cpp
    native/compatibility/property_virtualizer.cpp)
target_compile_features(butterd_compat PRIVATE cxx_std_20)
target_compile_options(butterd_compat PRIVATE -Wall -Wextra -fvisibility=hidden -fvisibility-inlines-hidden)
target_link_options(butterd_compat PRIVATE -Wl,--exclude-libs,ALL -Wl,-z,max-page-size=16384)
target_link_libraries(butterd_compat PRIVATE log)
