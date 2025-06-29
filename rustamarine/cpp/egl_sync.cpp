#include "glad/glad.h"
#include "glad/glad_egl.h"
#include "rustamarine/internal/egl_sync.h"
#include <hyprutils/memory/UniquePtr.hpp>
#include <hyprutils/os/FileDescriptor.hpp>
#define UP Hyprutils::Memory::CUniquePointer
UP<CEGLSync> CEGLSync::create(EGLDisplay eglDisplay) {
	EGLSyncKHR sync =
			eglCreateSyncKHR(eglDisplay, EGL_SYNC_NATIVE_FENCE_ANDROID, nullptr);

	if (sync == EGL_NO_SYNC_KHR) {
		fprintf(stderr, "eglCreateSyncKHR failed\n");
		return nullptr;
	}

	// we need to flush otherwise we might not get a valid fd
	glFlush();

	int fd = eglDupNativeFenceFDANDROID(eglDisplay, sync);
	if (fd == EGL_NO_NATIVE_FENCE_FD_ANDROID) {
		fprintf(stderr, "eglDupNativeFenceFDANDROID failed\n");
		return nullptr;
	}

	UP<CEGLSync> eglSync(new CEGLSync);
	eglSync->sync_fd = Hyprutils::OS::CFileDescriptor(fd);
	eglSync->sync = sync;
	eglSync->valid = true;
	eglSync->eglDisplay = eglDisplay;
	return eglSync;
}

CEGLSync::~CEGLSync() {
	if (this->sync == EGL_NO_SYNC_KHR)
		return;

	if (!eglDestroySyncKHR ||
			eglDestroySyncKHR(this->eglDisplay, sync) != EGL_TRUE)
		fprintf(stderr, "eglDestroySyncKHR failed\n");
}

Hyprutils::OS::CFileDescriptor &CEGLSync::fd() { return this->sync_fd; }

Hyprutils::OS::CFileDescriptor &&CEGLSync::takeFd() {
	return std::move(this->sync_fd);
}

bool CEGLSync::isValid() {
	return this->valid && this->sync != EGL_NO_SYNC_KHR &&
				 this->sync_fd.isValid();
}
