use drawer2d::{drawer::*, rect::*};
use exo::pool::*;

// Struct exposing the immediate-mode docking API
pub struct Docking {
    area_pool: Pool<Area>,
    root: Handle<Area>,
    default_area: Handle<Area>,
    tabviews: Vec<TabView>,
    floating_containers: Vec<Handle<Area>>,
    ui: DockingUi,
}

// Split direction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Horizontal,
    Vertical,
}

// Binary tree internal node that splits two nodes either vertically or horizontally
#[derive(Clone, Debug)]
struct AreaSplitter {
    direction: Direction,
    left_child: Handle<Area>,
    right_child: Handle<Area>,
    splits: f32,
    rect: Rect,
}

// Binary tree leaf that contains tabs
// TODO: Remove dyanmic allocations :(
#[derive(Clone, Debug)]
struct AreaContainer {
    tabviews: Vec<usize>,
    selected: Option<usize>,
    rect: Rect,
}

// A binary tree node representing either a split or a collection of tabs
#[derive(Clone, Debug)]
enum Area {
    Splitter(AreaSplitter),
    Container(AreaContainer),
}

// A tab that can be docked into areas
#[derive(Debug)]
struct TabView {
    title: String,
    area: Handle<Area>,
}

#[derive(Clone, Copy, Debug)]
struct DropTabEvent {
    i_tabview: usize,
    in_container: Handle<Area>,
}

#[derive(Clone, Copy, Debug)]
enum DockingEvent {
    DropTab(DropTabEvent),
    DetachTab(usize),
    Split(SplitDirection, usize, Handle<Area>),
}

struct DockingUi {
    em_size: f32,
    active_tab: Option<usize>,
    events: Vec<DockingEvent>,
}

impl Docking {
    // Create a new docking system
    pub fn new() -> Self {
        let mut docking = Self {
            area_pool: Pool::new(),
            root: Handle::default(),
            default_area: Handle::default(),
            tabviews: Vec::new(),
            floating_containers: Vec::new(),
            ui: DockingUi {
                em_size: 0.0,
                active_tab: None,
                events: Vec::new(),
            },
        };

        docking.root = docking.area_pool.add(Area::Container(AreaContainer {
            selected: None,
            tabviews: Vec::new(),
            rect: Rect {
                pos: [0.0, 0.0],
                size: [0.0, 0.0],
            },
        }));

        docking.default_area = docking.root;

        docking
    }

    // Immediate mode tab rendering, returns the drawing area if the tab is visible
    pub fn tabview(&mut self, tab_name: &str) -> Option<Rect> {
        let i_tabview = self
            .tabviews
            .iter()
            .position(|tabview| tabview.title == tab_name)
            .unwrap_or_else(|| {
                self.tabviews.push(TabView {
                    title: String::from(tab_name),
                    area: self.default_area,
                });

                let i_new_tabview = self.tabviews.len() - 1;
                Self::insert_tabview(
                    &mut self.area_pool,
                    &mut self.tabviews,
                    i_new_tabview,
                    self.default_area,
                );

                i_new_tabview
            });

        let tabview = &self.tabviews[i_tabview];
        let area = self.area_pool.get(tabview.area);

        let container = area.container().unwrap();
        match container.selected {
            Some(i_selected) if container.tabviews[i_selected] == i_tabview => {
                Some(container.rects(self.ui.em_size).1)
            }
            _ => None,
        }
    }

    fn insert_tabview(
        area_pool: &mut Pool<Area>,
        tabviews: &mut [TabView],
        i_tabview: usize,
        area_handle: Handle<Area>,
    ) {
        let area = area_pool.get_mut(area_handle);
        let tabview = &mut tabviews[i_tabview];

        let container = area
            .container_mut()
            .expect("Tab views should not be inserted in splitters.");
        container.tabviews.push(i_tabview);
        tabview.area = area_handle;
    }

