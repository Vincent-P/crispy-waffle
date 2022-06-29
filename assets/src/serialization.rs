pub trait Serializable {
    fn load(&mut self, serializer: &Serializer);
    fn write(&self, serializer: &mut Serializer);
}

pub struct Serializer {}
