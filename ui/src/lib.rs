use drawer2d::{drawer::*, font::*, rect::*};
use std::rc::Rc;

pub struct UiTheme<'a> {
    button_bg_color: ColorU32,
    button_pressed_bg_color: ColorU32,
    button_hover_bg_color: ColorU32,
    ui_face: Rc<Face<'a>>,
}

pub struct UiInputs {
    mouse_pos: [f32; 2],
    left_mouse_button_pressed: bool,
}

pub struct UiState<'a> {
    focused: Option<u64>,
    active: Option<u64>,
    gen: u64,
    theme: UiTheme<'a>,
    inputs: UiInputs,
}

pub struct UiButton<'a> {
    pub label: &'a str,
    pub rect: Rect,
}

impl<'a> UiState<'a> {
    pub fn new(ui_face: Rc<Face<'a>>) -> Self {
        Self {
            focused: None,
            active: None,
            gen: 0,
            theme: UiTheme {
                button_bg_color: ColorU32(0xFF0000FF),
                button_pressed_bg_color: ColorU32(0xFF00FF00),
                button_hover_bg_color: ColorU32(0xFFFF0000),
                ui_face,
            },
            inputs: UiInputs {
                mouse_pos: [0.0, 0.0],
                left_mouse_button_pressed: false,
            },
        }
    }

    fn make_id(&mut self) -> u64 {
        let new_id = self.gen;
        self.gen += 1;
        new_id
    }

    fn is_hovering(&self, rect: Rect) -> bool {
        rect.contains_point(self.inputs.mouse_pos)
    }

    pub fn set_mouse_position(&mut self, pos: [f32; 2]) {
        self.inputs.mouse_pos = pos;
    }

    pub fn set_left_mouse_button_pressed(&mut self, pressed: bool) {
        self.inputs.left_mouse_button_pressed = pressed;
    }

    pub fn new_frame(&mut self) {
        self.gen = 0;
        self.focused = None;
    }

    pub fn end_frame(&mut self) {
        if !self.inputs.left_mouse_button_pressed {
            self.active = None;
        }
    }

    pub fn button(&mut self, drawer: &mut Drawer, button: UiButton) -> bool {
        let mut result = false;
        let id = self.make_id();

        if self.is_hovering(button.rect) {
            self.focused = Some(id);
            if self.active == None && self.inputs.left_mouse_button_pressed {
                self.active = Some(id);
            }
        }

        if !self.inputs.left_mouse_button_pressed
            && self.focused == Some(id)
            && self.active == Some(id)
        {
            result = true;
        }

        let color = match (self.focused, self.active) {
            (Some(f), Some(a)) if f == id && a == id => self.theme.button_pressed_bg_color,
            (Some(f), _) if f == id => self.theme.button_hover_bg_color,
            _ => self.theme.button_bg_color,
        };

        drawer.draw_colored_rect(button.rect, 0, color);

        drawer.draw_label(&self.theme.ui_face, button.label, button.rect, 0);

        result
    }
}
