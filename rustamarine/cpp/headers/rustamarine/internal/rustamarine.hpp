#pragma once
#include "./utils.hpp"
#include <aquamarine/backend/Backend.hpp>
#include <aquamarine/output/Output.hpp>
#include <hyprutils/memory/SharedPtr.hpp>
#include <hyprutils/os/FileDescriptor.hpp>
#include <hyprutils/signal/Listener.hpp>
#include <rustamarine.h>
#include <rustamarine/internal/screen.hpp>
#include <rustamarine/internal/input-manager.h>
#include <vector>

struct RustamarineOpenGLContext {
	void *gbmDevice = nullptr, *eglDevice = nullptr, *eglDisplay = nullptr, *eglContext = nullptr;
	Hyprutils::OS::CFileDescriptor gbmFd;
};
struct Rustamarine {
	SP<Aquamarine::CBackend> backend;
	std::vector<SP<RustamarineScreen>> screens;
	struct {
		Hyprutils::Signal::CHyprSignalListener newOutputListener;
	} listeners;
	RustamarineOpenGLContext openGLContext;
	rustamarine::InputManager inputManager;
};
#undef Listener

SP<RustamarineScreen> createScreenFromOutput(SP<Rustamarine> rustamarine,
																						 SP<Aquamarine::IOutput> output);
void setup_segfault_handler();
