use drawer2d::{drawer::*, font::*, rect::*};
use std::rc::Rc;

pub struct Theme {
    button_bg_color: ColorU32,
    button_pressed_bg_color: ColorU32,
    button_hover_bg_color: ColorU32,
    font: Rc<Font>,
    font_size: f32,
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

pub struct Ui {
    pub activation: Activation,
    pub theme: Theme,
    pub inputs: Inputs,
}

pub struct Button<'a> {
    pub label: &'a str,
    pub pos: [f32; 2],
    pub margins: [f32; 2],
}

impl Ui {
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
        }
    }

    pub fn mouse_position(&self) -> [f32; 2] {
        self.inputs.mouse_pos
    }

    pub fn set_mouse_position(&mut self, pos: [f32; 2]) {
        self.inputs.mouse_pos = pos;
    }

    pub fn set_left_mouse_button_pressed(&mut self, pressed: bool) {
        self.inputs.left_mouse_button_pressed = pressed;
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

    fn has_clicked(&self, id: u64) -> bool {
        !self.inputs.left_mouse_button_pressed
            && self.activation.focused == Some(id)
            && self.activation.active == Some(id)
    }

    pub fn button(&mut self, drawer: &mut Drawer, button: Button) -> bool {
        let mut result = false;
        let id = self.activation.make_id();

        let (label_run, label_layout) =
            drawer.shape_and_layout_text(&self.theme.face(), button.label);
        let label_size = label_layout.size();

        let button_rect = Rect {
            pos: button.pos,
            size: [
                label_size[0] + 2.0 * button.margins[0],
                label_size[1] + 2.0 * button.margins[1],
            ],
        };

        if self.inputs.is_hovering(button_rect) {
            self.activation.focused = Some(id);
            if self.activation.active == None && self.inputs.left_mouse_button_pressed {
                self.activation.active = Some(id);
            }
        }

        if self.has_clicked(id) {
            result = true;
        }

        let color = match (self.activation.focused, self.activation.active) {
            (Some(f), Some(a)) if f == id && a == id => self.theme.button_pressed_bg_color,
            (Some(f), _) if f == id => self.theme.button_hover_bg_color,
            _ => self.theme.button_bg_color,
        };

        drawer.draw_colored_rect(button_rect, 0, color);

        drawer.draw_text_run(
            &label_run,
            &label_layout,
            [
                button_rect.pos[0] + button.margins[0],
                button_rect.pos[1] + button.margins[1],
            ],
            0,
        );

        result
    }
}

impl Activation {
    pub fn make_id(&mut self) -> u64 {
        let new_id = self.gen;
        self.gen += 1;
        new_id
    }
}

impl Inputs {
    pub fn is_hovering(&self, rect: Rect) -> bool {
        rect.contains_point(self.mouse_pos)
    }
}

impl Theme {
    pub fn face(&self) -> Face {
        Face::from_font(&self.font, self.font_size)
    }
}
