#include <libunwind.h>
#include <backtrace.h>
#include <backtrace-supported.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <cxxabi.h>  // para demangling

struct backtrace_state *bt_state = NULL;

void error_callback(void *data, const char *msg, int errnum) {
    fprintf(stderr, "libbacktrace error: %s (%d)\n", msg, errnum);
}

typedef struct {
    uintptr_t pc;
    int have_pcinfo;
    const char *filename;
    int lineno;
    char funcbuf[512];
} frame_data_t;

// Callback para backtrace_pcinfo - guarda arquivo e linha no frame_data
int full_callback(void *data, uintptr_t pc, const char *filename, int lineno, const char *function) {
    frame_data_t *frame = (frame_data_t *)data;
    frame->have_pcinfo = 1;
    frame->filename = filename;
    frame->lineno = lineno;

    // Não usamos 'function' daqui porque já vamos demanglar no syminfo_callback
    return 0;
}

// Callback para backtrace_syminfo - demangla o nome e guarda em frame_data
void syminfo_callback(void *data, uintptr_t pc, const char *symname, uintptr_t symval, uintptr_t symsize) {
    frame_data_t *frame = (frame_data_t *)data;
    if (symname) {
        int status;
        char *demangled = abi::__cxa_demangle(symname, NULL, NULL, &status);
        if (status == 0 && demangled) {
            snprintf(frame->funcbuf, sizeof(frame->funcbuf), "%s", demangled);
            free(demangled);
        } else {
            snprintf(frame->funcbuf, sizeof(frame->funcbuf), "%s", symname);
        }
    } else {
        snprintf(frame->funcbuf, sizeof(frame->funcbuf), "???");
    }
}

// Caso de erro do backtrace
void bt_error(void *data, const char *msg, int errnum) {
    fprintf(stderr, "libbacktrace error: %s (%d)\n", msg, errnum);
}

void segfault_handler(int sig, siginfo_t *si, void *context) {
    ucontext_t *uc = (ucontext_t *)context;
    unw_cursor_t cursor;

    if (unw_init_local2(&cursor, uc, UNW_INIT_SIGNAL_FRAME) < 0) {
        fprintf(stderr, "unw_init_local2 failed\n");
        _exit(11);
    }

    fprintf(stderr, "Segfault! Address: %p\nBacktrace:\n", si->si_addr);
    while (unw_step(&cursor) > 0) {
        unw_word_t pc;
        unw_get_reg(&cursor, UNW_REG_IP, &pc);
        if (pc) pc--;

        frame_data_t frame = {0};
        frame.pc = pc;
        frame.have_pcinfo = 0;
        frame.filename = NULL;
        frame.lineno = 0;
        frame.funcbuf[0] = '\0';

        backtrace_syminfo(bt_state, pc, syminfo_callback, bt_error, &frame);
        backtrace_pcinfo(bt_state, pc, full_callback, bt_error, &frame);

        if (frame.have_pcinfo && frame.filename) {
            fprintf(stderr, "  %s at %s:%d [0x%lx]\n", frame.funcbuf, frame.filename, frame.lineno, pc);
        } else {
            fprintf(stderr, "  %s [0x%lx]\n", frame.funcbuf, pc);
        }
    }

    _exit(11);
}

void setup_segfault_handler() {
    bt_state = backtrace_create_state(NULL, 1, bt_error, NULL);

    struct sigaction sa = {};
    sa.sa_sigaction = segfault_handler;
    sa.sa_flags = SA_SIGINFO;
    sigemptyset(&sa.sa_mask);
    sigaction(SIGSEGV, &sa, NULL);
}
