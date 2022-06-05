use drawer2d::{drawer::*, rect::*};
use exo::pool::*;

struct DockingUi {
    em_size: f32,
    active_tab: Option<usize>,
}

pub struct Docking {
    areas: Pool<Area>,
    root: Handle<Area>,
    tabviews: Vec<TabView>,
    ui: DockingUi,
}

enum Direction {
    None,
    Horizontal,
    Vertical,
}

struct Area {
    direction: Direction,
    children: Vec<Handle<Area>>,
    children_size: Vec<f32>,
    selected: Option<usize>,
    tabviews: Vec<usize>,
    rect: Rect,
}

struct TabView {
    title: String,
    area: Handle<Area>,
}

impl Docking {
    pub fn new() -> Self {
        let mut docking = Self {
            areas: Pool::new(),
            root: Handle::default(),
            tabviews: Vec::new(),
            ui: DockingUi {
                em_size: 0.0,
                active_tab: None,
            },
        };

        docking.root = docking.areas.add(Area {
            direction: Direction::None,
            children: Vec::new(),
            children_size: Vec::new(),
            selected: None,
            tabviews: Vec::new(),
            rect: Rect {
                pos: [0.0, 0.0],
                size: [0.0, 0.0],
            },
        });

        docking
    }

    // Immediate mode tab rendering, returns the drawing area if the tab is visible
    pub fn tab_view(&mut self, tab_name: &str) -> Option<Rect> {
        let i_tab_view = self
            .tabviews
            .iter()
            .position(|tabview| tabview.title == tab_name)
            .unwrap_or_else(|| {
                self.tabviews.push(TabView {
                    title: String::from(tab_name),
                    area: self.root,
                });

                let root_area = self.areas.get_mut(self.root);

                root_area.tabviews.push(self.tabviews.len() - 1);
                self.tabviews.len() - 1
            });

        let tabview = &self.tabviews[i_tab_view];
        let area = self.areas.get(tabview.area);
        if area.selected.is_some() && area.selected.unwrap() == i_tab_view {
            let (_tabwell_rect, content_rect) = area.rects(self.ui.em_size);
            Some(content_rect)
        } else {
            None
        }
    }

    pub fn begin_docking(&mut self, ui: &ui::Ui, rect: Rect) {
        let root_area = self.areas.get_mut(self.root);
        root_area.rect = rect;

        if root_area.selected.is_none() && !root_area.tabviews.is_empty() {
            root_area.selected = Some(0);
        }

        self.ui.em_size = ui.theme.font_size;
        self.ui.active_tab = None;
    }

    fn draw_tabbar(&mut self, ui: &mut ui::Ui, drawer: &mut Drawer, i_tabview: usize, rect: Rect) {
        let tabview = &self.tabviews[i_tabview];
        let area = self.areas.get_mut(tabview.area);

        let id = ui.activation.make_id();

        if ui.inputs.is_hovering(rect) {
            ui.activation.focused = Some(id);
            if ui.activation.active == None && ui.inputs.left_mouse_button_pressed {
                ui.activation.active = Some(id);
            }
        } else if ui.activation.active == Some(id) {
            self.ui.active_tab = Some(i_tabview);
        }

        if ui.has_clicked(id) {
            area.selected = Some(i_tabview);
        }

        let color = match (ui.activation.focused, ui.activation.active) {
            (Some(f), Some(a)) if f == id && a == id => ColorU32::from_f32(0.13, 0.13, 0.43, 1.0),
            (Some(f), _) if f == id => ColorU32::from_f32(0.13, 0.13, 0.83, 1.0),
            _ => ColorU32::from_f32(0.53, 0.53, 0.73, 1.0),
        };

        drawer.draw_colored_rect(rect, 0, color);

        let (label_run, label_layout) =
            drawer.shape_and_layout_text(&ui.theme.face(), &tabview.title);
        let label_size = label_layout.size();

        drawer.draw_text_run(
            &label_run,
            &label_layout,
            Rect::center(rect, label_size).pos,
            0,
        );

        ui.state.add_rect_to_last_container(rect);
    }

    fn draw_area(&mut self, ui: &mut ui::Ui, drawer: &mut Drawer, area_handle: Handle<Area>) {
        let area = self.areas.get_mut(area_handle);
        let em = self.ui.em_size;

        if ui.inputs.is_hovering(area.rect) {
            if let Some(_) = self.ui.active_tab {
                let area_overlay_rect = area.rect.inset(1.0 * em);
                drawer.draw_colored_rect(
                    area_overlay_rect,
                    0,
                    ColorU32::from_f32(0.05, 0.05, 0.33, 0.5),
                );
            }
        }
    }

    fn draw_docking(&mut self, ui: &mut ui::Ui, drawer: &mut Drawer) {
        let em = self.ui.em_size;

        if let Some(active_tab) = self.ui.active_tab {
            let tabview = &self.tabviews[active_tab];

            let rect = Rect {
                pos: ui.mouse_position(),
                size: [10.0 * em, 1.5 * em],
            };
            drawer.draw_colored_rect(rect, 0, ColorU32::from_f32(0.0, 0.0, 0.0, 0.5));

            let (label_run, label_layout) =
                drawer.shape_and_layout_text(&ui.theme.face(), &tabview.title);
            let label_size = label_layout.size();

            drawer.draw_text_run(
                &label_run,
                &label_layout,
                Rect::center(rect, label_size).pos,
                0,
            );
        }
    }

    // Draw tabwells and docking overlay
    pub fn end_docking(&mut self, ui: &mut ui::Ui, drawer: &mut Drawer) {
        let root_area = self.areas.get_mut(self.root);
        if root_area.should_display_tabwell() {
            let (tabwell_rect, _content_rect) = root_area.rects(self.ui.em_size);
            let mut tabwell_rect = tabwell_rect.unwrap();

            for i_tab in 0..root_area.tabviews.len() {
                let tabview = &self.tabviews[i_tab];
                let (tab_rect, rest_rect) = tabwell_rect
                    .split_left_pixels((tabview.title.len() as f32) * 0.75 * self.ui.em_size);
                tabwell_rect = rest_rect;

                self.draw_tabbar(ui, drawer, i_tab, tab_rect);
            }

            self.draw_area(ui, drawer, self.root);
        }
        self.draw_docking(ui, drawer);
    }
}

impl Area {
    fn should_display_tabwell(&self) -> bool {
        !self.tabviews.is_empty()
    }

    fn rects(&self, em: f32) -> (Option<Rect>, Rect) {
        if self.should_display_tabwell() {
            let (tabwell_rect, content_rect) = self.rect.split_top_pixels(1.5 * em);
            (Some(tabwell_rect), content_rect)
        } else {
            (None, self.rect)
        }
    }
}
