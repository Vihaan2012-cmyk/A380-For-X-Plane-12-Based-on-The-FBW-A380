// Bench-only sampling profiler (A380_SAMPLE=N): nothing allocates while the sampled thread is suspended.
#include "sampler.h"

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <dbghelp.h>
#include <timeapi.h>
#pragma comment(lib, "dbghelp.lib")
#pragma comment(lib, "winmm.lib")

#include <algorithm>
#include <atomic>
#include <cstdio>
#include <map>
#include <mutex>
#include <set>
#include <string>
#include <thread>
#include <vector>

namespace {

std::atomic<bool> g_run{false};
std::thread g_thread;
HANDLE g_target = nullptr;
std::mutex g_mutex;
std::vector<std::vector<DWORD64>> g_stacks;

void sample_loop() {
    timeBeginPeriod(1);
    while (g_run.load()) {
        DWORD64 pcs[48];
        int n = 0;
        if (SuspendThread(g_target) != (DWORD)-1) {
            CONTEXT ctx{};
            ctx.ContextFlags = CONTEXT_FULL;
            if (GetThreadContext(g_target, &ctx)) {
                for (int depth = 0; depth < 48 && ctx.Rip; depth++) {
                    pcs[n++] = ctx.Rip;
                    DWORD64 base = 0;
                    PRUNTIME_FUNCTION fn = RtlLookupFunctionEntry(ctx.Rip, &base, nullptr);
                    if (!fn) {
                        ctx.Rip = *(DWORD64*)ctx.Rsp;
                        ctx.Rsp += 8;
                        continue;
                    }
                    void* handler = nullptr;
                    DWORD64 frame = 0;
                    RtlVirtualUnwind(UNW_FLAG_NHANDLER, base, ctx.Rip, fn, &ctx, &handler, &frame, nullptr);
                }
            }
            ResumeThread(g_target);
            std::lock_guard<std::mutex> lock(g_mutex);
            g_stacks.emplace_back(pcs, pcs + n);
        }
        Sleep(1);
    }
    timeEndPeriod(1);
}

}  // namespace

void sampler_start() {
    DuplicateHandle(GetCurrentProcess(), GetCurrentThread(), GetCurrentProcess(), &g_target, 0, FALSE, DUPLICATE_SAME_ACCESS);
    g_run = true;
    g_thread = std::thread(sample_loop);
}

void sampler_report(FILE* out, int top) {
    g_run = false;
    if (g_thread.joinable()) g_thread.join();
    HANDLE proc = GetCurrentProcess();
    SymSetOptions(SYMOPT_UNDNAME | SYMOPT_DEFERRED_LOADS);
    SymInitialize(proc, nullptr, TRUE);
    std::map<DWORD64, std::string> names;
    auto name_of = [&](DWORD64 pc) -> const std::string& {
        auto it = names.find(pc);
        if (it != names.end()) return it->second;
        char buf[sizeof(SYMBOL_INFO) + 512] = {};
        auto* sym = (SYMBOL_INFO*)buf;
        sym->SizeOfStruct = sizeof(SYMBOL_INFO);
        sym->MaxNameLen = 511;
        DWORD64 disp = 0;
        std::string n;
        if (SymFromAddr(proc, pc, &disp, sym)) n = sym->Name;
        else {
            HMODULE mod = nullptr;
            char path[MAX_PATH] = "?";
            if (GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, (LPCSTR)pc, &mod))
                GetModuleFileNameA(mod, path, MAX_PATH);
            n = std::string("[") + (strrchr(path, '\\') ? strrchr(path, '\\') + 1 : path) + "]";
        }
        return names.emplace(pc, n).first->second;
    };
    std::map<std::string, int> self, incl;
    for (auto& s : g_stacks) {
        if (s.empty()) continue;
        self[name_of(s[0])]++;
        std::set<std::string> seen;
        for (DWORD64 pc : s) seen.insert(name_of(pc));
        for (auto& n : seen) incl[n]++;
    }
    auto print = [&](const char* title, std::map<std::string, int>& m) {
        std::vector<std::pair<int, std::string>> v;
        for (auto& [k, c] : m) v.emplace_back(c, k);
        std::sort(v.rbegin(), v.rend());
        std::fprintf(out, "%s (of %zu samples)\n", title, g_stacks.size());
        for (int i = 0; i < top && i < (int)v.size(); i++)
            std::fprintf(out, "%6.1f%%  %s\n", 100.0 * v[i].first / std::max<size_t>(1, g_stacks.size()), v[i].second.c_str());
    };
    print("self", self);
    print("inclusive", incl);
    if (const char* focus = std::getenv("A380_SAMPLE_FOCUS")) {
        std::map<std::string, int> chains;
        for (auto& s : g_stacks) {
            size_t at = 0;
            while (at < s.size() && name_of(s[at]).find(focus) == std::string::npos) at++;
            if (at == s.size()) continue;
            std::string c;
            for (size_t i = at; i < s.size() && i < at + 14; i++) c += (i > at ? " < " : "") + name_of(s[i]);
            chains[c]++;
        }
        print("callers of the focus", chains);
    }
    SymCleanup(proc);
}

#else

void sampler_start() {}
void sampler_report(FILE*, int) {}

#endif
