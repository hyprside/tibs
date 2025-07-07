use std::collections::HashMap;
use std::sync::Arc;

use super::{LoginManager, LoginState};
use crate::animation::colors::hsl_to_rgb;
use crate::frame_pool::FrameAllocator;
use crate::session_manager::{self, DesktopEnvironmentFile, SessionManager, SessionStatus};
use crate::textbox::Textbox;
use crate::{custom_elements::CustomElements, skia::asset_loaders::SkiaImageAsset};
use crate::{format_id, frame_alloc_format, TibsClayScope};
use assets_manager::AssetCache;
use clay_layout::fit;
use clay_layout::text::TextElementConfig;
use rustamarine::keys::{KEY_KP_Enter, KEY_Return};
use rustamarine::Rustamarine;
use skia_safe::Image;
use uzers::os::unix::UserExt;
use uzers::{all_users, User};

use clay_layout::{
	elements::{FloatingAttachPointType, FloatingAttachToElement},
	fixed, grow,
	layout::{Alignment, LayoutAlignmentX as LX, LayoutAlignmentY as LY, LayoutDirection, Padding},
	text::TextConfig,
	Declaration,
};
#[derive(Hash, PartialEq, Eq, Clone, Copy)]

enum KnownDEs {
	KDE,
	GNOME,
	Hyprland,
	HyprDE,
	Unknown,
}
impl KnownDEs {
	pub fn from_de(de: &DesktopEnvironmentFile) -> Self {
		if de.name().starts_with("Plasma (") {
			Self::KDE
		} else if de.name().starts_with("GNOME Shell") {
			Self::GNOME
		} else if de.name() == "Hyprland" {
			Self::Hyprland
		} else if de.name() == "HyprDE" {
			Self::HyprDE
		} else {
			Self::Unknown
		}
	}
}
// --------- Login Screen

pub struct LoginScreen {
	user_list: Vec<User>,
	selected_user: u32,
	selected_username: String,
	login_icon: Image,
	eye_icon: Image,
	eye_off_icon: Image,
	avatars: HashMap<u32, Image>,
	password_input: Textbox,
	de_icons: HashMap<KnownDEs, SkiaImageAsset>,
	is_desktop_environment_popup_open: bool,
	selected_de: Option<DesktopEnvironmentFile>,
	session_open_error: Option<String>,
}

fn is_user_uid(uid: u32) -> bool {
	return uid >= 1000 && uid < 65534;
}

fn load_avatar(u: &User) -> Option<Image> {
	let face_file_path = u.home_dir().join(".face");
	let face_image_data = skia_safe::Data::from_filename(face_file_path)?;
	let face_image = Image::from_encoded(face_image_data)?;
	return Some(face_image);
}

