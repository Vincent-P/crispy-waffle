use drawer2d::{drawer::*, rect::*};
use exo::pool::*;

// Struct exposing the immediate-mode docking API
pub struct Docking {
    area_tree: Pool<Area>,
    root: Handle<Area>,
    default_area: Handle<Area>,
    tabviews: Vec<TabView>,
    ui: DockingUi,
}

// Split direction
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Horizontal,
    Vertical,
}

// A docking area that contains splits or containers
#[derive(Debug)]
struct AreaSplitter {
    direction: Direction,
    children: Vec<Handle<Area>>,
    splits: Vec<f32>,
    rect: Rect,
}

// A docking area that contains tabs
#[derive(Debug)]
struct AreaContainer {
    tabviews: Vec<usize>,
    selected: Option<usize>,
    rect: Rect,
}

// A docking area
#[derive(Debug)]
enum Area {
    Splitter(AreaSplitter),
    Container(AreaContainer),
}

// A tab that can be docked into areas
struct TabView {
    title: String,
    area: Handle<Area>,
}

#[derive(Clone, Copy, Debug)]
enum DragType {
    SplitTop,
    SplitBottom,
    SplitLeft,
    SplitRight,
    Dock,
    SplitHorizontal,
    SplitVertical,
}

#[derive(Clone, Copy, Debug)]
struct DragEvent {
    i_tabview: usize,
    previous_area: Handle<Area>,
    next_area: Handle<Area>,
    drag_type: DragType,
}

struct DockingUi {
    em_size: f32,
    active_tab: Option<usize>,
    dragging_events: Vec<DragEvent>,
}

impl Docking {
    // Create a new docking system
    pub fn new() -> Self {
        let mut docking = Self {
            area_tree: Pool::new(),
            root: Handle::default(),
            default_area: Handle::default(),
            tabviews: Vec::new(),
            ui: DockingUi {
                em_size: 0.0,
                active_tab: None,
                dragging_events: Vec::new(),
            },
        };

        docking.root = docking.area_tree.add(Area::from(AreaContainer {
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
                    &mut self.area_tree,
                    &mut self.tabviews,
                    i_new_tabview,
                    self.default_area,
                    DragType::Dock,
                );

                i_new_tabview
            });

        let tabview = &self.tabviews[i_tabview];
        let area = self.area_tree.get(tabview.area);

        let container = area.container().unwrap();
        match container.selected {
            Some(i_selected) if container.tabviews[i_selected] == i_tabview => {
                Some(container.rects(self.ui.em_size).1)
            }
            _ => None,
        }
    }

    fn insert_tabview(
        area_tree: &mut Pool<Area>,
        tabviews: &mut [TabView],
        i_tabview: usize,
        area_handle: Handle<Area>,
        drag_type: DragType,
    ) {
        let area = area_tree.get_mut(area_handle);
        let tabview = &mut tabviews[i_tabview];

        match drag_type {
            DragType::Dock => {
                let container = area
                    .container_mut()
                    .expect("Tab views should not be inserted in splitters.");
                container.tabviews.push(i_tabview);
                tabview.area = area_handle;
            }
            _ => {}
        }
    }

    fn remove_tabview(
        area_tree: &mut Pool<Area>,
        tabviews: &mut Vec<TabView>,
        i_tabview: usize,
        area_handle: Handle<Area>,
    ) {
        let area = area_tree.get_mut(area_handle);
        let tabview = &mut tabviews[i_tabview];

        match area {
            Area::Container(container) => {
                if container.tabviews.len() > 1 {
                    container.tabviews.swap_remove(
                        container
                            .tabviews
                            .iter()
                            .position(|i| *i == i_tabview)
                            .unwrap(),
                    );
                } else {
                    container.tabviews.pop();
                }

                tabview.area = Handle::invalid();
            }
            Area::Splitter(_splitter) => {
                panic!(
                    "Tabviews should not be removed from splitter (there is no tabviews field...)"
                )
            }
        }
    }