    fn remove_tabview(
        area_pool: &mut Pool<Area>,
        tabviews: &mut [TabView],
        i_tabview: usize,
        area_handle: Handle<Area>,
    ) {
        let area = area_pool.get_mut(area_handle);
        let tabview = &mut tabviews[i_tabview];

        let container = area
            .container_mut()
            .expect("Tabviews should not be removed from splitter (there is no tabviews field...)");

        assert!(!container.tabviews.is_empty());

        if container.tabviews.len() > 1 {
            container.tabviews.swap_remove(
                container
                    .tabviews
                    .iter()
                    .position(|i| *i == i_tabview)
                    .unwrap(),
            );
        } else if container.tabviews.len() == 1 {
            assert!(container.tabviews[0] == i_tabview);
            container.tabviews.pop();
        }

        tabview.area = Handle::invalid();
    }

    fn find_parent(
        area_pool: &Pool<Area>,
        node: Handle<Area>,
        child: Handle<Area>,
    ) -> Option<Handle<Area>> {
        let area = area_pool.get(node);

        if let Area::Splitter(splitter) = area {
            if splitter.left_child == child {
                return Some(node);
            }

            if splitter.right_child == child {
                return Some(node);
            }

            if let Some(found) = Self::find_parent(area_pool, splitter.left_child, child) {
                return Some(found);
            }

            if let Some(found) = Self::find_parent(area_pool, splitter.right_child, child) {
                return Some(found);
            }
        }

        None
    }

    // Split previous area with new_area in split_direction
    fn split_area(
        area_pool: &mut Pool<Area>,
        tabviews: &mut [TabView],
        previous_area_handle: Handle<Area>,
        split_direction: SplitDirection,
        new_area_handle: Handle<Area>,
    ) {
        // Copy the previous area
        let previous_area = area_pool.get(previous_area_handle).clone();
        let new_old_area_handle = area_pool.add(previous_area);

        // Update all tabviews to use the new handle
        if let Area::Container(_) = area_pool.get(new_old_area_handle) {
            for tabview in tabviews {
                if tabview.area == previous_area_handle {
                    tabview.area = new_old_area_handle;
                }
            }
        };

        let (left_child, right_child) = match split_direction {
            SplitDirection::Top | SplitDirection::Left => (new_area_handle, new_old_area_handle),
            SplitDirection::Bottom | SplitDirection::Right => {
                (new_old_area_handle, new_area_handle)
            }
        };

        // Replace the old area slot with a new splitter
        *area_pool.get_mut(previous_area_handle) = Area::Splitter(AreaSplitter {
            direction: Direction::from(split_direction),
            left_child,
            right_child,
            splits: 0.5,
            rect: Rect {
                pos: [0.0, 0.0],
                size: [0.0, 0.0],
            },
        });
    }

    // Propagate rect to children, and select tabview if none is selected
    fn update_area_rect(area_pool: &mut Pool<Area>, area_handle: Handle<Area>, area_rect: Rect) {
        match area_pool.get_mut(area_handle) {
            Area::Container(container) => {
                container.rect = area_rect;

                // Update the selected tabview
                match container.selected {
                    Some(i_selected) => {
                        if container.tabviews.is_empty() {
                            // Remove selection if there is no tabs
                            container.selected = None;
                        } else if i_selected >= container.tabviews.len() {
                            // Select the first one if selection is invalid
                            container.selected = Some(0);
                        }
                    }

                    // Select first tab if none is selected
                    None => {
                        if !container.tabviews.is_empty() {
                            container.selected = Some(0);
                        }
                    }
                }
            }

            Area::Splitter(splitter) => {
                splitter.rect = area_rect;

                let (left_child_rect, right_child_rect) = match splitter.direction {
                    Direction::Vertical => area_rect.split_vertical_ratio(splitter.splits),
                    Direction::Horizontal => area_rect.split_horizontal_ratio(splitter.splits),
                };

                let left_child = splitter.left_child;
                let right_child = splitter.right_child;

                Self::update_area_rect(area_pool, left_child, left_child_rect);
                Self::update_area_rect(area_pool, right_child, right_child_rect);
            }
        }
    }

