#include <glad/glad_egl.h>
#include <glad/glad.h>
#include <rustamarine/internal/utils.hpp>
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <GLES3/gl32.h>
#include <cstring>
#include <gbm.h>
#include <hyprutils/os/FileDescriptor.hpp>
#include <rustamarine/internal/opengl.hpp>
#include <rustamarine/internal/rustamarine.hpp>
#include <xf86drm.h>

static bool drmDeviceHasName(const drmDevice *device, const std::string &name) {
	for (size_t i = 0; i < DRM_NODE_MAX; i++) {
		if (!(device->available_nodes & (1 << i)))
			continue;

		if (device->nodes[i] == name)
			return true;
	}
	return false;
}
EGLDeviceEXT eglDeviceFromDRMFD(int drmFD) {
	EGLint nDevices = 0;
	if (!eglQueryDevicesEXT(0, nullptr, &nDevices)) {
		fprintf(stderr, "eglDeviceFromDRMFD: eglQueryDevicesEXT failed\n");
		return EGL_NO_DEVICE_EXT;
	}

	if (nDevices <= 0) {
		fprintf(stderr, "eglDeviceFromDRMFD: no devices\n");
		return EGL_NO_DEVICE_EXT;
	}

	std::vector<EGLDeviceEXT> devices;
	devices.resize(nDevices);

	if (!eglQueryDevicesEXT(nDevices, devices.data(), &nDevices)) {
		fprintf(stderr, "eglDeviceFromDRMFD: eglQueryDevicesEXT failed (2)\n");
		return EGL_NO_DEVICE_EXT;
	}

	drmDevice *drmDev = nullptr;
	if (int ret = drmGetDevice(drmFD, &drmDev); ret < 0) {
		fprintf(stderr, "eglDeviceFromDRMFD: drmGetDevice failed\n");
		return EGL_NO_DEVICE_EXT;
	}

	for (auto const &d : devices) {
		auto devName = eglQueryDeviceStringEXT(d, EGL_DRM_DEVICE_FILE_EXT);
		if (!devName)
			continue;

		if (drmDeviceHasName(drmDev, devName)) {
			printf("[LOG] eglDeviceFromDRMFD: Using device %s\n", devName);
			drmFreeDevice(&drmDev);
			return d;
		}
	}

	drmFreeDevice(&drmDev);
	printf("[LOG] eglDeviceFromDRMFD: No drm devices found\n");
	return EGL_NO_DEVICE_EXT;
}
static int openRenderNode(int drmFd) {
	auto renderName = drmGetRenderDeviceNameFromFd(drmFd);
	if (!renderName) {
		// This can happen on split render/display platforms, fallback to
		// primary node
		renderName = drmGetPrimaryDeviceNameFromFd(drmFd);
		if (!renderName) {
			printf("[ERR] drmGetPrimaryDeviceNameFromFd failed\n");
			return -1;
		}
		printf("[LOG] DRM dev %s has no render node, falling back to primary\n",
					 renderName);

		drmVersion *render_version = drmGetVersion(drmFd);
		if (render_version && render_version->name) {
			printf("[LOG] DRM dev versionName %s\n", render_version->name);
			if (strcmp(render_version->name, "evdi") == 0) {
				free(renderName);
				renderName = (char *)malloc(sizeof(char) * 15);
				strcpy(renderName, "/dev/dri/card0");
			}
			drmFreeVersion(render_version);
		}
	}

	printf("[LOG] openRenderNode got drm device %s\n", renderName);

	int renderFD = open(renderName, O_RDWR | O_CLOEXEC);
	if (renderFD < 0)
		printf("[ERR] openRenderNode failed to open drm device %s\n", renderName);

	free(renderName);
	return renderFD;
}

