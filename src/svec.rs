use core::{
	fmt::Debug,
	mem::MaybeUninit,
	ops::{Index, IndexMut},
};

pub struct SVec<T, const N: usize> {
	inner: [MaybeUninit<T>; N],
	length: usize,
}

impl<T: Debug, const N: usize> Debug for SVec<T, N> {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		write!(f, "{:?}", self.get_slice())
	}
}

impl<T, const N: usize> SVec<T, N> {
	pub const fn new() -> Self {
		Self {
			inner: MaybeUninit::uninit_array(),
			length: 0,
		}
	}
}

impl<T, const N: usize> SVec<T, N> {
	pub fn len(&self) -> usize {
		self.length
	}

	pub fn capacity(&self) -> usize {
		N
	}

	pub fn push(&mut self, value: T) {
		self.inner[self.length] = MaybeUninit::new(value);
		self.length += 1;
	}

	pub fn pop(&mut self) -> Option<T> {
		if self.length > 0 {
			self.length -= 1;
			Some(unsafe { self.inner[self.length].assume_init_read() })
		} else {
			None
		}
	}

	pub fn remove(&mut self, index: usize) -> T {
		if index >= self.length {
			panic!("Index out of bounds");
		}

		unsafe {
			let t = core::ptr::read(&self.inner[index]).assume_init();
			if index + 1 < self.length {
				core::ptr::copy(
					&self.inner[index + 1],
					&mut self.inner[index],
					self.length - index - 1,
				);
			}
			self.length -= 1;
			t
		}
	}

	pub fn get_slice(&self) -> &[T] {
		unsafe { core::mem::transmute(&self.inner[..self.length]) }
	}

	pub fn get_slice_mut(&mut self) -> &mut [T] {
		unsafe { core::mem::transmute(&mut self.inner[..self.length]) }
	}
}

impl<T, const N: usize> Index<usize> for SVec<T, N> {
	type Output = T;

	fn index(&self, index: usize) -> &Self::Output {
		if index >= self.length {
			panic!(
				"Index out of bounds; index was {}, max was {}",
				index,
				self.length - 1
			);
		} else {
			unsafe { self.inner[index].assume_init_ref() }
		}
	}
}

impl<T, const N: usize> IndexMut<usize> for SVec<T, N> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		if index >= self.length {
			panic!(
				"Index out of bounds; index was {}, max was {}",
				index,
				self.length - 1
			);
		} else {
			unsafe { self.inner[index].assume_init_mut() }
		}
	}
}

impl<T: Clone, const N: usize> Clone for SVec<T, N> {
	fn clone(&self) -> Self {
		let mut ret = SVec::new();
		for i in self.get_slice() {
			ret.push(i.clone());
		}
		ret
	}
}

impl<T, const N: usize> Drop for SVec<T, N> {
	fn drop(&mut self) {
		for item in self.get_slice_mut() {
			core::mem::drop(item);
		}
	}
}

impl<const N: usize> SVec<u8, N> {
	/// Get the `str` value of this `SVec`, interpretted as UTF-8 meaning compatibility with ASCII.
	pub fn to_str(&self) -> &str {
		if self.length == 0 {
			return "";
		}
		core::str::from_utf8(self.get_slice()).unwrap()
	}
}

#[rustfmt::skip]
impl<const N: usize> SVec<char, N> where [(); N * 4]: {
	/// Converts the `char`s into `u8`s one-by-one.
	pub fn to_u8(&self) -> SVec<u8, { N * 4 }> {
		let slice = self.get_slice();
		let mut ret = SVec::new();

		for c in slice {
			let mut buf = [0; 4];
			let s = c.encode_utf8(&mut buf);
			for b in s.bytes() {
				ret.push(b);
			}
		}
		ret
	}
}
