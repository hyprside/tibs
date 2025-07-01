use std::env;
use std::fs::File;
use std::path::PathBuf;

#[cfg(feature = "opengl_loader")]
use gl_generator::*;

fn main() {
	let dst = cmake::Config::new("cpp")
		.define("CMAKE_EXPORT_COMPILE_COMMANDS", "ON")
		.build();

	println!(
		"cargo:rustc-link-search=native={}",
		dst.join("lib").display()
	);
	println!("cargo:rustc-link-lib=static=rustamarine-cpp");
	// Linkar bibliotecas encontradas via pkg-config
	for lib in &[
		"aquamarine",
		"hyprutils",
		"libdrm",
		"gbm",
		"libunwind",
		"xkbcommon",
	] {
		let libs = pkg_config::probe_library(lib).expect(&format!("Failed to find {}", lib));
		for path in libs.link_paths {
			println!("cargo:rustc-link-search=native={}", path.display());
		}
		for libname in libs.libs {
			println!("cargo:rustc-link-lib={}", libname);
		}
	}
	// Recompile if any C++ or header files change
	println!("cargo:rerun-if-changed=cpp");
	// Re-linka bibliotecas do sistema
	println!("cargo:rustc-link-lib=stdc++");
	println!("cargo:rustc-link-lib=backtrace");
	println!("cargo:rustc-link-lib=EGL");
	println!("cargo:rustc-link-lib=GLESv2");
	let target = env::var("CARGO_CFG_TARGET_ARCH").unwrap();

	match target.as_str() {
		"x86_64" => {
			println!("cargo:rustc-link-lib=unwind-x86_64");
		}
		"aarch64" => {
			println!("cargo:rustc-link-lib=unwind-aarch64");
		}
		_ => {
			println!("cargo:rustc-link-lib=unwind"); // fallback genérico
		}
	}

	println!("cargo:rustc-link-lib=unwind-ptrace");

	// Bindgen
	let bindings = bindgen::Builder::default()
		.header("cpp/headers/rustamarine.h")
		.parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
		.generate()
		.expect("Unable to generate bindings");

	let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
	bindings
		.write_to_file(out_path.join("rustamarine_bindings.rs"))
		.expect("Couldn't write bindings!");

	#[cfg(feature = "opengl_loader")]
	{
		let mut file = File::create(out_path.join("opengl_bindings.rs")).unwrap();
		println!("cargo:rerun-if-changed=build.rs");
		Registry::new(Api::Gles2, (3, 2), Profile::Core, Fallbacks::All, [])
			.write_bindings(GlobalGenerator, &mut file)
			.unwrap();
	}
}