pub enum LoginScreenOutMessage {
	StartLoginAnimation,
}
impl LoginScreen {
	pub fn username(&self) -> &str {
		&self.selected_username
	}
	pub fn new(assets: &AssetCache) -> Self {
		let SkiaImageAsset(login_icon) = assets
			.load_owned("icons.login")
			.expect("Failed to load icons.login");
		let SkiaImageAsset(eye_icon) = assets
			.load_owned("icons.eye")
			.expect("Failed to load icons.eye");
		let SkiaImageAsset(eye_off_icon) = assets
			.load_owned("icons.eye-off")
			.expect("Failed to load icons.eye-off");

		let mut de_icons: HashMap<KnownDEs, SkiaImageAsset> = HashMap::new();
		de_icons.insert(KnownDEs::KDE, assets.load_owned("icons.kde").unwrap());
		de_icons.insert(KnownDEs::GNOME, assets.load_owned("icons.gnome").unwrap());
		de_icons.insert(
			KnownDEs::Hyprland,
			assets.load_owned("icons.hyprland").unwrap(),
		);
		de_icons.insert(KnownDEs::HyprDE, assets.load_owned("icons.hyprde").unwrap());
		de_icons.insert(
			KnownDEs::Unknown,
			assets.load_owned("icons.unknown").unwrap(),
		);
		let user_list = unsafe { all_users() }
			.filter(|u| is_user_uid(u.uid()) && !u.shell().ends_with("nologin"))
			.collect::<Vec<User>>();

		let selected_user = user_list[0].uid();
		let selected_username = user_list[0].name().to_str().unwrap().to_string();
		Self {
			avatars: user_list
				.iter()
				.filter_map(|u| Some((u.uid(), load_avatar(u)?)))
				.collect(),
			user_list,
			selected_user,
			selected_username,
			login_icon,
			password_input: Textbox::new("login-input", true),
			eye_icon,
			eye_off_icon,
			de_icons,

			// Don't forget to reset these fields when switching users
			is_desktop_environment_popup_open: false,
			selected_de: None,
			session_open_error: None,
		}
	}
	pub fn update<'clay, 'render>(
		&'render mut self,
		c: &mut clay_layout::Clay,
		rmar: &Rustamarine,
		login_manager: &mut LoginManager,
		session_manager: &SessionManager,
	) where
		'clay: 'render,
	{
		if rmar.is_mouse_button_pressed(0) && !c.pointer_over(c.id("desktop-environments-popup")) {
			self.is_desktop_environment_popup_open = false;
		}
		if let Some(selected) = self
			.user_list
			.iter()
			.find(|u| u.uid() == self.selected_user)
		{
			let n = selected.name().to_str().unwrap();
			if self.selected_username != n {
				self.selected_username = n.to_string();
			}
		}
		self.password_input.update(rmar, &mut *c);
		if c.pointer_over(c.id("show-password")) && rmar.is_mouse_button_released(0) {
			self.password_input.hide_input = !self.password_input.hide_input
		}
		if ((c.pointer_over(c.id("login-button")) && rmar.is_mouse_button_released(0))
			|| (self.password_input.is_focused()
				&& (rmar.is_key_pressed(KEY_Return) || rmar.is_key_pressed(KEY_KP_Enter))))
			&& !self.password_input.disabled
		{
			if session_manager.get_desktop_environments_list().len() == 1 {
				self.on_de_select(
					session_manager
						.get_desktop_environments_list()
						.first()
						.unwrap(),
					login_manager,
					session_manager,
				);
			} else {
				self.is_desktop_environment_popup_open = true
			}
		}
		self.password_input.disabled = self.is_logging(login_manager, session_manager);
		for (i, de) in session_manager
			.get_desktop_environments_list()
			.iter()
			.enumerate()
		{
			if c.pointer_over(c.id_index("desktop-environment", i as u32))
				&& rmar.is_mouse_button_released(0)
			{
				self.on_de_select(de, login_manager, session_manager);
			}
		}
	}
	fn on_de_select(
		&mut self,
		de: &DesktopEnvironmentFile,
		login_manager: &mut LoginManager,
		session_manager: &SessionManager,
	) {
		if self.is_logging(login_manager, session_manager)
			|| session_manager.is_running(self.selected_user)
		{
			return;
		}
		self.selected_de = Some(de.clone());
		self.session_open_error = None;
		self.is_desktop_environment_popup_open = false;
		login_manager.start_login(&self.selected_username, self.password_input.text(), true);
	}
	pub fn start_session(
		&mut self,
		login_manager: &LoginManager,
		session_manager: &mut SessionManager,
	) {
		if let Some(selected_de) = &self.selected_de {
			if let Err(e) =
				session_manager.start_session(login_manager, &self.selected_username, selected_de)
			{
				self.session_open_error = Some(e.to_string());
			}
		}
	}
	pub fn session_open_failed(&self) -> bool {
		self.session_open_error.is_some()
	}
	pub fn authenticated_with_no_session(
		&self,
		login_manager: &LoginManager,
		session_manager: &SessionManager,
	) -> Option<u32> {
		match login_manager.get_current_login_state(&self.selected_username) {
			Some(LoginState::Authenticated(u)) => {
				if !session_manager.is_running(u) {
					Some(u)
				} else {
					None
				}
			}
			_ => None,
		}
	}
	fn is_logging(&self, login_manager: &LoginManager, session_manager: &SessionManager) -> bool {
		if matches!(
			login_manager.get_current_login_state(&self.selected_username),
			Some(LoginState::Logging | LoginState::Authenticated(_))
		) {
			matches!(
				session_manager.get_session_state_of_user(self.selected_user),
				None | Some(SessionStatus::Crashed) | Some(SessionStatus::ShutdownGracefully)
			)
		} else {
			false
		}
	}
	fn login_failed(&self, login_manager: &LoginManager) -> bool {
		matches!(
			login_manager.get_current_login_state(&self.selected_username),
			Some(LoginState::Failed)
		)
	}
	pub fn render<'clay, 'render>(
		&'render self,
		c: &mut TibsClayScope<'clay, 'render>,
		login_manager: &LoginManager,
		session_manager: &'render SessionManager,
		frame_pool: &FrameAllocator<'render>,
		rmar: &Rustamarine,
	) where
		'clay: 'render,
	{
		self.render_selected_user(c, login_manager, session_manager, frame_pool, rmar);
		if !self.is_logging(login_manager, session_manager) {
			self.render_user_list(c, frame_pool);
		}
	}

