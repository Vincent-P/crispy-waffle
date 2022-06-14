#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SplitDirection {
    Top,
    Bottom,
    Left,
    Right,
}

impl SplitDirection {
    pub fn is_horizontal(self) -> bool {
        self == SplitDirection::Left || self == SplitDirection::Right
    }
    pub fn is_vertical(self) -> bool {
        self == SplitDirection::Top || self == SplitDirection::Bottom
    }
}

pub struct RectSplit<'a> {
    pub rect: &'a mut Rect,
    pub direction: SplitDirection,
}

impl RectSplit<'_> {
    pub fn split(&mut self, value: f32) -> Rect {
        self.rect.split(self.direction, value)
    }
}

pub fn rectsplit(rect: &mut Rect, direction: SplitDirection) -> RectSplit {
    RectSplit { rect, direction }
}

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

    // -- Positioning

    pub fn offset(&self, d: [f32; 2]) -> Self {
        Self {
            pos: [self.pos[0] + d[0], self.pos[1] + d[1]],
            size: self.size,
        }
    }

    pub fn outset(&self, margin: f32) -> Self {
        Self {
            pos: [self.pos[0] - margin, self.pos[1] - margin],
            size: [self.size[0] + 2.0 * margin, self.size[1] + 2.0 * margin],
        }
    }

    pub fn margins(&self, m: [f32; 2]) -> Self {
        Self {
            pos: [self.pos[0] + m[0], self.pos[1] + m[1]],
            size: [self.size[0] - 2.0 * m[0], self.size[1] - 2.0 * m[1]],
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

    // -- Splitting
    pub fn split(&mut self, direction: SplitDirection, value: f32) -> Self {
        match direction {
            SplitDirection::Top => self.split_top(value),
            SplitDirection::Bottom => self.split_bottom(value),
            SplitDirection::Left => self.split_left(value),
            SplitDirection::Right => self.split_right(value),
        }
    }

    pub fn split_top(&mut self, height: f32) -> Self {
        let top = Self {
            pos: self.pos,
            size: [self.size[0], height],
        };
        let bottom = Self {
            pos: [self.pos[0], self.pos[1] + top.size[1]],
            size: [self.size[0], self.size[1] - top.size[1]],
        };

        *self = bottom;
        top
    }

    pub fn split_bottom(&mut self, height: f32) -> Self {
        let top = Self {
            pos: self.pos,
            size: [self.size[0], self.size[1] - height],
        };
        let bottom = Self {
            pos: [self.pos[0], self.pos[1] + top.size[1]],
            size: [self.size[0], height],
        };

        *self = top;
        bottom
    }

    pub fn split_left(&mut self, width: f32) -> Self {
        let left = Self {
            pos: self.pos,
            size: [width, self.size[1]],
        };
        let right = Self {
            pos: [self.pos[0] + left.size[0], self.pos[1]],
            size: [self.size[0] - left.size[0], self.size[1]],
        };

        *self = right;
        left
    }

    pub fn split_right(&mut self, width: f32) -> Self {
        let left = Self {
            pos: self.pos,
            size: [self.size[0] - width, self.size[1]],
        };
        let right = Self {
            pos: [self.pos[0] + left.size[0], self.pos[1]],
            size: [width, self.size[1]],
        };

        *self = left;
        right
    }

    // Docking

    pub fn split_horizontal_ratio(&self, ratio: f32) -> (Self, Self) {
        let top = Self {
            pos: [self.pos[0], self.pos[1]],
            size: [self.size[0], self.size[1] * ratio],
        };

        let bottom = Self {
            pos: [self.pos[0], self.pos[1] + top.size[1]],
            size: [self.size[0], self.size[1] * (1.0 - ratio)],
        };

        (top, bottom)
    }

    pub fn split_vertical_ratio(&self, ratio: f32) -> (Self, Self) {
        let left = Self {
            pos: [self.pos[0], self.pos[1]],
            size: [self.size[0] * ratio, self.size[1]],
        };

        let right = Self {
            pos: [self.pos[0] + left.size[0], self.pos[1]],
            size: [self.size[0] * (1.0 - ratio), self.size[1]],
        };

        (left, right)
    }
}

impl Default for Rect {
    fn default() -> Self {
        Self {
            pos: [0.0, 0.0],
            size: [0.0, 0.0],
        }
    }
}