    pub fn begin_docking(&mut self, ui: &ui::Ui, rect: Rect) {
        Self::update_area_rect(&mut self.area_pool, self.root, rect);

        self.ui.em_size = ui.theme.font_size;
        self.ui.active_tab = None;
    }

    pub fn end_docking(&mut self, ui: &mut ui::Ui, drawer: &mut Drawer) {
        let floating_roots = self.floating_containers.clone();

        self.draw_area_rec(ui, drawer, self.root);
        for floating in floating_roots {
            self.draw_area_rec(ui, drawer, floating);
        }

        self.draw_docking(ui, drawer);

        for (area_handle, area) in self.area_pool.iter() {
            Self::draw_area_overlay(&mut self.ui, ui, drawer, area_handle, area);
        }

        // drop events
        for event in &self.ui.events {
            match event {
                DockingEvent::DropTab(event) => {
                    let previous_area = self.tabviews[event.i_tabview].area;
                    if event.in_container != previous_area {
                        Self::remove_tabview(
                            &mut self.area_pool,
                            &mut self.tabviews,
                            event.i_tabview,
                            previous_area,
                        );
                        Self::insert_tabview(
                            &mut self.area_pool,
                            &mut self.tabviews,
                            event.i_tabview,
                            event.in_container,
                        );
                    }
                }

                DockingEvent::Split(direction, i_dropped_tab, container_handle) => {
                    let previous_area = self.tabviews[*i_dropped_tab].area;
                    Self::remove_tabview(
                        &mut self.area_pool,
                        &mut self.tabviews,
                        *i_dropped_tab,
                        previous_area,
                    );
                    let new_container = self.area_pool.add(Area::Container(AreaContainer {
                        selected: Some(0),
                        tabviews: vec![],
                        rect: Rect::default(),
                    }));

                    Self::insert_tabview(
                        &mut self.area_pool,
                        &mut self.tabviews,
                        *i_dropped_tab,
                        new_container,
                    );

                    Self::split_area(
                        &mut self.area_pool,
                        &mut self.tabviews,
                        *container_handle,
                        *direction,
                        new_container,
                    );
                }

                DockingEvent::DetachTab(i_tabview) => {
                    let container_handle = self.tabviews[*i_tabview].area;
                    Self::remove_tabview(
                        &mut self.area_pool,
                        &mut self.tabviews,
                        *i_tabview,
                        container_handle,
                    );

                    let new_container = self.area_pool.add(Area::Container(AreaContainer {
                        selected: Some(0),
                        tabviews: vec![*i_tabview],
                        rect: Rect {
                            pos: [200.0, 200.0],
                            size: [500.0, 500.0],
                        },
                    }));

                    self.tabviews[*i_tabview].area = new_container;
                    self.floating_containers.push(new_container);
                }
            }
        }
        self.ui.events.clear();
    }
}

// -- Drawing
enum TabState {
    Dragging,
    ClickedTitle,
    ClickedDetach,
    None,
}

impl Docking {
    // Draw a tab inside a tabwell
    fn draw_tab(
        ui: &mut ui::Ui,
        drawer: &mut Drawer,
        docking_ui: &mut DockingUi,
        tabview: &TabView,
        rect: &mut Rect,
    ) -> TabState {
        let mut res = TabState::None;
        let em = docking_ui.em_size;
        let id = ui.activation.make_id();

        // -- Layout
        let (label_run, label_layout) =
            drawer.shape_and_layout_text(&ui.theme.face(), &tabview.title);
        let label_size = label_layout.size();

        let title_rect = rect.split_left(label_size[0] + 1.0 * em);
        let detach_rect = rect.split_left(1.5 * em);

        // -- Interaction
        if ui.inputs.is_hovering(title_rect) {
            ui.activation.focused = Some(id);
            if ui.activation.active == None && ui.inputs.left_mouse_button_pressed {
                ui.activation.active = Some(id);
            }
        } else if ui.activation.active == Some(id) {
            res = TabState::Dragging;
        }

        if ui.has_clicked(id) {
            res = TabState::ClickedTitle;
        }

        if ui.button(drawer, ui::Button::with_label("D").rect(detach_rect)) {
            res = TabState::ClickedDetach;
        }

        // -- Drawing
        let color = match (ui.activation.focused, ui.activation.active) {
            (Some(f), Some(a)) if f == id && a == id => ColorU32::from_f32(0.13, 0.13, 0.43, 1.0),
            (Some(f), _) if f == id => ColorU32::from_f32(0.13, 0.13, 0.83, 1.0),
            _ => ColorU32::from_f32(0.53, 0.53, 0.73, 1.0),
        };

        drawer.draw_colored_rect(ColoredRect::new(title_rect).color(color));

        drawer.draw_text_run(
            &label_run,
            &label_layout,
            Rect::center(title_rect, label_size).pos,
            0,
            ColorU32::greyscale(0xFF),
        );

        ui.state.add_rect_to_last_container(title_rect);

        res
    }

