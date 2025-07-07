pub struct FPSCounter {
	start_time: std::time::Instant,
	frame_count: u32,
}
impl FPSCounter {
	pub fn new() -> Self {
		Self {
			start_time: std::time::Instant::now(),
			frame_count: 0,
		}
	}
	pub fn tick(&mut self) -> Option<f32> {
		self.frame_count += 1;
		let elapsed = self.start_time.elapsed().as_secs_f32();
		if elapsed >= 1.0 {
			let fps = self.frame_count as f32 / elapsed;
			self.frame_count = 0;
			self.start_time = std::time::Instant::now();
			Some(fps)
		} else {
			None
		}
	}
}