	fn render_user_list<'clay, 'render>(
		&'render self,
		c: &mut TibsClayScope<'clay, 'render>,
		frame_pool: &FrameAllocator<'render>,
	) where
		'clay: 'render,
	{
		c.with(
			Declaration::new()
				.floating()
				.attach_to(FloatingAttachToElement::Parent)
				.attach_points(
					FloatingAttachPointType::LeftBottom,
					FloatingAttachPointType::LeftBottom,
				)
				.offset((55.0, -55.0).into())
				.end()
				.layout()
				.direction(LayoutDirection::TopToBottom)
				.width(fit!(250.0))
				.end(),
			|c| {
				for user in &self.user_list {
					let is_selected = user.uid() == self.selected_user;
					self.render_user_item(c, user, is_selected, &frame_pool);
				}
			},
		);
	}

	fn render_user_item<'clay, 'render>(
		&'render self,
		c: &mut TibsClayScope<'clay, 'render>,
		user: &'render User,
		is_selected: bool,
		frame_pool: &FrameAllocator<'render>,
	) where
		'clay: 'render,
	{
		let user_name = user.name().to_str().unwrap();
		let id = c.id(frame_pool.alloc(format!("user_item-{user_name}")).as_str());
		// If the user is selected, apply a highlight background color.
		let mut decl = Declaration::new();
		decl
			.layout()
			.direction(LayoutDirection::LeftToRight)
			.padding(Padding::all(5))
			.child_gap(20)
			.child_alignment(Alignment::new(LX::Left, LY::Center))
			.width(grow!())
			.padding(Padding::all(10))
			.end()
			.corner_radius()
			.all(10.)
			.end()
			.id(id);

		let is_hovered = c.pointer_over(id);
		if is_hovered {
			decl.background_color((0x2E / 2, 0x3E / 2, 0x4E / 2, 0x30).into());
		} else if is_selected {
			decl.background_color((0x2E, 0x3E, 0x4E, 0x30).into());
		}

		c.with(&decl, |c| {
			// User avatar as a circle
			let mut avatar_declaration = Declaration::new();
			avatar_declaration
				.layout()
				.width(fixed!(50.0))
				.height(fixed!(50.0))
				.end()
				.corner_radius()
				.all(99999.0)
				.end();
			if let Some(avatar) = self.avatars.get(&user.uid()) {
				avatar_declaration.image().data(avatar).end();
			}
			c.with(&avatar_declaration, |_| {});
			// Display name text
			c.text(
				&user_name,
				TextConfig::new()
					.color((0xFF, 0xFF, 0xFF).into())
					.font_size(20)
					.alignment(clay_layout::text::TextAlignment::Left)
					.end(),
			);
		});
	}

