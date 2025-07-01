pub mod keys;
use super::*;
impl Rustamarine {
	/// Check if a key is currently being held down.
	///
	/// # Arguments
	///
	/// * `key` - The key to check, can be a constant from the [`keys`] module
	///
	/// # Returns
	///
	/// `true` if the key is currently down, `false` otherwise
	pub fn is_key_down(&self, key: u32) -> bool {
		unsafe { sys::rmarIsKeyDown(self.inner, key) }
	}

	/// Check if a key was just pressed this frame.
	///
	/// This function only returns `true` for one frame when the key is first pressed.
	///
	/// # Arguments
	///
	/// * `key` - The key to check, can be a constant from the [`keys`] module
	///
	/// # Returns
	///
	/// `true` if the key was just pressed, `false` otherwise
	pub fn is_key_pressed(&self, key: u32) -> bool {
		unsafe { sys::rmarIsKeyPressed(self.inner, key) }
	}

	/// Check if a key was just released this frame.
	///
	/// This function only returns `true` for one frame when the key is released.
	///
	/// # Arguments
	///
	/// * `key` - The key to check, can be a constant from the [`keys`] module
	///
	/// # Returns
	///
	/// `true` if the key was just released, `false` otherwise
	pub fn is_key_released(&self, key: u32) -> bool {
		unsafe { sys::rmarIsKeyReleased(self.inner, key) }
	}

	/// Check if a key should generate a character in text input.
	///
	/// This accounts for key repeating behavior - when a key is held, it will
	/// periodically return `true` to simulate keyboard repeat.
	///
	/// # Arguments
	///
	/// * `key` - The key to check, can be a constant from the [`keys`] module
	///
	/// # Returns
	///
	/// `true` if the key should generate a character, `false` otherwise
	pub fn should_type_key(&self, key: u32) -> bool {
		unsafe { sys::rmarShouldTypeKey(self.inner, key) }
	}
	/// Check if a mouse button is currently being held down.
	///
	/// # Arguments
	///
	/// * `button` - The mouse button to check (e.g., [`keys::MOUSE_LEFT`], [`keys::MOUSE_RIGHT`])
	///
	/// # Returns
	///
	/// `true` if the button is currently down, `false` otherwise
	pub fn is_mouse_button_down(&self, button: u32) -> bool {
		unsafe { sys::rmarIsMouseButtonDown(self.inner, button) }
	}

	/// Check if a mouse button was just pressed this frame.
	///
	/// This function only returns `true` for one frame when the button is first pressed.
	///
	/// # Arguments
	///
	/// * `button` - The mouse button to check (e.g., [`keys::MOUSE_LEFT`], [`keys::MOUSE_RIGHT`])
	///
	/// # Returns
	///
	/// `true` if the button was just pressed, `false` otherwise
	pub fn is_mouse_button_pressed(&self, button: u32) -> bool {
		unsafe { sys::rmarIsMouseButtonPressed(self.inner, button) }
	}

	/// Check if a mouse button was just released this frame.
	///
	/// This function only returns `true` for one frame when the button is released.
	///
	/// # Arguments
	///
	/// * `button` - The mouse button to check (e.g., [`keys::MOUSE_LEFT`], [`keys::MOUSE_RIGHT`])
	///
	/// # Returns
	///
	/// `true` if the button was just released, `false` otherwise
	pub fn is_mouse_button_released(&self, button: u32) -> bool {
		unsafe { sys::rmarIsMouseButtonReleased(self.inner, button) }
	}

	/// Get the current mouse X position.
	///
	/// # Returns
	///
	/// The current X coordinate of the mouse cursor
	pub fn get_mouse_x(&self) -> i32 {
		unsafe { sys::rmarGetMouseX(self.inner) }
	}

	/// Get the current mouse Y position.
	///
	/// # Returns
	///
	/// The current Y coordinate of the mouse cursor
	pub fn get_mouse_y(&self) -> i32 {
		unsafe { sys::rmarGetMouseY(self.inner) }
	}

	/// Get the change in mouse X position since the last frame.
	///
	/// # Returns
	///
	/// The delta X value (positive is right, negative is left)
	pub fn get_mouse_delta_x(&self) -> i32 {
		unsafe { sys::rmarGetMouseDeltaX(self.inner) }
	}

	/// Get the change in mouse Y position since the last frame.
	///
	/// # Returns
	///
	/// The delta Y value (positive is down, negative is up)
	pub fn get_mouse_delta_y(&self) -> i32 {
		unsafe { sys::rmarGetMouseDeltaY(self.inner) }
	}

	/// Get the horizontal scroll wheel movement.
	///
	/// # Returns
	///
	/// The horizontal scroll amount since the last frame
	pub fn get_mouse_scroll_x(&self) -> i32 {
		unsafe { sys::rmarGetMouseScrollX(self.inner) }
	}

	/// Get the vertical scroll wheel movement.
	///
	/// # Returns
	///
	/// The vertical scroll amount since the last frame (positive is down, negative is up)
	pub fn get_mouse_scroll_y(&self) -> i32 {
		unsafe { sys::rmarGetMouseScrollY(self.inner) }
	}
	/// Set the mouse X position.
	///
	/// # Arguments
	///
	/// * `x` - The new X coordinate for the mouse cursor
	pub fn set_mouse_x(&self, x: i32) {
		unsafe { sys::rmarSetMouseX(self.inner, x) }
	}

	/// Set the mouse Y position.
	///
	/// # Arguments
	///
	/// * `y` - The new Y coordinate for the mouse cursor
	pub fn set_mouse_y(&self, y: i32) {
		unsafe { sys::rmarSetMouseY(self.inner, y) }
	}
	/// Get the UTF-8 text input for the current frame.
	///
	/// This returns all the characters typed since the last frame, taking into account
	/// keyboard layout, modifiers, and compose sequences. This is ideal for text input
	/// in applications and games.
	///
	/// # Returns
	///
	/// A UTF-8 string containing all characters typed since the last frame, or an empty
	/// string if no characters were typed
	///
	/// # Example
	///
	/// ```
	/// let rmar = Rustamarine::new();
	/// // In your main loop:
	/// rmar.poll_events();
	/// let text_input = rmar.get_char();
	/// if !text_input.is_empty() {
	///     // Handle text input
	///     my_text_buffer.push_str(text_input);
	/// }
	/// ```
	pub fn get_typed_characters(&self) -> String {
		unsafe {
			let ptr = sys::rmarGetTypedCharacters(self.inner);
			if ptr.is_null() {
				return String::new();
			}
			let c_str = std::ffi::CStr::from_ptr(ptr);
			c_str.to_str().map(|s| s.to_string()).unwrap_or_default()
		}
	}
}
