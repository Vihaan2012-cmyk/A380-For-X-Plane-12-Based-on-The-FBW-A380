#ifndef A380_C_API_H
#define A380_C_API_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct a380d_display a380d_display;
typedef struct a380d_inputs a380d_inputs;

/* All a380d_* calls that touch a display (create, draw, destroy) must come from the thread
   that called a380d_gpu_init. Functions returning int return 0 on success and a negative
   code on failure (details in a380d_last_error()), unless stated otherwise. No call throws. */

#define A380D_ABI_VERSION 3

/* Version of this interface; a host compares it with the A380D_ABI_VERSION it was built against
   before calling anything else. Bumped on any incompatible change. */
int a380d_abi_version(void);

/* Where the displays read their files. kind is one of
     "html_ui"  the aircraft's html_ui folder; sets "fonts" (<p>/Fonts), "images" (<p>/Images/fbw-a380x)
                and "ife" (<p>/Pages/VCockpit/Instruments/A380X/IFE) in one go
     "fonts"    folder searched recursively for .ttf/.otf
     "images"   folder holding the PNGs (SD_*.png, fcu/, Common/, TRIM_INDICATOR.png)
     "ife"      the IFE page folder (land.js, theme/theme.json)
     "amdb"     folder with <ICAO>.json airport files
   An empty or NULL path clears the setting. Settings take priority over the offline tools'
   environment variables (A380_FONT_DIR, A380_IMAGE_DIR, A380_IFE_DIR, A380_AMDB_DIR). Call it
   before the first a380d_create; fonts are indexed on first use. Returns 0, or -2 for an unknown
   kind or NULL kind. Any thread. */
int a380d_set_asset_dir(const char* kind, const char* path);

/* Creates the render context. Returns 1 when drawing runs on the GPU, 0 when only the CPU
   rasteriser is available (still usable), negative on failure. Idempotent. */
int a380d_gpu_init(void);

/* Text of the last failure on the calling thread, "" when none. Valid until the next call. */
const char* a380d_last_error(void);

/* id: "nd" "mfd" "mfdx" "pfd" "ewd" "sd" "sdv2" ...; variant: "1", "2" or NULL/"". NULL on failure. */
a380d_display* a380d_create(const char* id, const char* variant);
void a380d_destroy(a380d_display* display);

/* CSS size of the display; the pixel size is round(css * dpr) with the dpr of the inputs drawn. */
void a380d_size(const a380d_display* display, int* css_w, int* css_h);

a380d_inputs* a380d_inputs_new(void);
void a380d_inputs_free(a380d_inputs* inputs);

/* A simulator or local variable, e.g. ("L:A32NX_EFIS_L_ND_MODE", "number", 1.0). unit may be NULL. */
int a380d_set_var(a380d_inputs* inputs, const char* name, const char* unit, double value);
int a380d_set_string(a380d_inputs* inputs, const char* name, const char* utf8);
/* Latest event on a shared-bus topic; json is JSON text. */
int a380d_set_bus(a380d_inputs* inputs, const char* topic, const char* json);
/* Same, for a topic whose payload is a plain string (for example "a380.mfd.dom" holding the
   HTML snapshot); no JSON parsing or escaping, len bytes. */
int a380d_set_bus_string(a380d_inputs* inputs, const char* topic, const char* utf8, size_t len);
/* Latest generic-data payload under key; JSON text, or any text (kept as a string when it does not parse). */
int a380d_set_generic(a380d_inputs* inputs, const char* key, const char* json);
int a380d_set_fetched(a380d_inputs* inputs, const char* url, const char* body, size_t len);
/* Number from the recording/environment record by dotted path ("connectedAt.perf",
   "frozenAt.perf", "fmaChangedAt", "fmaPrev.<var>"). */
int a380d_set_env_number(a380d_inputs* inputs, const char* path, double value);
/* Simulation time in milliseconds. */
int a380d_set_now(a380d_inputs* inputs, double ms);
int a380d_set_dpr(a380d_inputs* inputs, double dpr);

/* Renders the display into rgba (premultiplied RGBA8888, row stride in bytes, opaque black
   background; A380_PAGE_TRANSPARENT=1 in the environment gives a transparent one). w and h must equal round(css size * dpr of the inputs). The display's GPU
   surface is kept and reused across calls. Returns 1 when the picture differs from the
   previous draw of this display (or it is the first), 0 when identical, negative on failure. */
int a380d_draw(a380d_display* display, const a380d_inputs* inputs, uint8_t* rgba, int w, int h, int stride);

