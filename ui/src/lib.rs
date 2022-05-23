use drawer2d::{drawer::*, rect::*};

pub struct UiTheme {
    button_bg_color: ColorU32,
    button_pressed_bg_color: ColorU32,
    button_hover_bg_color: ColorU32,
}

pub struct UiInputs {
    mouse_pos: [f32; 2],
    left_mouse_button_pressed: bool,
}

pub struct UiState {
    focused: u64,
    active: u64,
    gen: u64,
    theme: UiTheme,
    inputs: UiInputs,
}

pub struct UiButton<'a> {
    label: &'a str,
    rect: Rect,
}

impl UiState {
    fn make_id(&mut self) -> u64 {
        let new_id = self.gen;
        self.gen += 1;
        new_id
    }

    fn is_hovering(&self, rect: Rect) -> bool {
        rect.contains_point(self.inputs.mouse_pos)
    }

    pub fn button(&mut self, drawer: &mut Drawer, button: UiButton) -> bool {
        let mut result = false;
        let id = self.make_id();

        if self.is_hovering(button.rect) {
            self.focused = id;
            if self.active == 0 && self.inputs.left_mouse_button_pressed {
                self.active = id;
            }
        }

        if !self.inputs.left_mouse_button_pressed && self.focused == id && self.active == id {
            result = true;
        }

        let color = match (self.focused, self.active) {
            (f, a) if f == id && a == id => self.theme.button_pressed_bg_color,
            (f, _) if f == id => self.theme.button_hover_bg_color,
            _ => self.theme.button_bg_color,
        };

        drawer.draw_colored_rect(button.rect, 0, color);

        result
    }
}
