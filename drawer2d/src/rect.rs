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

    pub fn outset(&self, margin: f32) -> Self {
        Self {
            pos: [self.pos[0] - margin, self.pos[1] - margin],
            size: [self.size[0] + 2.0 * margin, self.size[1] + 2.0 * margin],
        }
    }

    pub fn inset(&self, margin: f32) -> Self {
        self.outset(-margin)
    }

    pub fn center(container: Self, element_size: [f32; 2]) -> Self {
        Self {
            pos: [
                container.pos[0] + (container.size[0] - element_size[0]) / 2.0,
                container.pos[1] + (container.size[1] - element_size[1]) / 2.0,
            ],
            size: element_size,
        }
    }

    pub fn split_top_pixels(&self, height: f32) -> (Self, Self) {
        (
            Self {
                pos: self.pos,
                size: [self.size[0], height],
            },
            Self {
                pos: [self.pos[0], self.pos[1] + height],
                size: [self.size[0], self.size[1] - height],
            },
        )
    }

    pub fn split_left_pixels(&self, width: f32) -> (Self, Self) {
        (
            Self {
                pos: self.pos,
                size: [width, self.size[1]],
            },
            Self {
                pos: [self.pos[0] + width, self.pos[1]],
                size: [self.size[1] - width, self.size[1]],
            },
        )
    }
}