    // Draw the overlay and docking controls above a container
    fn draw_area_overlay(
        docking_ui: &mut DockingUi,
        ui: &mut ui::Ui,
        drawer: &mut Drawer,
        area_handle: Handle<Area>,
        area: &Area,
    ) {
        let em = docking_ui.em_size;

        let active_tab = match docking_ui.active_tab {
            Some(i) => i,
            None => return,
        };

        match area {
            Area::Container(container) => {
                // Draw an overlay to dock tabs
                const HANDLE_SIZE: f32 = 3.0;
                const HANDLE_OFFSET: f32 = HANDLE_SIZE + 0.5;
                let drop_rect = Rect::center(container.rect, [HANDLE_SIZE * em, HANDLE_SIZE * em]);
                let split_top_rect = drop_rect.clone().offset([0.0, -HANDLE_OFFSET * em]);
                let split_right_rect = drop_rect.clone().offset([HANDLE_OFFSET * em, 0.0]);
                let split_bottom_rect = drop_rect.clone().offset([0.0, HANDLE_OFFSET * em]);
                let split_left_rect = drop_rect.clone().offset([-HANDLE_OFFSET * em, 0.0]);

                let overlay_color = ColorU32::from_f32(0.25, 0.01, 0.25, 0.25);
                drawer.draw_colored_rects(&[
                    ColoredRect::new(drop_rect).color(overlay_color),
                    ColoredRect::new(split_top_rect).color(overlay_color),
                    ColoredRect::new(split_right_rect).color(overlay_color),
                    ColoredRect::new(split_bottom_rect).color(overlay_color),
                    ColoredRect::new(split_left_rect).color(overlay_color),
                ]);

                // Drop a tab in a container
                if !ui.inputs.left_mouse_button_pressed {
                    if ui.inputs.is_hovering(drop_rect) {
                        docking_ui.events.push(DockingEvent::DropTab(DropTabEvent {
                            i_tabview: active_tab,
                            in_container: area_handle,
                        }));

                        docking_ui.active_tab = None;
                    } else if ui.inputs.is_hovering(split_top_rect) {
                        docking_ui.events.push(DockingEvent::Split(
                            SplitDirection::Top,
                            active_tab,
                            area_handle,
                        ));
                    } else if ui.inputs.is_hovering(split_right_rect) {
                        docking_ui.events.push(DockingEvent::Split(
                            SplitDirection::Right,
                            active_tab,
                            area_handle,
                        ));
                    } else if ui.inputs.is_hovering(split_bottom_rect) {
                        docking_ui.events.push(DockingEvent::Split(
                            SplitDirection::Bottom,
                            active_tab,
                            area_handle,
                        ));
                    } else if ui.inputs.is_hovering(split_left_rect) {
                        docking_ui.events.push(DockingEvent::Split(
                            SplitDirection::Left,
                            active_tab,
                            area_handle,
                        ));
                    }
                }
            }
            _ => {}
        }
    }

