#pragma once
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <hyprutils/memory/UniquePtr.hpp>
#include <hyprutils/os/FileDescriptor.hpp>
class CEGLSync {
public:
	static Hyprutils::Memory::CUniquePointer<CEGLSync>
	create(EGLDisplay eglDisplay);

	~CEGLSync();

	Hyprutils::OS::CFileDescriptor &fd();
	Hyprutils::OS::CFileDescriptor &&takeFd();
	bool isValid();

private:
	CEGLSync() = default;

	Hyprutils::OS::CFileDescriptor sync_fd;
	EGLSyncKHR sync = EGL_NO_SYNC_KHR;
	bool valid = false;
	EGLDisplay eglDisplay;
	friend class CHyprOpenGLImpl;
};
