cmake_minimum_required(VERSION 3.20)
project(a380_displays CXX)

string(FIND "${CMAKE_BINARY_DIR}/" "${CMAKE_SOURCE_DIR}/" _inside)
if(_inside EQUAL 0)
  message(FATAL_ERROR "build tree ${CMAKE_BINARY_DIR} is inside the source tree; use E:/A380/fbw-build/a380-displays/<name>")
endif()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()

set(A380_SKIA_ROOT "${A380_SKIA_ROOT}" CACHE PATH "skia-bindings source folder containing include/")
if(NOT A380_SKIA_ROOT)
  file(GLOB _skia_candidates "$ENV{USERPROFILE}/.cargo/registry/src/*/skia-bindings-0.153.3/skia" "$ENV{HOME}/.cargo/registry/src/*/skia-bindings-0.153.3/skia")
  list(GET _skia_candidates 0 A380_SKIA_ROOT)
endif()
set(A380_SKIA_LIB_DIR_LINUX "" CACHE PATH "folder with Skia's lib*.a on Linux")
set(A380_SKIA_LIB_DIR "E:/A380/fbw-build/target/x86_64-pc-windows-msvc/release/build/skia-bindings-afe26377a320d469/out/skia" CACHE PATH "folder with skia.lib")

if(WIN32)
  set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")
  add_compile_options(/utf-8 /EHsc /permissive- /Zc:__cplusplus /bigobj)
  add_compile_definitions(_CRT_SECURE_NO_WARNINGS WIN32_LEAN_AND_MEAN NOMINMAX NDEBUG
    SK_TYPEFACE_FACTORY_DIRECTWRITE SK_FONTMGR_DIRECTWRITE_AVAILABLE SK_FONTMGR_GDI_AVAILABLE)
  set(A380_SKIA_LIBS "${A380_SKIA_LIB_DIR}/skia.lib")
  foreach(l skcms libpng zlib wuffs expat)
    if(EXISTS "${A380_SKIA_LIB_DIR}/${l}.lib")
      list(APPEND A380_SKIA_LIBS "${A380_SKIA_LIB_DIR}/${l}.lib")
    endif()
  endforeach()
  list(APPEND A380_SKIA_LIBS
    user32 gdi32 ole32 oleaut32 windowscodecs dwrite d3d11 dxgi opengl32 advapi32 shell32 usp10 fontsub)
else()
  set(A380_SKIA_LIBS)
  if(A380_SKIA_LIB_DIR_LINUX)
    file(GLOB _skia_archives "${A380_SKIA_LIB_DIR_LINUX}/lib*.a")
    list(APPEND A380_SKIA_LIBS -Wl,--start-group ${_skia_archives} -Wl,--end-group)
    if(NOT EXISTS "${A380_SKIA_LIB_DIR_LINUX}/libfreetype2.a")
      list(APPEND A380_SKIA_LIBS freetype)
    endif()
  else()
    list(APPEND A380_SKIA_LIBS skia)
  endif()
  find_package(Threads REQUIRED)
  list(APPEND A380_SKIA_LIBS Threads::Threads ${CMAKE_DL_LIBS})
  add_compile_options(-fno-rtti)
endif()

option(A380_GPU "render through Skia Ganesh GL (needs a ganesh-gl skia.lib)" OFF)
if(A380_GPU)
  add_compile_definitions(A380_GPU SK_GANESH SK_GL)
  if(NOT WIN32)
    list(APPEND A380_SKIA_LIBS EGL GLESv2)
  endif()
endif()

