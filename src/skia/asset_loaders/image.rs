use std::ops::{Deref, DerefMut};

use assets_manager::{loader::Loader, Asset};
use color_eyre::eyre::OptionExt;

pub struct SkiaImageAsset(pub skia_safe::Image);

impl Deref for SkiaImageAsset {
	type Target = skia_safe::Image;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl DerefMut for SkiaImageAsset {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl Asset for SkiaImageAsset {
	const EXTENSIONS: &'static [&'static str] = &["png", "jpg", "jpeg", "webp"];
	type Loader = SkiaImageLoader;
}
impl From<skia_safe::Image> for SkiaImageAsset {
	fn from(value: skia_safe::Image) -> Self {
		Self(value)
	}
}

pub struct SkiaImageLoader;

impl Loader<SkiaImageAsset> for SkiaImageLoader {
	fn load(
		content: std::borrow::Cow<[u8]>,
		_ext: &str,
	) -> Result<SkiaImageAsset, assets_manager::BoxedError> {
		Ok(
			skia_safe::Image::from_encoded(skia_safe::Data::new_copy(&*content))
				.ok_or_eyre("Failed to load image (Invalid contents?)")?
				.into(),
		)
	}
}