/* Draws several displays onto one surface of the size every one of them needs. The first layer
   is drawn on the page background, each later layer over the result without clearing, each with its
   own inputs (all with the same dpr). The result is read back as in a380d_draw. The surface and
   the change detection belong to displays[0]. Returns 1 when the picture differs from the
   previous layered draw of displays[0], 0 when identical, negative on failure. */
int a380d_draw_layers(a380d_display* const* displays, const a380d_inputs* const* inputs, int n, uint8_t* rgba, int w, int h, int stride);

/* Picture drawn scaled to the whole surface, with linear filtering, before the display
   (terrain or weather under the ND). rgba is straight (not premultiplied) RGBA8888, stride in
   bytes; the pixels are copied. NULL rgba removes the underlay. Changes what the next draw
   of this display shows. */
int a380d_set_underlay(a380d_display* display, const uint8_t* rgba, int w, int h, int stride);

/* ABI 2: input. Pointer and key calls may come from any thread; each display serialises its own
   input state with its draws. Coordinates are CSS pixels of the display, origin top-left.
   kind: 0 down, 1 drag (moved while pressed), 2 up, 3 move, 4 wheel (button carries the clicks).
   Returns 1 when an element of the page took the event, 0 when nothing there takes it, negative
   on failure. What the page writes in answer is queued; collect it with a380d_take_writes. */
#define A380D_POINTER_DOWN 0
#define A380D_POINTER_DRAG 1
#define A380D_POINTER_UP 2
#define A380D_POINTER_MOVE 3
#define A380D_POINTER_WHEEL 4
int a380d_pointer(a380d_display* display, double x, double y, int kind, int button);
/* key: character code (or virtual key when no character), mods: host modifier flags. */
int a380d_key(a380d_display* display, int key, int mods, int down);

/* One write the page makes: kind 'L' (local variable), 'K' (key event), 'H' (HTML event), with the
   name as written ("L:A32NX_BTN_UP", "H:A32NX_SD_STS_NEXT_PAGE") and the value; delay_ms > 0 means
   the page writes it that long after the event (momentary buttons release this way). */
typedef struct a380d_write {
    char kind;
    char name[127];
    double value;
    double delay_ms;
} a380d_write;

/* Moves up to cap queued writes, oldest first, into out; returns how many. Any thread. */
int a380d_take_writes(a380d_display* display, a380d_write* out, int cap);

/* 1 when every click and key of this display is handled here (a host forwards nothing to the
   page), 0 while part of the page's input logic still runs in the page. */
int a380d_input_native(const a380d_display* display);

/* ABI 3: the laid-out border box of every element of the document drawn last, as x, y, w, h in CSS
   pixels, in document order: body first, text, svg shapes and children of replaced elements
   (svg, canvas, img, input, textarea, select) left out, before scroll offsets. Writes up to cap
   boxes (4 floats each) and returns how many the document has (may exceed cap); 0 when the display
   has no document. Owner thread, after a380d_draw. */
int a380d_boxes(const a380d_display* display, float* xywh, int cap);

/* Main thread, with the host's GL context current: makes n GL contexts sharing its objects,
   which the next n render threads take in a380d_gpu_init. Returns how many were made. */
int a380d_gpu_share(int n);

/* 1 when this thread's GPU context shares objects with the host's. */
int a380d_gpu_shared(void);

/* a380d_draw_layers into a GL texture the host's context can read (one of three per display,
   in turn) instead of read-back pixels. *texture is its GL name, *fence a GLsync the reader
   waits on before reading. */
int a380d_draw_layers_gpu(a380d_display* const* displays, const a380d_inputs* const* inputs, int n, int w, int h, unsigned* texture, void** fence);

/* Makes this thread's GL context wait for fence on the GPU, then deletes it. */
int a380d_gpu_wait(void* fence);

/* Draws the layers on the GL context current on this thread (the host's, in its draw
   callback) into a w x h RGBA8 texture with mips that the first display owns, rebuilds its
   mips, resets the GL bindings Skia used and stores the texture's name in *texture: no
   thread, copy or fence. Returns 1. */
/* Copies (and clears) the one-off checks a380d_draw_layers_host made of its screen textures
   into out (NUL-terminated); returns the length, 0 when there is nothing. */
int a380d_host_diag(char* out, int cap);

int a380d_draw_layers_host(a380d_display* const* displays, const a380d_inputs* const* inputs, int n, int w, int h, unsigned* texture);

#ifdef __cplusplus
}
#endif

#endif