    fn find_parent(
        area_tree: &Pool<Area>,
        area_handle: Handle<Area>,
        needle: Handle<Area>,
    ) -> Option<Handle<Area>> {
        let area = area_tree.get(area_handle);

        if let Area::Splitter(splitter) = area {
            for child in &splitter.children {
                if *child == needle {
                    return Some(area_handle);
                }

                if let Some(found) = Self::find_parent(area_tree, *child, needle) {
                    return Some(found);
                }
            }
        }

        None
    }

    fn split_container(
        area_tree: &mut Pool<Area>,
        root: &mut Handle<Area>,
        area_handle: Handle<Area>,
        direction: Direction,
    ) {
        // Only containers should be split
        assert!(area_tree.get_mut(area_handle).container().is_some());

        // Create a new empty split
        let empty_container = area_tree.add(Area::from(AreaContainer {
            selected: None,
            tabviews: Vec::new(),
            rect: Rect {
                pos: [0.0, 0.0],
                size: [0.0, 0.0],
            },
        }));

        // Create a parent to split the empty split and the area
        let new_split = area_tree.add(Area::from(AreaSplitter {
            direction,
            children: vec![area_handle, empty_container],
            splits: vec![0.5],
            rect: Rect {
                pos: [0.0, 0.0],
                size: [0.0, 0.0],
            },
        }));

        // Find the parent of the area to split
        if let Some(parent_handle) = Self::find_parent(area_tree, *root, area_handle) {
            // Get the original position of the area
            let i_container_in_original_parent = area_tree
                .get(parent_handle)
                .splitter()
                .expect("Parents are splitter")
                .children
                .iter()
                .position(|child| *child == area_handle)
                .unwrap();

            // Replace it with the newly created split
            area_tree
                .get_mut(parent_handle)
                .splitter_mut()
                .expect("Parents are splitter")
                .children[i_container_in_original_parent] = new_split;
        } else {
            // If the area doesn't have a parent, it should be the root
            assert!(area_handle == *root);

            *root = new_split;
        }
    }

