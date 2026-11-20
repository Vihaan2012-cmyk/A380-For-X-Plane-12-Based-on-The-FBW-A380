# C++ cockpit displays

FlyByWire's A380 cockpit screens (PFD, ND, EWD, SD, MFD, OIT, EFB, ISIS, FCU, RMP and the small displays) drawn in C++ with Skia, as an alternative to running the pages in Chromium. Each display reads the same variables and events as FlyByWire's page and draws what that page draws.

This is still in development and **off by default**. The plugin uses it when it is built with the `native-displays` feature and `"nativeScreens": "all"` is set in `X-Plane 12/Output/preferences/xphfbw.json`. Several screens are still behind the Chromium ones.

## Layout

- `core/`: inputs, the display registry, fonts, text, the GPU context and the C API the plugin loads (`core/include/a380/c_api.h`).
- `displays/<id>/`: one folder per display.
- `tools/render.cpp`: renders one display from a saved input state to a PNG, or times it (`--bench`, `--bench-all`).

## Building

CMake, Ninja and a C++20 compiler, against Skia M152 built with Ganesh GL (`tools/skia/args.gn`). The plugin's build (`build.rs`, feature `native-displays`) builds `a380_displays.dll` from here and loads it at run time; `A380_DISPLAYS_DIR` points it at this folder.

```
cmake -S a380-displays -B <build dir> -G Ninja -DCMAKE_BUILD_TYPE=Release -DA380_GPU=ON -DA380_SKIA_ROOT=<skia> -DA380_SKIA_LIB_DIR=<skia>/out/<config>
cmake --build <build dir>
```

## Licence

GPL-3.0 like the rest of this repository; the drawing follows FlyByWire's A380X instrument code. `third_party/nlohmann` is MIT.