    // Draw the ui for a docking area
    fn draw_area_rec(&mut self, ui: &mut ui::Ui, drawer: &mut Drawer, area_handle: Handle<Area>) {
        let em = self.ui.em_size;
        let area = self.area_pool.get_mut(area_handle);

        match area {
            Area::Container(container) => {
                let (tabwell_rect, _content_rect) = container.rects(self.ui.em_size);
                let mut tabwell_rect = tabwell_rect;

                // Draw the tabwell background
                drawer.draw_colored_rect(
                    ColoredRect::new(tabwell_rect).color(ColorU32::greyscale(0x3A)),
                );

                // Draw each tab title
                for (i, i_tabview) in container.tabviews.iter().enumerate() {
                    let tabview = &self.tabviews[*i_tabview];

                    let _margin = tabwell_rect.split_left(0.5 * em);
                    let tabstate =
                        Self::draw_tab(ui, drawer, &mut self.ui, tabview, &mut tabwell_rect);
                    match tabstate {
                        TabState::Dragging => {
                            self.ui.active_tab = Some(*i_tabview);
                        }
                        TabState::ClickedTitle => {
                            container.selected = Some(i);
                        }
                        TabState::ClickedDetach => {
                            self.ui.events.push(DockingEvent::DetachTab(*i_tabview))
                        }
                        _ => {}
                    }
                }
                // Draw a border between the tabwell and the top, and the tabwell and the content
                let top_border_rect = tabwell_rect.split_top((0.1 * em).max(1.0));
                let bottom_border_rect = tabwell_rect.split_bottom(0.2 * em);
                drawer.draw_colored_rects(&[
                    ColoredRect::new(top_border_rect).color(ColorU32::greyscale(0x2A)),
                    ColoredRect::new(bottom_border_rect).color(ColorU32::greyscale(0x2A)),
                ]);
            }

            Area::Splitter(splitter) => {
                let direction = splitter.direction;
                let left_child = splitter.left_child;
                let right_child = splitter.right_child;

                match direction {
                    Direction::Vertical => {
                        ui.splitter_x(
                            drawer,
                            ui::Splitter {
                                rect: splitter.rect,
                            },
                            &mut splitter.splits,
                        );
                    }
                    Direction::Horizontal => {
                        ui.splitter_y(
                            drawer,
                            ui::Splitter {
                                rect: splitter.rect,
                            },
                            &mut splitter.splits,
                        );
                    }
                }

                self.draw_area_rec(ui, drawer, left_child);
                self.draw_area_rec(ui, drawer, right_child);
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
            drawer.draw_colored_rect(
                ColoredRect::new(rect).color(ColorU32::from_f32(0.0, 0.0, 0.0, 0.5)),
            );

            let (label_run, label_layout) =
                drawer.shape_and_layout_text(&ui.theme.face(), &tabview.title);
            let label_size = label_layout.size();

            drawer.draw_text_run(
                &label_run,
                &label_layout,
                Rect::center(rect, label_size).pos,
                0,
                ColorU32::greyscale(0xFF),
            );
        }
    }
}

impl AreaContainer {
    fn rects(&self, em: f32) -> (Rect, Rect) {
        let mut content_rect = self.rect;
        let tabwell_rect = content_rect.split_top(1.5 * em);
        (tabwell_rect, content_rect)
    }
}

impl Area {
    pub fn splitter(&self) -> Option<&AreaSplitter> {
        match &self {
            Self::Splitter(inner) => Some(inner),
            _ => None,
        }
    }

    pub fn splitter_mut(&mut self) -> Option<&mut AreaSplitter> {
        match self {
            Self::Splitter(inner) => Some(inner),
            _ => None,
        }
    }

    pub fn container(&self) -> Option<&AreaContainer> {
        match &self {
            Self::Container(inner) => Some(inner),
            _ => None,
        }
    }

    pub fn container_mut(&mut self) -> Option<&mut AreaContainer> {
        match self {
            Self::Container(inner) => Some(inner),
            _ => None,
        }
    }
}

impl Default for Docking {
    fn default() -> Self {
        Self::new()
    }
}

impl From<SplitDirection> for Direction {
    fn from(split_direction: SplitDirection) -> Self {
        match split_direction {
            SplitDirection::Top | SplitDirection::Bottom => Self::Horizontal,
            SplitDirection::Left | SplitDirection::Right => Self::Vertical,
        }
    }
}