void initEGL(SP<Rustamarine> rmar, bool gbm = false) {
	if (gbm) {
		rmar->openGLContext.gbmFd =
				Hyprutils::OS::CFileDescriptor{openRenderNode(rmar->backend->drmFD())};
		if (!rmar->openGLContext.gbmFd.isValid())
			RASSERT(false, "Couldn't open a gbm fd");

		rmar->openGLContext.gbmDevice =
				gbm_create_device(rmar->openGLContext.gbmFd.get());
		if (!rmar->openGLContext.gbmDevice)
			RASSERT(false, "Couldn't open a gbm device");

	} else {
		rmar->openGLContext.eglDevice = eglDeviceFromDRMFD(rmar->backend->drmFD());
	}
	auto eglDisplay = eglGetPlatformDisplayEXT(
			gbm ? EGL_PLATFORM_GBM_KHR : EGL_PLATFORM_DEVICE_EXT,
			gbm ? rmar->openGLContext.gbmDevice : rmar->openGLContext.eglDevice,
			nullptr);
	if (!eglDisplay) {
		if (gbm) {
			panic("Failed to initialize EGL Display (eglGetPlatformDisplayEXT)")
		} else {
			initEGL(rmar, true);
			return;
		}
	}
	EGLint version[2] = {0, 0};
	if (eglInitialize(eglDisplay, &version[0], &version[1]) == EGL_FALSE) {
		if (gbm) {
			panic("Failed to initialize EGL Display (eglInitialize)")
		} else {
			initEGL(rmar, true);
			return;
		}
	}
	printf("[LOG] EGL version: %d.%d\n", version[0], version[1]);
	auto eglContext =
			eglCreateContext(eglDisplay, EGL_NO_CONFIG_KHR, EGL_NO_CONTEXT,
											 (int[]){EGL_CONTEXT_MAJOR_VERSION, 3,
															 EGL_CONTEXT_MINOR_VERSION, 2, EGL_NONE});
	if (eglContext == EGL_NO_CONTEXT)
		panic("Failed to create EGL Context") {
			EGLint priority = EGL_CONTEXT_PRIORITY_MEDIUM_IMG;
			eglQueryContext(eglDisplay, eglContext, EGL_CONTEXT_PRIORITY_LEVEL_IMG,
											&priority);
		}
	eglMakeCurrent(eglDisplay, EGL_NO_SURFACE, EGL_NO_SURFACE, eglContext);
	rmar->openGLContext.eglDisplay = eglDisplay;
	rmar->openGLContext.eglContext = eglContext;
}

static void EGLAPIENTRY eglLog(EGLenum error, const char *command,
															 EGLint messageType, EGLLabelKHR threadLabel,
															 EGLLabelKHR objectLabel, const char *message) {
	const char *typeStr = "";
	switch (messageType) {
	case EGL_DEBUG_MSG_CRITICAL_KHR:
		typeStr = "CRITICAL";
		break;
	case EGL_DEBUG_MSG_ERROR_KHR:
		typeStr = "ERROR";
		break;
	case EGL_DEBUG_MSG_WARN_KHR:
		typeStr = "WARN";
		break;
	case EGL_DEBUG_MSG_INFO_KHR:
		typeStr = "INFO";
		break;
	default:
		typeStr = "UNKNOWN";
		break;
	}
	fprintf(stderr, "[EGL %s] error=%s, cmd=%s, msg=%s\n", typeStr,
					eglErrorToString(error), command ? command : "(null)",
					message ? message : "(null)");
}
void initializeOpenGL(SP<Rustamarine> rmar) {
	gladLoadEGL();
	static const EGLAttrib debugAttrs[] = {
			EGL_DEBUG_MSG_CRITICAL_KHR,
			EGL_TRUE,
			EGL_DEBUG_MSG_ERROR_KHR,
			EGL_TRUE,
			EGL_DEBUG_MSG_WARN_KHR,
			EGL_TRUE,
			EGL_DEBUG_MSG_INFO_KHR,
			EGL_TRUE,
			EGL_NONE,
	};
	eglDebugMessageControlKHR(::eglLog, debugAttrs);
	if (gladLoadGLES2Loader((GLADloadproc)eglGetProcAddress)){
		rmar->openGLContext.eglDisplay = eglGetCurrentDisplay();
		rmar->openGLContext.eglContext = eglGetCurrentContext();
		rmar->openGLContext.eglDevice = eglDeviceFromDRMFD(rmar->backend->drmFD());
		return;
	}
	eglBindAPI(EGL_OPENGL_ES_API);
	initEGL(rmar);
	if (!gladLoadGLES2Loader((GLADloadproc)eglGetProcAddress))
		panic("Failed to load OpenGL functions with glad");
	printf("[LOG] Initialized OpenGL Context!\n");
	printf("[LOG] Using: %s\n", (char *)glGetString(GL_VERSION));
	printf("[LOG] Vendor: %s\n", (char *)glGetString(GL_VENDOR));
	printf("[LOG] Renderer: %s\n", (char *)glGetString(GL_RENDERER));
}
void tearDownOpenGL(RustamarineOpenGLContext *opengl) {
	if (opengl->eglDisplay != nullptr && opengl->eglContext != nullptr) {
		eglDestroyContext(opengl->eglDisplay, opengl->eglContext);
	}
	eglReleaseThread();
}
void *rmarGetProcAddress(struct Rustamarine *, const char *procName) {
  auto fn = (void *)eglGetProcAddress(procName);
	return fn;
}
void ensureOpenGLInitialized(SP<Rustamarine> rmar) {
 if (rmar->openGLContext.eglContext == EGL_NO_CONTEXT) {
  initializeOpenGL(rmar);
 }
}
