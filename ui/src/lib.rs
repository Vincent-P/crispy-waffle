use drawer2d::{drawer::*, font::*, rect::*};
use std::rc::Rc;

mod widgets;
pub use widgets::*;

pub struct Theme {
    button_bg_color: ColorU32,
    button_pressed_bg_color: ColorU32,
    button_hover_bg_color: ColorU32,
    font: Rc<Font>,
    pub font_size: f32,
}

pub struct Inputs {
    mouse_pos: [f32; 2],
    left_mouse_button_pressed: bool,
}

pub struct Activation {
    focused: Option<u64>,
    active: Option<u64>,
    gen: u64,
}

pub struct State {
    container_stack: Vec<Container>,
    i_container_stack: usize,
}

pub struct Ui {
    pub activation: Activation,
    pub theme: Theme,
    pub inputs: Inputs,
    state: State,
}

#[derive(Clone, Copy, Debug)]
pub struct Container {
    min_pos: [f32; 2],
    max_pos: [f32; 2],
}

impl Ui {
    // -- Main UI API
    pub fn new(font: Rc<Font>, font_size: f32) -> Self {
        Self {
            activation: Activation {
                focused: None,
                active: None,
                gen: 0,
            },
            theme: Theme {
                button_bg_color: ColorU32::from_f32(0.43, 0.23, 0.12, 1.0),
                button_pressed_bg_color: ColorU32::from_f32(0.13, 0.23, 0.42, 1.0),
                button_hover_bg_color: ColorU32::from_f32(0.23, 0.43, 0.12, 1.0),
                font,
                font_size,
            },
            inputs: Inputs {
                mouse_pos: [0.0, 0.0],
                left_mouse_button_pressed: false,
            },
            state: State {
                container_stack: vec![Container::default()],
                i_container_stack: 0,
            },
        }
    }

    pub fn new_frame(&mut self) {
        self.activation.gen = 0;
        self.activation.focused = None;
    }

    pub fn end_frame(&mut self) {
        if !self.inputs.left_mouse_button_pressed {
            self.activation.active = None;
        }
    }

    // -- Helpers
    pub fn mouse_position(&self) -> [f32; 2] {
        self.inputs.mouse_pos
    }

    pub fn set_mouse_position(&mut self, pos: [f32; 2]) {
        self.inputs.mouse_pos = pos;
    }

    pub fn set_left_mouse_button_pressed(&mut self, pressed: bool) {
        self.inputs.left_mouse_button_pressed = pressed;
    }

    // Returns the size of an em in pixels
    pub fn em(&self) -> f32 {
        self.theme.font_size
    }

    // -- Widgets API
    fn has_clicked(&self, id: u64) -> bool {
        !self.inputs.left_mouse_button_pressed
            && self.activation.focused == Some(id)
            && self.activation.active == Some(id)
    }

    pub fn begin_container(&mut self) -> Container {
        assert!(self.state.i_container_stack <= self.state.container_stack.len());

        self.state.i_container_stack += 1;

        if self.state.i_container_stack == self.state.container_stack.len() {
            self.state.container_stack.push(Container::default());
        }

        let container = &mut self.state.container_stack[self.state.i_container_stack];

        let old_container = *container;
        *container = Container::default();

        old_container
    }

    pub fn end_container(&mut self) {
        let ended_container_rect = self.state.container_stack[self.state.i_container_stack].rect();
        self.state.i_container_stack -= 1;
        assert!(self.state.i_container_stack < self.state.container_stack.len());
        self.state.add_rect_to_last_container(ended_container_rect);
    }
}

impl Activation {
    // -- Widgets API
    pub fn make_id(&mut self) -> u64 {
        let new_id = self.gen;
        self.gen += 1;
        new_id
    }
}

impl Inputs {
    // -- Widgets API
    pub fn is_hovering(&self, rect: Rect) -> bool {
        rect.contains_point(self.mouse_pos)
    }
}

impl Theme {
    pub fn face(&self) -> Face {
        Face::from_font(&self.font, self.font_size)
    }
}

impl State {
    // Add a rect to the latest container
    pub fn add_rect_to_last_container(&mut self, rect: Rect) {
        self.container_stack[self.i_container_stack].add_rect(rect);
    }
}

impl Container {
    // Add a new rectangle inside a container
    pub fn add_rect(&mut self, rect: Rect) {
        let rect_min_pos = rect.pos;
        let rect_max_pos = [rect.pos[0] + rect.size[0], rect.pos[1] + rect.size[1]];

        self.min_pos = [
            self.min_pos[0].min(rect_min_pos[0]),
            self.min_pos[1].min(rect_min_pos[1]),
        ];

        self.max_pos = [
            self.max_pos[0].max(rect_max_pos[0]),
            self.max_pos[1].max(rect_max_pos[1]),
        ];
    }

    // Returns the size of the container as a Rect
    pub fn rect(&self) -> Rect {
        Rect {
            pos: self.min_pos,
            size: [
                self.max_pos[0] - self.min_pos[0],
                self.max_pos[1] - self.min_pos[1],
            ],
        }
    }
}

impl Default for Container {
    fn default() -> Self {
        Self {
            min_pos: [f32::INFINITY, f32::INFINITY],
            max_pos: [f32::NEG_INFINITY, f32::NEG_INFINITY],
        }
    }
}
