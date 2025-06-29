#include "EGL/eglplatform.h"
#include "drm_fourcc.h"
#include "glad/glad.h"
#include "glad/glad_egl.h"
#include <EGL/eglext.h>
#include <cstdio>
#include <iostream>
#include <rustamarine/internal/opengl.hpp>
#include <rustamarine/internal/screen.hpp>

EGLImageKHR createEGLImage(const Aquamarine::SDMABUFAttrs &attrs,
													 SP<Rustamarine> rmar) {
	std::vector<uint32_t> attribs;

	attribs.push_back(EGL_WIDTH);
	attribs.push_back(attrs.size.x);
	attribs.push_back(EGL_HEIGHT);
	attribs.push_back(attrs.size.y);
	attribs.push_back(EGL_LINUX_DRM_FOURCC_EXT);
	attribs.push_back(attrs.format);

	struct {
		EGLint fd;
		EGLint offset;
		EGLint pitch;
		EGLint modlo;
		EGLint modhi;
	} attrNames[4] = {
			{EGL_DMA_BUF_PLANE0_FD_EXT, EGL_DMA_BUF_PLANE0_OFFSET_EXT,
			 EGL_DMA_BUF_PLANE0_PITCH_EXT, EGL_DMA_BUF_PLANE0_MODIFIER_LO_EXT,
			 EGL_DMA_BUF_PLANE0_MODIFIER_HI_EXT},
			{EGL_DMA_BUF_PLANE1_FD_EXT, EGL_DMA_BUF_PLANE1_OFFSET_EXT,
			 EGL_DMA_BUF_PLANE1_PITCH_EXT, EGL_DMA_BUF_PLANE1_MODIFIER_LO_EXT,
			 EGL_DMA_BUF_PLANE1_MODIFIER_HI_EXT},
			{EGL_DMA_BUF_PLANE2_FD_EXT, EGL_DMA_BUF_PLANE2_OFFSET_EXT,
			 EGL_DMA_BUF_PLANE2_PITCH_EXT, EGL_DMA_BUF_PLANE2_MODIFIER_LO_EXT,
			 EGL_DMA_BUF_PLANE2_MODIFIER_HI_EXT},
			{EGL_DMA_BUF_PLANE3_FD_EXT, EGL_DMA_BUF_PLANE3_OFFSET_EXT,
			 EGL_DMA_BUF_PLANE3_PITCH_EXT, EGL_DMA_BUF_PLANE3_MODIFIER_LO_EXT,
			 EGL_DMA_BUF_PLANE3_MODIFIER_HI_EXT}};

	for (int i = 0; i < attrs.planes; i++) {
		attribs.push_back(attrNames[i].fd);
		attribs.push_back(attrs.fds[i]);
		attribs.push_back(attrNames[i].offset);
		attribs.push_back(attrs.offsets[i]);
		attribs.push_back(attrNames[i].pitch);
		attribs.push_back(attrs.strides[i]);
		if (attrs.modifier != DRM_FORMAT_MOD_INVALID) {
			attribs.push_back(attrNames[i].modlo);
			attribs.push_back(attrs.modifier & 0xFFFFFFFF);
			attribs.push_back(attrNames[i].modhi);
			attribs.push_back(attrs.modifier >> 32);
		}
	}

	attribs.push_back(EGL_IMAGE_PRESERVED_KHR);
	attribs.push_back(EGL_TRUE);

	attribs.push_back(EGL_NONE);

	EGLImageKHR image =
			eglCreateImageKHR(rmar->openGLContext.eglDisplay, EGL_NO_CONTEXT,
												EGL_LINUX_DMA_BUF_EXT, nullptr, (int *)attribs.data());
	if (image == EGL_NO_IMAGE_KHR) {
		std::cerr << std::format("[ERROR] EGL: EGLCreateImageKHR failed: {}\n",
														 eglErrorToString(eglGetError()))
										 .data();
		return EGL_NO_IMAGE_KHR;
	}

	return image;
}

rustamarine::RenderBuffer::RenderBuffer(SP<Aquamarine::IBuffer> buffer,
																				uint32_t format, SP<Rustamarine> rmar)
		: underlyingBuffer(buffer), eglDisplay(rmar->openGLContext.eglDisplay) {
	if (buffer == nullptr)
		return;
	auto dma = buffer->dmabuf();
	this->eglImage = createEGLImage(dma, rmar);
	if (this->eglImage == EGL_NO_IMAGE_KHR) {
		printf("[ERROR] rb: createEGLImage failed\n");
		return;
	}

	glGenRenderbuffers(1, &this->renderBufferID);
	glBindRenderbuffer(GL_RENDERBUFFER, this->renderBufferID);
	glEGLImageTargetRenderbufferStorageOES(GL_RENDERBUFFER,
																				 (GLeglImageOES)eglImage);
	glBindRenderbuffer(GL_RENDERBUFFER, 0);

	glGenFramebuffers(1, &this->frameBufferID);
	this->width = buffer->size.x;
	this->height = buffer->size.y;

	glBindFramebuffer(GL_DRAW_FRAMEBUFFER, this->frameBufferID);
	glViewport(0, 0, this->width, this->height);
	glFramebufferRenderbuffer(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0,
														GL_RENDERBUFFER, this->renderBufferID);

	if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE) {
		printf("[ERROR] rbo: glCheckFramebufferStatus failed\n");
		return;
	}

	glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);
	this->destroyBufferListener =
			buffer->events.destroy.registerListener([this, rmar](std::any d) {
				for (auto &screen : rmar->screens) {
					std::erase_if(screen->renderBuffers, [this](const auto &rb) {
						return rb->renderBufferId() == this->renderBufferID;
					});
				}
			});

	this->isValid = true;
}

rustamarine::RenderBuffer::~RenderBuffer() {
	if (!this->isValid)
		return;
	if (this->frameBufferID) {
		glBindFramebuffer(GL_FRAMEBUFFER, 0);
		glDeleteFramebuffers(1, &this->frameBufferID);
		this->frameBufferID = 0;
	}
	if (this->renderBufferID) {
		glBindRenderbuffer(GL_RENDERBUFFER, 0);
		glDeleteRenderbuffers(1, &renderBufferID);
		this->renderBufferID = 0;
	}
	if (eglImage && eglImage != EGL_NO_IMAGE_KHR) {
		eglDestroyImageKHR(this->eglDisplay, eglImage);
		this->eglImage = EGL_NO_IMAGE_KHR;
	}
	this->isValid = false;
}

void rustamarine::RenderBuffer::bind() {
	glBindRenderbuffer(GL_RENDERBUFFER, this->renderBufferID);
	glBindFramebuffer(GL_DRAW_FRAMEBUFFER, this->frameBufferID);
}
