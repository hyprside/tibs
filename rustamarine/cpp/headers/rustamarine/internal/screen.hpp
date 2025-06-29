#pragma once
#include "./utils.hpp"
#include "aquamarine/buffer/Buffer.hpp"
#include "glad/glad.h"
#include "glad/glad_egl.h"
#include "rustamarine/internal/egl_sync.h"
#include <cstdint>
#include <hyprutils/memory/UniquePtr.hpp>
#include <rustamarine/internal/rustamarine.hpp>

#include <aquamarine/output/Output.hpp>
namespace rustamarine {
class RenderBuffer {
public:
	explicit RenderBuffer(SP<Aquamarine::IBuffer> buffer, uint32_t format,
												SP<Rustamarine> rmar);
	~RenderBuffer();
	void bind();
	inline bool valid() { return isValid; }
	inline GLuint renderBufferId() { return this->renderBufferID; }
	inline GLuint frameBufferId() { return this->frameBufferID; }
	inline bool isBuffer(SP<Aquamarine::IBuffer> buffer) {
		return underlyingBuffer.get() == buffer.get();
	}
	inline SP<Aquamarine::IBuffer> buffer() { return underlyingBuffer; }

private:
	Hyprutils::Signal::CHyprSignalListener destroyBufferListener;
	GLuint renderBufferID = 0, frameBufferID = 0;
	uint64_t width, height;
	EGLImageKHR eglImage;
	EGLDisplay eglDisplay;
	SP<Aquamarine::IBuffer> underlyingBuffer;
	bool isValid = false;
};
} // namespace rustamarine

struct RustamarineScreen {
	SP<Aquamarine::IOutput> output;
	SP<Rustamarine> rustamarine;
	// Each buffer in a surface will have their corresponding render buffer
	// The size of this vector will depend if the surface is double-buffered,
	// triple-buffered or N-buffered
	std::vector<SP<rustamarine::RenderBuffer>> renderBuffers;
	SP<rustamarine::RenderBuffer>
	getOrCreateRenderbuffer(SP<Aquamarine::IBuffer> buffer, uint32_t fmt);
	bool test();
	bool updateSwapchain();
	bool isVBlank = false;
	Hyprutils::Signal::CHyprSignalListener needsFrameListener, frameListener,
			onStateListener, presentListener;
	SP<rustamarine::RenderBuffer> currentBuffer;
	bool ensureCurrentBufferIsSet();
	// onRender callback fields
	void *onRenderContext = nullptr;
	void (*onRenderCFunc)(void *, RustamarineScreen *) = nullptr;

	~RustamarineScreen();
};

extern "C" {
void rmarFreeRustClosure(void *ptr);
}
