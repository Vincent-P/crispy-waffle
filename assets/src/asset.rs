use crate::serialization::Serializable;
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
