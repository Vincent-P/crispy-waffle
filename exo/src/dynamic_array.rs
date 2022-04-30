use std::mem::MaybeUninit;
use std::ops::{Deref, Index, IndexMut};

#[derive(Clone)]
pub struct DynamicArray<T, const CAPACITY: usize> {
    array: [T; CAPACITY],
    size: usize,
}

impl<T, const CAPACITY: usize> DynamicArray<T, CAPACITY> {
    #[allow(clippy::uninit_assumed_init)]
    pub fn new() -> Self {
        Self {
            array: unsafe { MaybeUninit::uninit().assume_init() },
            size: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        assert!(self.size < CAPACITY);
        self.array[self.size] = value;
        self.size += 1;
    }

    pub fn clear(&mut self) {
        for i in 0..self.size {
            unsafe {
                let ptr = &mut self.array[i] as *mut _;
                std::ptr::drop_in_place(ptr);
            }
        }
        self.size = 0;
    }

    pub fn back(&self) -> &T {
        assert!(self.size > 0);
        &self.array[self.size - 1]
    }

    pub fn back_mut(&mut self) -> &mut T {
        assert!(self.size > 0);
        &mut self.array[self.size - 1]
    }

    /// Return a slice containing all elements of the vector.
    pub fn as_slice(&self) -> &[T] {
        let len = self.len();
        unsafe { std::slice::from_raw_parts(self.as_ptr(), len) }
    }

    pub fn resize(&mut self, new_length: usize, value: T)
    where
        T: Copy,
    {
        assert!(new_length < CAPACITY);
        for i in self.size..new_length {
            self.array[i] = value;
        }
        self.size = new_length;
    }
}

impl<T, const CAPACITY: usize> Default for DynamicArray<T, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

// Index operator
impl<T, const CAPACITY: usize> Index<usize> for DynamicArray<T, CAPACITY> {
    type Output = T;
    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.size);
        &self.array[index]
    }
}

// Index operator returning mutable ref
impl<T, const CAPACITY: usize> IndexMut<usize> for DynamicArray<T, CAPACITY> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.size);
        &mut self.array[index]
    }
}

// Convert to slice with &dynarray
impl<T, const CAPACITY: usize> Deref for DynamicArray<T, CAPACITY> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.array[0..self.size]
    }
}

// Const iterator from &dynarray
impl<'a, T, const CAPACITY: usize> IntoIterator for &'a DynamicArray<T, CAPACITY> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> std::slice::Iter<'a, T> {
        self.iter()
    }
}

// Constructor from slice
impl<T: Copy, const CAPACITY: usize> From<&[T]> for DynamicArray<T, CAPACITY> {
    fn from(slice: &[T]) -> Self {
        assert!(slice.len() < CAPACITY);
        let mut dynarray = Self::new();
        dynarray.array[..slice.len()].copy_from_slice(slice);
        dynarray.size = slice.len();
        dynarray
    }
}

// Constructor from array
impl<T: Copy, const N: usize, const CAPACITY: usize> From<[T; N]> for DynamicArray<T, CAPACITY> {
    fn from(slice: [T; N]) -> Self {
        assert!(N < CAPACITY);
        let mut dynarray = Self::new();
        dynarray.array[..slice.len()].copy_from_slice(&slice);
        dynarray.size = slice.len();
        dynarray
    }
}