    // Propagate rect to children, and select tabview if none is selected
    fn update_area(area_tree: &mut Pool<Area>, area_handle: Handle<Area>, area_rect: Rect) {
        match area_tree.get_mut(area_handle) {
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

                // Need to clone all splitter fields here to tell the borrow checker
                // that I *don't* use `splitter` inside the loop
                let direction = splitter.direction;
                let children = splitter.children.clone();
                let mut splits_iter = splitter.splits.clone().into_iter();
                let mut range_start = 0.0;

                for child in children {
                    let end = splits_iter.next().unwrap_or(1.0);
                    let child_rect = match direction {
                        Direction::Vertical => area_rect.split_vertical_range(range_start, end),
                        Direction::Horizontal => area_rect.split_horizontal_range(range_start, end),
                    };
                    range_start = end;

                    Self::update_area(area_tree, child, child_rect);
                }
            }
        }
    }

    pub fn begin_docking(&mut self, ui: &ui::Ui, rect: Rect) {
        Self::update_area(&mut self.area_tree, self.root, rect);

        self.ui.em_size = ui.theme.font_size;
        self.ui.active_tab = None;
    }

    // Draw a tab inside a tabwell
    fn draw_tab(
        ui: &mut ui::Ui,
        drawer: &mut Drawer,
        docking_ui: &mut DockingUi,
        tabview: &TabView,
        i_tabview: usize,
        area: &mut AreaContainer,
        rect: Rect,
    ) {
        let id = ui.activation.make_id();

        if ui.inputs.is_hovering(rect) {
            ui.activation.focused = Some(id);
            if ui.activation.active == None && ui.inputs.left_mouse_button_pressed {
                ui.activation.active = Some(id);
            }
        } else if ui.activation.active == Some(id) {
            docking_ui.active_tab = Some(area.tabviews[i_tabview]);
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

    // Draw the overlay and docking controls above a container
    fn draw_area_overlay(
        docking_ui: &mut DockingUi,
        tabviews: &mut Vec<TabView>,
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
            Area::Splitter(splitter) => match splitter.direction {
                Direction::Vertical => {
                    let mut splits = vec![0.0];
                    for split in &splitter.splits {
                        splits.push(*split);
                    }
                    splits.push(1.0);

                    const HANDLE_WIDTH: f32 = 2.0;

                    let rects = splits.into_iter().map(|split| Rect {
                        pos: [
                            splitter.rect.pos[0] + split * splitter.rect.size[0]
                                - HANDLE_WIDTH * em / 2.0,
                            splitter.rect.pos[1],
                        ],
                        size: [HANDLE_WIDTH * em, splitter.rect.size[1]],
                    });

                    for (i_rect, rect) in rects.into_iter().enumerate() {
                        let overlay_color = ColorU32::from_f32(0.25, 0.01, 0.25, 0.25);
                        drawer.draw_colored_rect(rect, 0, overlay_color);

                        if ui.inputs.is_hovering(rect) && !ui.inputs.left_mouse_button_pressed {
                            println!("Dropped tab #{} in {} split.", active_tab, i_rect);
                        }
                    }
                }
                Direction::Horizontal => {
                    let mut splits = vec![0.0];
                    for split in &splitter.splits {
                        splits.push(*split);
                    }
                    splits.push(1.0);

                    const HANDLE_WIDTH: f32 = 3.0;

                    let rects = splits.into_iter().map(|split| Rect {
                        pos: [
                            splitter.rect.pos[0],
                            splitter.rect.pos[1] + split * splitter.rect.size[1]
                                - HANDLE_WIDTH * em / 2.0,
                        ],
                        size: [splitter.rect.size[0], HANDLE_WIDTH * em],
                    });

                    for (i_rect, rect) in rects.into_iter().enumerate() {
                        let overlay_color = ColorU32::from_f32(0.25, 0.01, 0.25, 0.25);
                        drawer.draw_colored_rect(rect, 0, overlay_color);

                        if ui.inputs.is_hovering(rect) && !ui.inputs.left_mouse_button_pressed {
                            println!("Dropped tab #{} in {} split.", active_tab, i_rect);
                        }
                    }
                }
            },
            Area::Container(container) => {
                // Draw an overlay to dock tabs
                let overlay_rect = container.rect.inset(2.0 * em);
                let overlay_color = ColorU32::from_f32(0.25, 0.01, 0.25, 0.25);
                drawer.draw_colored_rect(overlay_rect, 0, overlay_color);

                if ui.inputs.is_hovering(overlay_rect) && !ui.inputs.left_mouse_button_pressed {
                    docking_ui.dragging_events.push(DragEvent {
                        i_tabview: active_tab,
                        previous_area: tabviews[active_tab].area,
                        next_area: area_handle,
                        drag_type: DragType::Dock,
                    });

                    docking_ui.active_tab = None;
                }
            }
        }
    }

    // Draw the ui for a docking area
    fn draw_area_rec(
        &mut self,
        ui: &mut ui::Ui,
        drawer: &mut Drawer,
        area_handle: Handle<Area>,
        parent_direction: Option<Direction>,
    ) {
        let em = self.ui.em_size;
        let area = self.area_tree.get_mut(area_handle);

        match area {
            Area::Container(container) => {
                let (tabwell_rect, _content_rect) = container.rects(self.ui.em_size);
                let mut tabwell_rect = tabwell_rect;

                // Draw the tabwell background
                drawer.draw_colored_rect(tabwell_rect, 0, ColorU32::greyscale(0x38));

                // Draw each tab title
                for i_tabview in 0..container.tabviews.len() {
                    let tabview = &self.tabviews[container.tabviews[i_tabview]];

                    let (tab_rect, rest_rect) =
                        tabwell_rect.split_left_pixels((tabview.title.len() as f32) * 0.75 * em);
                    tabwell_rect = rest_rect;

                    Self::draw_tab(
                        ui,
                        drawer,
                        &mut self.ui,
                        tabview,
                        i_tabview,
                        container,
                        tab_rect,
                    );
                }

                // Draw the splits button
                let (rest_rect, split_h_rect) = tabwell_rect.split_right_pixels(1.5 * em);
                if ui.button(
                    drawer,
                    ui::Button {
                        label: "H",
                        rect: split_h_rect,
                        enabled: parent_direction != Some(Direction::Horizontal),
                    },
                ) {
                    self.ui.dragging_events.push(DragEvent {
                        i_tabview: 0,
                        previous_area: area_handle,
                        next_area: Handle::invalid(),
                        drag_type: DragType::SplitHorizontal,
                    });
                }

                let (rest_rect, split_v_rect) = rest_rect.split_right_pixels(1.5 * em);
                if ui.button(
                    drawer,
                    ui::Button {
                        label: "V",
                        rect: split_v_rect,
                        enabled: parent_direction != Some(Direction::Vertical),
                    },
                ) {
                    self.ui.dragging_events.push(DragEvent {
                        i_tabview: 0,
                        previous_area: area_handle,
                        next_area: Handle::invalid(),
                        drag_type: DragType::SplitVertical,
                    });
                }
            }
            Area::Splitter(splitter) => {
                let direction = splitter.direction;
                let mut previous_split = 0.0;
                let mut split_iter = splitter.splits.iter_mut().peekable();
                loop {
                    let cur_split = match split_iter.next() {
                        Some(s) => s,
                        None => break,
                    };
                    let next_split = split_iter.peek().map(|r| **r).unwrap_or(1.0);

                    match direction {
                        Direction::Vertical => {
                            ui.splitter_x(
                                drawer,
                                ui::Splitter {
                                    rect: splitter.rect,
                                },
                                cur_split,
                            );
                        }
                        Direction::Horizontal => {
                            ui.splitter_y(
                                drawer,
                                ui::Splitter {
                                    rect: splitter.rect,
                                },
                                cur_split,
                            );
                        }
                    }

                    *cur_split = cur_split.clamp(previous_split + 0.02, next_split - 0.02);
                    previous_split = *cur_split;
                }

                for child in splitter.children.clone() {
                    self.draw_area_rec(ui, drawer, child, Some(direction));
                }
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
        let root_direction = match self.area_tree.get(self.root) {
            Area::Splitter(splitter) => Some(splitter.direction),
            Area::Container(container) => None,
        };
        self.draw_area_rec(ui, drawer, self.root, root_direction);
        self.draw_docking(ui, drawer);

        for (area_handle, area) in self.area_tree.iter() {
            Self::draw_area_overlay(
                &mut self.ui,
                &mut self.tabviews,
                ui,
                drawer,
                area_handle,
                area,
            );
        }

        // drop events
        for event in &self.ui.dragging_events {
            dbg!(&event);
            match event.drag_type {
                DragType::Dock => {
                    Self::remove_tabview(
                        &mut self.area_tree,
                        &mut self.tabviews,
                        event.i_tabview,
                        event.previous_area,
                    );
                    Self::insert_tabview(
                        &mut self.area_tree,
                        &mut self.tabviews,
                        event.i_tabview,
                        event.next_area,
                        event.drag_type,
                    );
                }
                DragType::SplitVertical => {
                    Self::split_container(
                        &mut self.area_tree,
                        &mut self.root,
                        event.previous_area,
                        Direction::Vertical,
                    );
                }
                DragType::SplitHorizontal => {
                    Self::split_container(
                        &mut self.area_tree,
                        &mut self.root,
                        event.previous_area,
                        Direction::Horizontal,
                    );
                }
                _ => {}
            }
        }
        self.ui.dragging_events.clear();
    }
}

impl AreaContainer {
    fn rects(&self, em: f32) -> (Rect, Rect) {
        let (tabwell_rect, content_rect) = self.rect.split_top_pixels(1.5 * em);
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

impl From<AreaSplitter> for Area {
    fn from(splitter: AreaSplitter) -> Self {
        Self::Splitter(splitter)
    }
}

impl From<AreaContainer> for Area {
    fn from(container: AreaContainer) -> Self {
        Self::Container(container)
    }
}
