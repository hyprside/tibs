#include "glad/glad.h"
#include "rustamarine.h"
#include <algorithm>
#include <aquamarine/output/Output.hpp>
#include <cstdint>
#include <cstdio>
#include <hyprutils/math/Region.hpp>

#include <cstring>
#include <hyprutils/memory/SharedPtr.hpp>
#include <ranges>
#include <unistd.h>
#include <vector>
#include <print>
#include <algorithm>
#include <rustamarine/internal/rustamarine.hpp>
using namespace Hyprutils::Math;
// Forward declaration for freeing Rust closure
extern "C" void rmarFreeRustClosure(void *ptr);

bool screenIsInactive(struct RustamarineScreen *self) {
	return self->rustamarine->backend->hasSession() &&
				 !self->rustamarine->backend->session->active;
}

bool RustamarineScreen::test() {
	if(!updateSwapchain()) return false;
	this->output->state->setBuffer(this->output->swapchain->next(nullptr));
  this->output->swapchain->rollback();
	return this->output->test();
}

SP<RustamarineScreen> createScreenFromOutput(SP<Rustamarine> rustamarine,
                                             SP<Aquamarine::IOutput> output) {
    SP screen(new RustamarineScreen{output, rustamarine, {}});
    screen->needsFrameListener =
        output->events.needsFrame.registerListener([screen](std::any _) {
            if (screenIsInactive(&*screen))
                return;
              screen->output->scheduleFrame(Aquamarine::IOutput::AQ_SCHEDULE_NEEDS_FRAME);
        });
    screen->frameListener =
        output->events.frame.registerListener([screen](std::any _) {
            screen->isVBlank = true;
            if (screen->onRenderCFunc) {
                screen->onRenderCFunc(screen->onRenderContext, screen.get());
            }
        });
    screen->onStateListener =
        output->events.state.registerListener([screen](std::any data) {
            auto event = std::any_cast<Aquamarine::IOutput::SStateEvent>(data);
            screen->renderBuffers.clear();
            screen->output->state->setEnabled(true);
            screen->output->state->setCustomMode(Hyprutils::Memory::makeShared<Aquamarine::SOutputMode>(Aquamarine::SOutputMode{.pixelSize = event.size}));
            screen->output->state->setFormat(DRM_FORMAT_XRGB8888);

        });

    screen->output->state->setEnabled(true);
    screen->output->state->setFormat(DRM_FORMAT_XRGB8888);
    auto name = screen->output->name;
    std::vector<SP<Aquamarine::SOutputMode>> &requestedModes = output->modes;
    bool success = false;
    for (auto const& mode : requestedModes) {
        std::string modeStr = std::format("{:X0}@{:.2f}Hz", mode->pixelSize, mode->refreshRate / 1000.f);

        if (mode->modeInfo.has_value() && mode->modeInfo->type == DRM_MODE_TYPE_USERDEF) {
            output->state->setCustomMode(mode);

            if (!screen->test()) {
                printf("ERROR: Monitor %s: REJECTED custom mode %s!\n", name.c_str(), modeStr.c_str());
                continue;
            }

        } else {
            output->state->setMode(mode);

            if (!screen->test()) {
                printf("ERROR: Monitor %s: REJECTED available mode %s!\n", name.c_str(), modeStr.c_str());
                if (mode->preferred)
                    printf("ERROR: Monitor %s: REJECTED preferred mode!!!\n", name.c_str());
                continue;
            }
        }

        success = true;

        if (mode->preferred)
            printf("LOG: Monitor %s: using preferred mode %s\n", name.c_str(), modeStr.c_str());
        else if (mode->modeInfo.has_value() && mode->modeInfo->type == DRM_MODE_TYPE_USERDEF)
            printf("LOG: Monitor %s: using custom mode %s\n", name.c_str(), modeStr.c_str());
        else
            printf("LOG: Monitor %s: using available mode %s\n", name.c_str(), modeStr.c_str());

        break;
    }

    // try requested as custom mode jic it works
    if (!success) {
        unsigned int        refreshRate = output->getBackend()->type() == Aquamarine::eBackendType::AQ_BACKEND_DRM ? 60 * 1000 : 0;
        auto        mode        = Hyprutils::Memory::makeShared<Aquamarine::SOutputMode>(Aquamarine::SOutputMode{.pixelSize = {16*60, 9*60}, .refreshRate = refreshRate});
        std::string modeStr     = std::format("{:X0}@{:.2f}Hz", mode->pixelSize, mode->refreshRate / 1000.f);
        output->state->setCustomMode(mode);
        if (screen->test()) {
            printf("LOG: Monitor %s: using custom mode %s\n", name.c_str(), modeStr.c_str());
            refreshRate     = mode->refreshRate / 1000.f;
            success = true;
        } else
            printf("ERROR: Monitor %s: REJECTED custom mode %s!\n", name.c_str(), modeStr.c_str());
    }

    // try any of the modes if none of the above work
    if (!success) {
        for (auto const& mode : output->modes) {
            output->state->setMode(mode);

            if (!screen->test())
                continue;

            auto errorMessage =
                std::format("Monitor {} failed to set a\ny requested modes, falling back to mode {:X0}@{:.2f}Hz", name, mode->pixelSize, mode->refreshRate / 1000.f);
            printf("WARN: %s\n", errorMessage.c_str());
            success = true;
            break;
        }
    }

    if (!success) {
      panic("ERROR: Monitor {} has NO FALLBACK MODES\n", name.c_str());
    } else {
    	screen->isVBlank = true;
    	rmarUseScreen(screen.get());
     	glClear(GL_COLOR_BUFFER_BIT);
     	glClearColor(0.0, 0.0, 0.0, 0.0);
    	rmarSwapBuffers(screen.get());
    }
    return screen;
}
SP<rustamarine::RenderBuffer>
RustamarineScreen::getOrCreateRenderbuffer(SP<Aquamarine::IBuffer> buffer,
																					 uint32_t fmt) {
	if (buffer.get() == nullptr)
		return nullptr;
	// Try to find an existing renderbuffer for this buffer
	auto it = std::find_if(this->renderBuffers.begin(), this->renderBuffers.end(),
												 [&](const SP<rustamarine::RenderBuffer> &rb) {
													 return rb && rb->valid() && rb->isBuffer(buffer);
												 });
	if (it != this->renderBuffers.end())
		return *it;

	// Create a new RenderBuffer
	auto rb =
			makeShared<rustamarine::RenderBuffer>(buffer, fmt, this->rustamarine);

	if (!rb->valid())
		return nullptr;

	this->renderBuffers.emplace_back(rb);
	return rb;
}
void ensureOpenGLInitialized(SP<Rustamarine> rmar);
unsigned int rmarUseScreen(struct RustamarineScreen *screen) {
	ensureOpenGLInitialized(screen->rustamarine);
	if (!screen->currentBuffer.get()) {
		auto newBuffer = screen->output->swapchain->next(nullptr);
		screen->output->state->setBuffer(newBuffer);
		screen->currentBuffer = screen->getOrCreateRenderbuffer(
				newBuffer, screen->output->state->state().drmFormat);
		if (!screen->currentBuffer.get())
			panic("Failed to create render buffer")
	}
	screen->currentBuffer->bind();
	return screen->currentBuffer->frameBufferId();
}

