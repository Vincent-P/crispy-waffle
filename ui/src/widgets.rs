use super::*;

pub struct Button<'a> {
    pub label: &'a str,
    pub rect: Rect,
    pub enabled: bool,
}

impl Ui {
    pub fn button(&mut self, drawer: &mut Drawer, button: Button) -> bool {
        let mut result = false;
        let id = self.activation.make_id();

        let button_rect = button.rect;

        // -- Interactions

        if button.enabled {
            if self.inputs.is_hovering(button_rect) {
                self.activation.focused = Some(id);
                if self.activation.active == None && self.inputs.left_mouse_button_pressed {
                    self.activation.active = Some(id);
                }
            }

            if self.has_clicked(id) {
                result = true;
            }
        }

        // -- Drawing
        let em = self.theme.font_size;

        let bg_color = match (self.activation.focused, self.activation.active) {
            (Some(f), Some(a)) if f == id && a == id => self.theme.button_pressed_bg_color,
            (Some(f), _) if f == id => self.theme.button_hover_bg_color,
            _ => self.theme.button_bg_color,
        };

        let outline_color = match (self.activation.focused, self.activation.active) {
            (Some(f), Some(a)) if f == id && a == id => self.theme.button_pressed_bg_outline_color,
            (Some(f), _) if f == id => self.theme.button_hover_bg_outline_color,
            _ => self.theme.button_bg_outline_color,
        };

        drawer.draw_colored_rect(
            ColoredRect::new(button_rect)
                .color(outline_color)
                .border_radius(0.33 * em),
        );

        drawer.draw_colored_rect(
            ColoredRect::new(button_rect.inset(self.theme.button_outline_width))
                .color(bg_color)
                .border_radius(0.33 * em),
        );

        let (label_run, label_layout) =
            drawer.shape_and_layout_text(&self.theme.face(), button.label);
        let label_size = label_layout.size();

        let fg_color = match (self.activation.focused, self.activation.active) {
            (Some(f), Some(a)) if f == id && a == id => self.theme.button_pressed_fg_color,
            (Some(f), _) if f == id => self.theme.button_hover_fg_color,
            _ => self.theme.button_fg_color,
        };
        drawer.draw_text_run(
            &label_run,
            &label_layout,
            Rect::center(button_rect, label_size).pos,
            0,
            fg_color,
        );

        if !button.enabled {
            drawer.draw_colored_rect(
                ColoredRect::new(button_rect)
                    .color(ColorU32::from_f32(0.0, 0.0, 0.0, 0.25))
                    .border_radius(0.33 * em),
            );
        }

        self.state.add_rect_to_last_container(button_rect);

        result
    }
}

// I hate Rust.
impl<'a> Button<'a> {
    pub fn with_label(label: &'a str) -> Self {
        Self {
            label,
            rect: Rect::default(),
            enabled: true,
        }
    }

    pub fn rect(mut self, rect: Rect) -> Self {
        self.rect = rect;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

pub struct Splitter {
    pub rect: Rect,
}

impl Ui {
    pub fn splitter_x(&mut self, drawer: &mut Drawer, splitter: Splitter, value: &mut f32) -> bool {
        let mut result = false;
        let id = self.activation.make_id();

        let input_width = 10.0;
        let input_rect = Rect {
            pos: [
                splitter.rect.pos[0] + (*value) * splitter.rect.size[0] - 0.5 * input_width,
                splitter.rect.pos[1],
            ],
            size: [input_width, splitter.rect.size[1]],
        };

        // -- Interactions

        if self.inputs.is_hovering(input_rect) {
            self.activation.focused = Some(id);
            if self.activation.active == None && self.inputs.left_mouse_button_pressed {
                self.activation.active = Some(id);
            }
        }

        if self.inputs.left_mouse_button_pressed && self.activation.active == Some(id) {
            *value = (self.inputs.mouse_pos[0] - splitter.rect.pos[0]) / splitter.rect.size[0];
            result = true;
        }

        // -- Drawing

        let color = match (self.activation.focused, self.activation.active) {
            (Some(f), Some(a)) if f == id && a == id => self.theme.button_pressed_bg_color,
            (Some(f), _) if f == id => self.theme.button_hover_bg_color,
            _ => self.theme.button_bg_color,
        };

        drawer.draw_colored_rect(ColoredRect::new(input_rect).color(color));

        self.state.add_rect_to_last_container(input_rect);

        result
    }

    pub fn splitter_y(&mut self, drawer: &mut Drawer, splitter: Splitter, value: &mut f32) -> bool {
        let mut result = false;
        let id = self.activation.make_id();

        let input_width = 10.0;
        let input_rect = Rect {
            pos: [
                splitter.rect.pos[0],
                splitter.rect.pos[1] + (*value) * splitter.rect.size[1] - 0.5 * input_width,
            ],
            size: [splitter.rect.size[0], input_width],
        };

        // -- Interactions

        if self.inputs.is_hovering(input_rect) {
            self.activation.focused = Some(id);
            if self.activation.active == None && self.inputs.left_mouse_button_pressed {
                self.activation.active = Some(id);
            }
        }

        if self.inputs.left_mouse_button_pressed && self.activation.active == Some(id) {
            *value = (self.inputs.mouse_pos[1] - splitter.rect.pos[1]) / splitter.rect.size[1];
            result = true;
        }

        // -- Drawing

        let color = match (self.activation.focused, self.activation.active) {
            (Some(f), Some(a)) if f == id && a == id => self.theme.button_pressed_bg_color,
            (Some(f), _) if f == id => self.theme.button_hover_bg_color,
            _ => self.theme.button_bg_color,
        };

        drawer.draw_colored_rect(ColoredRect::new(input_rect).color(color));

        self.state.add_rect_to_last_container(input_rect);

        result
    }
}
