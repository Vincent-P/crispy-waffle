use bytes::{Buf, BufMut};

pub trait Serializable {
    const VERSION: u32 = 0;
    fn load(&mut self, serializer: &mut Serializer);
    fn write(&self, serializer: &mut Serializer);
}

trait Source: BufMut + Buf {}

pub struct Serializer<'a> {
    source: &'a mut dyn Source,
    version: usize,
}

impl Serializer<'_> {
    pub fn version(&self) -> usize {
        self.version
    }

    pub fn load<T: Serializable + Sized>(&mut self, data: &mut T) {
        data.load(self);
    }

    pub fn load_slice<T: Serializable>(&mut self, data: &mut [T]) {
        for element in data {
            element.load(self);
        }
    }

    pub fn write<T: Serializable + Sized>(&mut self, data: &T) {
        data.write(self);
    }

    pub fn write_slice<T: Serializable>(&mut self, data: &[T]) {
        for element in data {
            element.write(self);
        }
    }

    pub fn load_bytes(&mut self, dst: &mut [u8]) {
        self.source.copy_to_slice(dst)
    }

    pub fn write_bytes(&mut self, src: &[u8]) {
        self.source.put_slice(src);
    }
}

impl Serializable for f32 {
    fn load(&mut self, serializer: &mut Serializer) {
        let mut bytes: [u8; 4] = [0, 0, 0, 0];
        serializer.load_bytes(&mut bytes);
        *self = Self::from_le_bytes(bytes);
    }

    fn write(&self, serializer: &mut Serializer) {
        let bytes = self.to_le_bytes();
        serializer.write_bytes(&bytes);
    }
}

impl Serializable for u128 {
    fn load(&mut self, serializer: &mut Serializer) {
        let mut bytes: [u8; 16] = [0; 16];
        serializer.load_bytes(&mut bytes);
        *self = Self::from_le_bytes(bytes);
    }

    fn write(&self, serializer: &mut Serializer) {
        let bytes = self.to_le_bytes();
        serializer.write_bytes(&bytes);
    }
}

impl Serializable for uuid::Uuid {
    fn load(&mut self, serializer: &mut Serializer) {
        let mut bytes: [u8; 16] = [0; 16];
        serializer.load_bytes(&mut bytes);
        *self = Self::from_bytes(bytes);
    }

    fn write(&self, serializer: &mut Serializer) {
        let bytes = self.into_bytes();
        serializer.write_bytes(&bytes);
    }
}