void rmarSwapBuffers(struct RustamarineScreen *self) {
	RASSERT(self->isVBlank, "Tried to swap buffers of screen {} out of vblank",
					self->output->name);



	if (!self->currentBuffer.get()) {
		return;
	}
	// self->output->state->addDamage()

	auto eglSync = CEGLSync::create(self->rustamarine->openGLContext.eglDisplay);
	auto syncFd = eglSync->takeFd();
	if (eglSync->isValid()) {
		self->output->state->setExplicitInFence(syncFd.get());
	} else {
		self->output->state->resetExplicitFences();
	}
	self->output->state->setPresentationMode(
			Aquamarine::AQ_OUTPUT_PRESENTATION_VSYNC);
	// auto damageRegion = Hyprutils::Math::CRegion(
	// 		0, 0, self->output->physicalSize.x, self->output->physicalSize.y);
	// self->output->state->addDamage(damageRegion);
	RASSERT(self->output->commit(), "Failed to commit");
	self->currentBuffer.reset();
	self->rustamarine->inputManager.onFrameEnd();
}
bool RustamarineScreen::updateSwapchain() {
	auto options = this->output->swapchain->currentOptions();
	const auto &STATE = this->output->state->state();
	const auto &MODE = STATE.mode ? STATE.mode : STATE.customMode;
	options.format = STATE.drmFormat;
	options.scanout = true;
	options.length = 2;
	options.size = MODE->pixelSize;
	return this->output->swapchain->reconfigure(options);
}
bool rmarIsVBlank(const struct RustamarineScreen *self) {
	return self->isVBlank;
}

