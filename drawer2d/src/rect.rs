#[derive(Clone, Copy, Debug)]
#[repr(C, packed)]
pub struct Rect {
    pub pos: [f32; 2],
    pub size: [f32; 2],
}

impl Rect {
    pub fn contains_point(&self, point: [f32; 2]) -> bool {
        self.pos[0] <= point[0]
            && point[0] <= self.pos[0] + self.size[0]
            && self.pos[1] <= point[1]
            && point[1] <= self.pos[1] + self.size[1]
    }
}
