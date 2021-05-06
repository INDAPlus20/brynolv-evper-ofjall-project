use core::{fmt::Write, mem::MaybeUninit, ops::{Index, IndexMut}};



pub struct SVec<T, const N: usize> {
    inner: [MaybeUninit<T>; N],
    length: usize,
}

impl<T, const N: usize> SVec<T, N> {
    pub const fn new() -> Self {
        Self {
            inner: MaybeUninit::uninit_array(),
            length: 0,
        }
    }

}

impl<T: Copy, const N: usize> SVec<T, N> {
    pub fn with_length(val: T, length: usize) -> Self {
        assert!(length <= N);
        let mut s = Self::new();
        for _ in 0..length {
            s.push(val);
        }
        s
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
                core::ptr::copy(&self.inner[index + 1], &mut self.inner[index], self.length - index - 1);
            }
            self.length -= 1;
            t
        }
    }

    pub fn insert(&mut self, index: usize, value: T) {
        if index > self.len() {
            panic!("Index out of bounds")
        } else if index == self.len() {
            self.push(value);
        } else {
            unsafe {
                core::ptr::copy(&self.inner[index], &mut self.inner[index + 1], self.len() - index);
            }
            self.inner[index] = MaybeUninit::new(value);
            self.length += 1;
        }
    }

    pub fn clear(&mut self) {
        while self.length > 0 {
            self.length -= 1;
            unsafe {
                self.inner[self.length].assume_init_drop();
            }
        }
    }

    pub fn clear_without_drop(&mut self) {
        self.length = 0;
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
                "Index out of bounds; index was {}, length was {}",
                index,
                self.length
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
                "Index out of bounds; index was {}, length was {}",
                index,
                self.length
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

impl<const N: usize> Write for SVec<char, N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            self.push(c);
        }
        Ok(())
    }
}

impl<const N: usize> Write for SVec<u8, N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            self.push(b);
        }
        Ok(())
    }
}
