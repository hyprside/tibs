#pragma once
#include <hyprutils/memory/SharedPtr.hpp>
#include <hyprutils/signal/Listener.hpp>
#include <rustamarine.h>
#include "rustamarine/internal/utils.hpp"
#include <map>
#include <sys/types.h>
#include <string>
#include <vector>
#include <memory>
#include <xkbcommon/xkbcommon-keysyms.h>
#include <xkbcommon/xkbcommon.h>
#include <xkbcommon/xkbcommon-compose.h>
using Hyprutils::Signal::CHyprSignalListener;


namespace rustamarine {
	struct MouseButtonState {
		bool down = false;        // true if the key is currently pressed
		bool justChanged = false; // true if the key state just changed (pressed or released this frame)
	};
	struct KeyState {
			bool down = false;        // true if the key is currently pressed
			bool justChanged = false; // true if the key state just changed (pressed or released this frame)
			bool repeating = false;   // true if the key is repeating (held down and generating repeat events)
			bool shouldTypeChar = false;
			uint64_t lastTypedCharTimestamp;
			uint64_t stateChangedTimestamp;
		};

	class InputManager;
	class Mouse : public std::enable_shared_from_this<Mouse> {
		public:
		static SP<Mouse> create(SP<Aquamarine::IPointer> pointer, InputManager* inputManager);
		explicit Mouse(SP<Aquamarine::IPointer> pointer, InputManager* inputManager);
		InputManager* inputManager;
		SP<Aquamarine::IPointer> pointer;
		CHyprSignalListener onRelativeMoveListenerListener, onWarpListener, onScrollListener, onDisconnectListener;
		std::map<uint8_t, MouseButtonState> mouseButtonStates;
		friend InputManager;

	};
	class Keyboard : public std::enable_shared_from_this<Keyboard> {
		public:
		static SP<Keyboard> create(SP<Aquamarine::IKeyboard> keyboard, InputManager* inputManager);
		explicit Keyboard(SP<Aquamarine::IKeyboard> keyboard, InputManager* inputManager);
		~Keyboard();

		// Initialize xkbcommon for this keyboard
		bool initXkb();
		// Clean up xkbcommon resources
		void cleanupXkb();

		// Convert keysym to UTF-8 string
		std::string keysymToUtf8(xkb_keysym_t keysym);

		// Handle key event with xkbcommon
		void handleKeyEvent(xkb_keysym_t keysym);

		// xkbcommon state
		struct xkb_context* xkbContext = nullptr;
		struct xkb_keymap* xkbKeymap = nullptr;
		struct xkb_state* xkbState = nullptr;
		struct xkb_compose_table* xkbComposeTable = nullptr;
		struct xkb_compose_state* xkbComposeState = nullptr;
		std::map<xkb_keysym_t, KeyState> keystates;

		SP<Aquamarine::IKeyboard> keyboard;
		InputManager* inputManager;
		CHyprSignalListener onKeyUpdateListener, onDisconnectListener;
		friend InputManager;
	};

	class InputManager {
		public:
		InputManager() = default;
		explicit InputManager(SP<Rustamarine> rmar);

		// Get UTF-8 character string for the current frame
		const std::string& getUtf8Characters();
		void onPollEvents();
		CHyprSignalListener
			onNewKeyboardListener,
			onNewMouseListener;
		SP<Rustamarine> rmar;
		std::vector<SP<Mouse>> mouses;
		std::vector<SP<Keyboard>> keyboards;
		double mouseDeltaX = 0;
		double mouseDeltaY = 0;
		double mouseAbsoluteX = 0;
		double mouseAbsoluteY = 0;
		double mouseScrollX = 0;
		double mouseScrollY = 0;

		// Character input tracking
		std::string currentFrameUtf8Input;

		friend Mouse;
		friend Keyboard;
	};
}
