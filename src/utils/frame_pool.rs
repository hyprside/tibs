use std::cell::UnsafeCell;
use std::marker::PhantomData;

pub struct FramePool<'p> {
	storage: UnsafeCell<Vec<Box<dyn std::any::Any>>>,
	_marker: PhantomData<&'p ()>,
}

impl<'p> FramePool<'p> {
	pub fn new() -> Self {
		Self {
			storage: UnsafeCell::new(Vec::new()),
			_marker: PhantomData,
		}
	}

	pub fn begin_alloc<'render>(&'render self) -> FrameAllocator<'render>
	where
		'p: 'render,
	{
		FrameAllocator { pool: self }
	}

	pub fn reset(&mut self) {
		unsafe { &mut *self.storage.get() }.clear();
	}
}
impl Drop for FramePool<'_> {
	fn drop(&mut self) {
		self.reset();
	}
}
pub struct FrameAllocator<'render> {
	pool: &'render FramePool<'render>,
}

impl<'render> FrameAllocator<'render> {
	pub fn alloc<T: 'static>(&self, value: T) -> &'render mut T {
		let storage = unsafe { &mut *self.pool.storage.get() };
		storage.push(Box::new(value));
		storage.last_mut().unwrap().downcast_mut::<T>().unwrap()
	}
}
#[macro_export]
macro_rules! frame_alloc_format {
    ($frame_alloc:expr, $($arg:tt)*) => {
        $frame_alloc.alloc(format!($($arg)*)).as_str()
    }
}
#[macro_export]
macro_rules! format_id {
    ($c:expr, $frame_alloc:expr, $($arg:tt)*) => {
        $c.id($frame_alloc.alloc(format!($($arg)*)).as_str())
    }
}

// ==============================
// ======== TESTES UNITÁRIOS =====
// ==============================
#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn basic_allocation() {
		let mut pool = FramePool::new();
		{
			let alloc = pool.begin_alloc();
			let x = alloc.alloc(42);
			let y = alloc.alloc("hello".to_string());

			assert_eq!(*x, 42);
			assert_eq!(y, "hello");
		}
		pool.reset();
		assert!(unsafe { &*pool.storage.get() }.is_empty());
	}

	#[test]
	fn multiple_allocations_same_frame() {
		let mut pool = FramePool::new();
		{
			let alloc = pool.begin_alloc();
			let a = alloc.alloc(1);
			let b = alloc.alloc(2);
			let c = alloc.alloc(3);

			assert_eq!(*a + *b + *c, 6);
		}
		pool.reset();
		assert!(unsafe { &*pool.storage.get() }.is_empty());
	}

	#[test]
	fn allocator_scope_prevents_reset() {
		let mut pool = FramePool::new();
		let alloc = pool.begin_alloc();
		let _x = alloc.alloc(10);

		// pool.reset(); // ERRO: borrow checker impede chamar reset enquanto allocator existir

		drop(alloc);
		pool.reset();
		assert!(unsafe { &*pool.storage.get() }.is_empty());
	}
}
