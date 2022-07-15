use crate::asset::Asset;
use crate::serialization::*;

struct Material {
    asset: Asset,
    albedo: [f32; 3],
}

impl Serializable for Material {
    const VERSION: u32 = 1;

    fn load(&mut self, serializer: &mut Serializer) {
        if serializer.version() >= 1 {
            serializer.load_slice(&mut self.albedo);
        }
    }

    fn write(&self, serializer: &mut Serializer) {
        serializer.write_slice(&self.albedo);
    }
}