file(GLOB_RECURSE A380_CORE_SOURCES CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/core/src/*.cpp")
file(GLOB_RECURSE A380_DISPLAY_SOURCES CONFIGURE_DEPENDS "${CMAKE_CURRENT_SOURCE_DIR}/displays/*.cpp")
option(A380_GLYPH_BAKE_TOOL "Windows only: DirectWrite glyph baking and proof (needs Skia src/ under A380_SKIA_ROOT)" OFF)
if(A380_GLYPH_BAKE_TOOL AND WIN32)
  add_compile_definitions(A380_GLYPH_BAKE_TOOL)
else()
  list(FILTER A380_CORE_SOURCES EXCLUDE REGEX "glyphs_win[.]cpp$")
endif()
set(A380_DISPLAYS "" CACHE STRING "semicolon list of displays/ folders to build; empty builds all")
if(A380_DISPLAYS)
  set(_picked)
  foreach(d IN LISTS A380_DISPLAYS)
    foreach(s IN LISTS A380_DISPLAY_SOURCES)
      if(s MATCHES "/displays/${d}/")
        list(APPEND _picked "${s}")
      endif()
    endforeach()
  endforeach()
  set(A380_DISPLAY_SOURCES ${_picked})
endif()
if(WIN32)
  list(FILTER A380_CORE_SOURCES EXCLUDE REGEX "platform_other[.]cpp$")
else()
  list(FILTER A380_CORE_SOURCES EXCLUDE REGEX "platform_win[.]cpp$")
endif()

add_library(a380_core OBJECT ${A380_CORE_SOURCES})
add_library(a380_displays_obj OBJECT ${A380_DISPLAY_SOURCES})
foreach(t a380_core a380_displays_obj)
  target_include_directories(${t} PUBLIC
    "${CMAKE_CURRENT_SOURCE_DIR}/core/include"
    "${CMAKE_CURRENT_SOURCE_DIR}/displays"
    "${CMAKE_CURRENT_SOURCE_DIR}/third_party"
    "${A380_SKIA_ROOT}")
endforeach()

add_executable(a380disp_render tools/render.cpp tools/sampler.cpp
  $<TARGET_OBJECTS:a380_core> $<TARGET_OBJECTS:a380_displays_obj>)
target_include_directories(a380disp_render PRIVATE
  "${CMAKE_CURRENT_SOURCE_DIR}/core/include"
  "${CMAKE_CURRENT_SOURCE_DIR}/third_party"
  "${A380_SKIA_ROOT}")
target_link_libraries(a380disp_render PRIVATE ${A380_SKIA_LIBS})


add_library(a380_displays_static STATIC
  $<TARGET_OBJECTS:a380_core> $<TARGET_OBJECTS:a380_displays_obj>)
target_include_directories(a380_displays_static PUBLIC
  "${CMAKE_CURRENT_SOURCE_DIR}/core/include")
set_target_properties(a380_displays_static PROPERTIES
  OUTPUT_NAME a380_displays_static
  ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")

set(_link_lines "$<TARGET_FILE:a380_displays_static>")
foreach(l IN LISTS A380_SKIA_LIBS)
  string(APPEND _link_lines "\n${l}")
endforeach()
file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/link-libs.txt" CONTENT "${_link_lines}\n")

add_library(a380_displays SHARED
  $<TARGET_OBJECTS:a380_core> $<TARGET_OBJECTS:a380_displays_obj>)
target_include_directories(a380_displays PUBLIC
  "${CMAKE_CURRENT_SOURCE_DIR}/core/include")
target_link_libraries(a380_displays PRIVATE ${A380_SKIA_LIBS})
set_target_properties(a380_displays PROPERTIES
  RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}"
  LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}"
  ARCHIVE_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")
if(WIN32)
  target_sources(a380_displays PRIVATE "${CMAKE_CURRENT_SOURCE_DIR}/core/a380_displays.def")
elseif(APPLE)
  target_link_options(a380_displays PRIVATE -Wl,-exported_symbol,_a380d_*)
else()
  target_link_options(a380_displays PRIVATE
    "-Wl,--version-script=${CMAKE_CURRENT_SOURCE_DIR}/core/a380_displays.map" -Wl,--exclude-libs,ALL
    -static-libstdc++ -static-libgcc)
endif()

add_executable(c_api_test tools/c_api_test.cpp)
target_include_directories(c_api_test PRIVATE
  "${CMAKE_CURRENT_SOURCE_DIR}/core/include"
  "${CMAKE_CURRENT_SOURCE_DIR}/third_party"
  "${A380_SKIA_ROOT}")
if(MSVC)
  target_link_options(c_api_test PRIVATE "/WHOLEARCHIVE:$<TARGET_FILE:a380_displays_static>")
  target_link_libraries(c_api_test PRIVATE a380_displays_static ${A380_SKIA_LIBS})
else()
  target_link_libraries(c_api_test PRIVATE
    -Wl,--whole-archive a380_displays_static -Wl,--no-whole-archive ${A380_SKIA_LIBS})
endif()

add_executable(c_api_test_dll tools/c_api_test.cpp)
target_include_directories(c_api_test_dll PRIVATE
  "${CMAKE_CURRENT_SOURCE_DIR}/core/include"
  "${CMAKE_CURRENT_SOURCE_DIR}/third_party"
  "${A380_SKIA_ROOT}")
target_link_libraries(c_api_test_dll PRIVATE a380_displays ${A380_SKIA_LIBS})
