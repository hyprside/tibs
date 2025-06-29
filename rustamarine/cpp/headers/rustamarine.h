#pragma once
#ifdef __cplusplus
extern "C" {
#endif
#include <stdbool.h>
#include <stddef.h>

struct RustamarineScreen;
struct Rustamarine;

struct Rustamarine *rmarInitialize();
void *rmarGetProcAddress(struct Rustamarine *self, const char *procName);
void rmarPollEvents(struct Rustamarine *self);
void rmarTearDown(struct Rustamarine *self);

struct RustamarineScreens {
	struct RustamarineScreen **screens;
	size_t count;
};

struct RustamarineScreens rmarGetScreens(struct Rustamarine *self);
void rmarFreeScreens(struct RustamarineScreens screens);
bool rmarIsVBlank(const struct RustamarineScreen *self);
void rmarUseScreen(struct RustamarineScreen *screen);
void rmarSwapBuffers(struct RustamarineScreen *self);
struct Rustamarine *rmarFromScreen(struct RustamarineScreen *screen);
void rmarScreenSetOnRender(struct RustamarineScreen *screen,
													 void (*callback)(void *, struct RustamarineScreen *),
													 void *context);
int rmarScreenGetWidth(const struct RustamarineScreen *screen);
int rmarScreenGetHeight(const struct RustamarineScreen *screen);
float rmarScreenGetRefreshRate(const struct RustamarineScreen *screen);
const char *rmarScreenGetName(const struct RustamarineScreen *screen);
bool rmarScreenIsEnabled(const struct RustamarineScreen *screen);
#ifdef __cplusplus
}
#endif