	fn render_selected_user<'clay, 'render>(
		&'render self,
		c: &mut TibsClayScope<'clay, 'render>,
		login_manager: &LoginManager,
		session_manager: &'render SessionManager,
		frame_pool: &FrameAllocator<'render>,
		rmar: &Rustamarine,
	) where
		'clay: 'render,
	{
		// Retrieve the selected user info
		if let Some(selected) = self
			.user_list
			.iter()
			.find(|u| u.uid() == self.selected_user)
		{
			c.with(
				Declaration::new()
					.layout()
					.child_alignment(Alignment::new(LX::Center, LY::Center))
					.width(grow!())
					.height(grow!())
					.end(),
				|c| {
					// Container for the selected user avatar and name
					c.with(
						Declaration::new()
							.layout()
							.child_alignment(Alignment::new(LX::Center, LY::Center))
							.padding(Padding::new(10, 10, 24, 24))
							.width(grow!(238.0))
							.direction(LayoutDirection::TopToBottom)
							.padding(Padding::all(20))
							.end(),
						|c| {
							let mut avatar_declaration = Declaration::new();
							avatar_declaration
								.layout()
								.width(fixed!(128.0))
								.height(fixed!(128.0))
								.end()
								.background_color((0xAA, 0xAA, 0xAA, 0x30).into())
								.corner_radius()
								.all(99999.0)
								.end();

							if let Some(avatar) = self.avatars.get(&selected.uid()) {
								avatar_declaration.image().data(avatar).end();
							}
							// Selected user avatar
							c.with(&avatar_declaration, |_| {});
							// Space between avatar and name
							c.with(
								Declaration::new()
									.layout()
									.width(grow!())
									.height(fixed!(20.0))
									.end(),
								|_| {},
							);
							let user_name = selected.name().to_str().unwrap();
							// Selected user name text
							c.text(
								&user_name,
								TextConfig::new()
									.color((0xFF, 0xFF, 0xFF).into())
									.font_size(32)
									.alignment(clay_layout::text::TextAlignment::Center)
									.end(),
							);

							let error_message = self.session_open_error.as_deref().or_else(|| {
								self.login_failed(login_manager).then_some(
									"Failed to login, please check if your password is correct and try again.",
								)
							});
							if let Some(error_message) = error_message {
								// Selected user name text
								c.with(
									Declaration::new()
										.layout()
										.padding(Padding::new(0, 0, 40, 8))
										.width(fit!(0., 600.))
										.end(),
									|c| {
										c.text(
											error_message,
											TextConfig::new()
												.color((0xFF, 0x50, 0x50).into())
												.font_size(16)
												.font_id(2)
												.alignment(clay_layout::text::TextAlignment::Center)
												.end(),
										);
									},
								);
							}
							c.with(
								Declaration::new()
									.layout()
									.child_alignment(Alignment::new(LX::Center, LY::Center))
									.end()
									.layout()
									.padding(Padding::new(
										0,
										0,
										if error_message.is_some() { 0 } else { 56 },
										0,
									))
									.child_gap(14)
									.end(),
								|c| {
									self.password_input.render(c);
									self.render_eye_button(c, rmar);
									self.render_login_button(c, login_manager, session_manager, frame_pool, rmar);
								},
							);
						},
					);
				},
			);
		}
	}

	fn render_login_button<'clay, 'render>(
		&'render self,
		c: &mut TibsClayScope<'clay, 'render>,
		login_manager: &LoginManager,
		session_manager: &'render SessionManager,
		frame_pool: &FrameAllocator<'render>,
		rmar: &Rustamarine,
	) where
		'clay: 'render,
	{
		c.with_styling(
			|c| {
				let mut d = Declaration::new();
				d.id(c.id("login-button"))
					.layout()
					.child_alignment(Alignment::new(LX::Center, LY::Center))
					.width(fixed!(50.0))
					.height(fixed!(50.0))
					.end()
					.background_color((0x0E, 0x1A, 0x26, 0x30).into())
					.corner_radius()
					.all(10.0)
					.end();

				// Adiciona borda vermelha se login falhar
				if self.login_failed(login_manager) {
					d.border()
						.color((255, 0, 0, 255).into())
						.all_directions(2)
						.end();
				}
				if c.hovered() {
					d.background_color((0x0E + 20, 0x1A + 20, 0x26 + 20, 0x30).into());
					if rmar.is_mouse_button_down(0) {
						d.background_color((0x0E + 30, 0x1A + 30, 0x26 + 30, 0x30).into());
					}
				}
				d
			},
			|c| {
				if self.is_logging(login_manager, session_manager) {
					// Mostra apenas o spinner
					c.with(
						Declaration::new()
							.layout()
							.width(fixed!(18.0))
							.height(fixed!(18.0))
							.end()
							.custom_element(&CustomElements::Spinner),
						|_| {},
					)
				} else {
					// Ícone normal do botão de login
					c.with(
						Declaration::new()
							.image()
							.data(&self.login_icon)
							.end()
							.layout()
							.width(fixed!(24.0))
							.height(fixed!(24.0))
							.end(),
						|_| {},
					)
				}
				if self.is_desktop_environment_popup_open {
					desktop_environments_popup(session_manager, c, frame_pool, &self.de_icons, rmar);
				}
			},
		);
	}

