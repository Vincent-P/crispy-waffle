use crate::serialization::*;
use uuid::Uuid;

pub type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

pub trait Importer<T> {
    const MAGIC_NUMBER: &'static str = "";
    const FILE_EXTENSIONS: &'static [&'static str] = &[""];
    fn import(&self, data: &[u8]) -> Result<T, BoxedError>;
}

pub struct Asset {
    pub uuid: Uuid,
    pub dependencies: Vec<Uuid>,
    pub hash: u128,
}

impl Serializable for Asset {
    const VERSION: u32 = 1;

    fn load(&mut self, serializer: &mut Serializer) {
        serializer.load(&mut self.uuid);
        serializer.load_slice(self.dependencies.as_mut_slice());
        serializer.load(&mut self.hash);
    }

    fn write(&self, serializer: &mut Serializer) {
        serializer.write(&self.uuid);
        serializer.write_slice(&self.dependencies);
        serializer.write(&self.hash);
    }
}