// Destructor for RustamarineScreen to free Rust closure if present
RustamarineScreen::~RustamarineScreen() {
	if (onRenderContext) {
		rmarFreeRustClosure(onRenderContext);
		onRenderContext = nullptr;
	}
}

extern "C" void rmarScreenSetOnRender(
	RustamarineScreen *screen,
	void (*callback)(void *, RustamarineScreen *), // callback: fn(*mut c_void, *mut RustamarineScreen)
	void *context
) {
	// Free previous closure if present
	if (screen->onRenderContext) {
		rmarFreeRustClosure(screen->onRenderContext);
		screen->onRenderContext = nullptr;
	}
	screen->onRenderCFunc = callback;
	screen->onRenderContext = context;
}
unsigned int rmarScreenGetWidth(const struct RustamarineScreen *screen) {
	if (!screen || !screen->output)
		return 0;
	const auto &state = screen->output->state->state();
	if (state.mode)
		return static_cast<unsigned int>(state.mode->pixelSize.x);
	if (state.customMode)
		return static_cast<unsigned int>(state.customMode->pixelSize.x);
	return static_cast<unsigned int>(screen->output->physicalSize.x);
}

unsigned int rmarScreenGetHeight(const struct RustamarineScreen *screen) {
	if (!screen || !screen->output)
		return 0;
	const auto &state = screen->output->state->state();
	if (state.mode)
		return static_cast<unsigned int>(state.mode->pixelSize.y);
	if (state.customMode)
		return static_cast<unsigned int>(state.customMode->pixelSize.y);
	return static_cast<unsigned int>(screen->output->physicalSize.y);
}

float rmarScreenGetRefreshRate(const struct RustamarineScreen *screen) {
	if (!screen || !screen->output)
		return 0.0f;
	const auto &state = screen->output->state->state();
	if (state.mode)
		return static_cast<float>(state.mode->refreshRate) / 1000.0f;
	if (state.customMode)
		return static_cast<float>(state.customMode->refreshRate) / 1000.0f;
	return 0.0f;
}

const char *rmarScreenGetName(const struct RustamarineScreen *screen) {
	if (!screen || !screen->output)
		return "";
	return screen->output->name.c_str();
}


bool rmarScreenIsEnabled(const struct RustamarineScreen *screen) {
	return screen->output->state->state().enabled;
}
void rmarScreenSetEnabled(const struct RustamarineScreen *screen, bool isEnabled) {
	if(isEnabled == screen->output->state->state().enabled) return;
	screen->output->state->setEnabled(isEnabled);
	screen->output->commit();
	if(isEnabled)
		screen->output->scheduleFrame(Aquamarine::IOutput::AQ_SCHEDULE_NEEDS_FRAME);
}