	fn render_eye_button<'clay, 'render>(
		&'render self,
		c: &mut TibsClayScope<'clay, 'render>,
		rmar: &Rustamarine,
	) where
		'clay: 'render,
	{
		c.with_styling(
			|c| {
				let mut d = Declaration::new();
				d.layout()
					.child_alignment(Alignment::new(LX::Center, LY::Center))
					.width(fixed!(50.0))
					.height(fixed!(50.0))
					.end()
					.background_color((0x0E, 0x1A, 0x26, 0x30).into())
					.corner_radius()
					.all(10.0)
					.end()
					.id(c.id("show-password"));

				if c.hovered() {
					d.background_color((0x0E + 20, 0x1A + 20, 0x26 + 20, 0x30).into());
					if rmar.is_mouse_button_down(0) {
						d.background_color((0x0E + 30, 0x1A + 30, 0x26 + 30, 0x30).into());
					}
				}
				d
			},
			|c| {
				let icon = if self.password_input.hide_input {
					&self.eye_icon
				} else {
					&self.eye_off_icon
				};
				c.with(
					Declaration::new()
						.image()
						.data(icon)
						.end()
						.layout()
						.width(fixed!(24.0))
						.height(fixed!(24.0))
						.end(),
					|_| {},
				);
			},
		);
	}
}

// --------- Componente popup

fn desktop_environments_popup<'clay: 'render, 'render>(
	session_manager: &'render SessionManager,
	c: &mut TibsClayScope<'clay, 'render>,
	frame_pool: &FrameAllocator<'render>,
	de_icons: &'render HashMap<KnownDEs, SkiaImageAsset>,
	rmar: &Rustamarine,
) {
	c.with(
		Declaration::new()
			.background_color(hsl_to_rgb(230., 27.6, 10.2).into())
			.corner_radius()
			.all(10.)
			.end()
			.floating()
			.attach_to(FloatingAttachToElement::Parent)
			.attach_points(
				FloatingAttachPointType::LeftTop,
				FloatingAttachPointType::RightTop,
			)
			.offset((10.0, 0.0).into())
			.end()
			.layout()
			.direction(LayoutDirection::TopToBottom)
			.width(fit!(250.0))
			.height(fit!(0.0, 130.0))
			.end()
			.layout()
			.padding(Padding::all(12))
			.child_gap(12)
			.end()
			.id(c.id("desktop-environments-popup")),
		|c| {
			let desktop_environments = session_manager.get_desktop_environments_list();
			c.text(
				"Select a desktop environment",
				TextConfig::new()
					.font_size(14)
					.color((0xFF, 0xFF, 0xFF, 200).into())
					.end(),
			);
			c.with_styling(
				|c| {
					let mut d = Declaration::new();
					d.border()
						.color((0xff, 0xff, 0xff, 20).into())
						.all_directions(1)
						.between_children(1)
						.end()
						.clip(false, true, c.scroll_offset());
					d.layout().width(grow!());
					d.corner_radius().all(10.);
					d
				},
				|c| {
					for (i, de) in desktop_environments.iter().enumerate() {
						c.with_styling(
							|c| {
								let mut d = Declaration::new();
								d.layout()
									.padding(Padding::new(14, 14, 14, 14))
									.child_gap(16)
									.child_alignment(Alignment::new(LX::Left, LY::Center))
									.width(grow!())
									.end()
									.id(c.id_index("desktop-environment", i as u32));
								if c.hovered() {
									d.background_color((0xff, 0xff, 0xff, 0x1f).into());
									if rmar.is_mouse_button_down(0) {
										d.background_color((0xff, 0xff, 0xff, 0x3f).into());
									}
								}

								if i == 0 {
									d.corner_radius().top_left(10.).top_right(10.);
								}
								if i == desktop_environments.len() - 1 {
									d.corner_radius().bottom_left(10.).bottom_right(10.);
								}
								d
							},
							|c| {
								let known_de = KnownDEs::from_de(de);
								c.with(
									Declaration::new()
										.image()
										.data(&de_icons.get(&known_de).unwrap().0)
										.end()
										.layout()
										.width(fixed!(28.0))
										.height(fixed!(28.0))
										.end(),
									|_| {},
								);
								c.text(
									match known_de {
										KnownDEs::KDE => "KDE Plasma",
										KnownDEs::GNOME => "GNOME",
										KnownDEs::Hyprland => "Hyprland",
										KnownDEs::HyprDE => "HyprDE",
										KnownDEs::Unknown => frame_pool
											.alloc(format!("{} (Unknown)", de.name()))
											.as_str(),
									},
									TextConfig::new()
										.color((0xFF, 0xFF, 0xFF).into())
										.font_size(14)
										.end(),
								);
							},
						);
					}
				},
			)
		},
	);
}
