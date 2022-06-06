use drawer2d::{drawer::*, rect::*};
use exo::pool::*;

struct DockingUi {
    em_size: f32,
    active_tab: Option<usize>,
    drag_events: Vec<(usize, Handle<Area>, Handle<Area>)>,
}

pub struct Docking {
    areas: Pool<Area>,
    root: Handle<Area>,
    default_area: Handle<Area>,
    tabviews: Vec<TabView>,
    ui: DockingUi,
}

#[derive(Clone, Copy, Debug)]
enum Direction {
    None,
    Horizontal,
    Vertical,
}

#[derive(Debug)]
struct Area {
    direction: Direction,
    children: Vec<Handle<Area>>,
    splits: Vec<f32>,
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
            default_area: Handle::default(),
            tabviews: Vec::new(),
            ui: DockingUi {
                em_size: 0.0,
                active_tab: None,
                drag_events: Vec::new(),
            },
        };

        let split_left = docking.areas.add(Area {
            direction: Direction::None,
            children: Vec::new(),
            splits: Vec::new(),
            selected: None,
            tabviews: Vec::new(),
            rect: Rect {
                pos: [0.0, 0.0],
                size: [0.0, 0.0],
            },
        });

        let split_right = docking.areas.add(Area {
            direction: Direction::None,
            children: Vec::new(),
            splits: Vec::new(),
            selected: None,
            tabviews: Vec::new(),
            rect: Rect {
                pos: [0.0, 0.0],
                size: [0.0, 0.0],
            },
        });

        docking.root = docking.areas.add(Area {
            direction: Direction::Horizontal,
            children: vec![split_left, split_right],
            splits: vec![0.3],
            selected: None,
            tabviews: Vec::new(),
            rect: Rect {
                pos: [0.0, 0.0],
                size: [0.0, 0.0],
            },
        });

        docking.default_area = split_left;

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
                    area: self.default_area,
                });

                let default_area = self.areas.get_mut(self.default_area);
                default_area.tabviews.push(self.tabviews.len() - 1);

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

    // Propagate rect to children, and select tabview if none is selected
    fn update_area(&mut self, area_handle: Handle<Area>, area_rect: Rect) {
        let area = self.areas.get_mut(area_handle);

        // Update the selected tabview
        match area.selected {
            Some(i_selected) => {
                if area
                    .tabviews
                    .iter()
                    .position(|i_tabview| *i_tabview == i_selected)
                    .is_none()
                {
                    area.selected = area.tabviews.iter().next().map(|r| *r);
                }
            }

            // Select first tab if none is selected
            None => {
                if !area.tabviews.is_empty() {
                    area.selected = Some(area.tabviews[0]);
                }
            }
        }

        area.rect = area_rect;

        let direction = area.direction;
        let children = area.children.clone();
        let mut splits = area.splits.clone().into_iter();

        let mut range_start = 0.0;

        for child in children {
            let end = splits.next().unwrap_or(1.0);
            let child_rect = match direction {
                Direction::Vertical => area_rect.split_vertical_range(range_start, end),
                Direction::Horizontal => area_rect.split_horizontal_range(range_start, end),
                _ => {
                    unreachable!()
                }
            };
            range_start = end;

            self.update_area(child, child_rect);
        }
    }

    pub fn begin_docking(&mut self, ui: &ui::Ui, rect: Rect) {
        self.update_area(self.root, rect);

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

    fn draw_area_overlay_impl(
        docking_ui: &mut DockingUi,
        tabviews: &mut Vec<TabView>,
        ui: &mut ui::Ui,
        drawer: &mut Drawer,
        area_handle: Handle<Area>,
        area: &Area,
    ) {
        let em = docking_ui.em_size;

        // Don't draw overlay on internal areas, only leaves
        if !area.children.is_empty() {
            return;
        }

        {
            let text = format!("Tabviews: {:?}", area.tabviews);
            let (label_run, label_layout) = drawer.shape_and_layout_text(&ui.theme.face(), &text);
            let label_size = label_layout.size();

            drawer.draw_text_run(
                &label_run,
                &label_layout,
                Rect::center(area.rect, label_size).pos,
                0,
            );
        }

        // Draw an overlay if the currently dragged tab is hovering this area
        if ui.inputs.is_hovering(area.rect) {
            if let Some(active_tab) = docking_ui.active_tab {
                let area_overlay_rect = area.rect.inset(1.0 * em);
                drawer.draw_colored_rect(
                    area_overlay_rect,
                    0,
                    ColorU32::from_f32(0.05, 0.05, 0.33, 0.5),
                );

                // Tab was dropped in this area
                if !ui.inputs.left_mouse_button_pressed {
                    let tabview = &mut tabviews[active_tab];
                    let previous_area = tabview.area;
                    let new_area = area_handle;
                    docking_ui
                        .drag_events
                        .push((active_tab, previous_area, new_area));

                    docking_ui.active_tab = None;
                }
            }
        }
    }

    fn draw_area_rec(&mut self, ui: &mut ui::Ui, drawer: &mut Drawer, area_handle: Handle<Area>) {
        let area = self.areas.get_mut(area_handle);
        if area.children.is_empty() {
            let (tabwell_rect, _content_rect) = area.rects(self.ui.em_size);

            if let Some(tabwell_rect) = tabwell_rect {
                let mut tabwell_rect = tabwell_rect;

                // Draw the tabwell background
                drawer.draw_colored_rect(tabwell_rect, 0, ColorU32::greyscale(0x38));

                // Draw each tab title
                for i_tabview in area.tabviews.clone() {
                    let tabview = &self.tabviews[i_tabview];

                    let (tab_rect, rest_rect) = tabwell_rect
                        .split_left_pixels((tabview.title.len() as f32) * 0.75 * self.ui.em_size);
                    tabwell_rect = rest_rect;

                    self.draw_tabbar(ui, drawer, i_tabview, tab_rect);
                }
            }
        } else {
            for child in area.children.clone() {
                self.draw_area_rec(ui, drawer, child);
            }
        }
    }

    fn draw_docking(&mut self, ui: &mut ui::Ui, drawer: &mut Drawer) {
        let em = self.ui.em_size;

        // Draw the currently dragged tabview
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
        self.draw_area_rec(ui, drawer, self.root);
        self.draw_docking(ui, drawer);

        for (area_handle, area) in self.areas.iter() {
            Self::draw_area_overlay_impl(
                &mut self.ui,
                &mut self.tabviews,
                ui,
                drawer,
                area_handle,
                area,
            );
        }

        // drop events
        for (active_tab, previous_area_handle, new_area_handle) in &self.ui.drag_events {
            let previous_area = &mut self.areas.get_mut(*previous_area_handle);

            if previous_area.tabviews.len() > 1 {
                previous_area.tabviews.swap_remove(
                    previous_area
                        .tabviews
                        .iter()
                        .position(|i_tabview| *i_tabview == *active_tab)
                        .unwrap(),
                );
            } else {
                previous_area.tabviews.pop();
            }

            let new_area = &mut self.areas.get_mut(*new_area_handle);
            new_area.tabviews.push(*active_tab);
            new_area.selected = Some(*active_tab);

            self.tabviews[*active_tab].area = *new_area_handle;
        }
        self.ui.drag_events.clear();
    }
}

impl Area {
    fn rects(&self, em: f32) -> (Option<Rect>, Rect) {
        if !self.tabviews.is_empty() {
            let (tabwell_rect, content_rect) = self.rect.split_top_pixels(1.5 * em);
            (Some(tabwell_rect), content_rect)
        } else {
            (None, self.rect)
        }
    }
}
