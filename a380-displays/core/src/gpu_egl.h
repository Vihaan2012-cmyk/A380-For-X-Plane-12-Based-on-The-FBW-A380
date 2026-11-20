#pragma once

#include "include/core/SkRefCnt.h"
#include "include/gpu/ganesh/gl/GrGLInterface.h"

namespace a380 {

sk_sp<const GrGLInterface> egl_make_interface();

}  // namespace a380
