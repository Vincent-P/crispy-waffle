use super::*;

pub struct Button<'a> {
    pub label: &'a str,
    pub rect: Rect,
}

impl Ui {
    pub fn button(&mut self, drawer: &mut Drawer, button: Button) -> bool {
        let mut result = false;
        let id = self.activation.make_id();

        let (label_run, label_layout) =
            drawer.shape_and_layout_text(&self.theme.face(), button.label);
        let label_size = label_layout.size();

        let button_rect = button.rect;

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
            Rect::center(button_rect, label_size).pos,
            0,
        );

        self.state.add_rect_to_last_container(button_rect);

        result
    }
}
