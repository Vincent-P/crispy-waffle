const FPS_HISTOGRAM_LENGTH: usize = 512;
pub struct FpsHistogram {
    frame_times: [f32; FPS_HISTOGRAM_LENGTH],
}

impl FpsHistogram {
    pub fn new() -> Self {
        Self {
            frame_times: [0.0; FPS_HISTOGRAM_LENGTH],
        }
    }

    pub fn push_time(&mut self, dt: f32) {
        self.frame_times.rotate_right(1);
        self.frame_times[0] = dt;
    }
}

impl Default for FpsHistogram {
    fn default() -> Self {
        Self::new()
    }
}

pub mod widgets {
    use drawer2d::{drawer::*, rect::*};

    pub struct FpsHistogram<'a> {
        pub histogram: &'a super::FpsHistogram,
        pub rect: Rect,
    }

    #[allow(clippy::excessive_precision)]
    fn turbo_colormap(x: f32) -> [f32; 3] {
        const RED_VEC4: [f32; 4] = [0.13572138, 4.61539260, -42.66032258, 132.13108234];
        const GREEN_VEC4: [f32; 4] = [0.09140261, 2.19418839, 4.84296658, -14.18503333];
        const BLUE_VEC4: [f32; 4] = [0.10667330, 12.64194608, -60.58204836, 110.36276771];
        const RED_VEC2: [f32; 2] = [-152.94239396, 59.28637943];
        const GREEN_VEC2: [f32; 2] = [4.27729857, 2.82956604];
        const BLUE_VEC2: [f32; 2] = [-89.90310912, 27.34824973];
        let dot4 = |a: [f32; 4], b: [f32; 4]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
        let dot2 = |a: [f32; 2], b: [f32; 2]| a[0] * b[0] + a[1] * b[1];

        let x = x.clamp(0.0, 1.0);
        let v4 = [1.0, x, x * x, x * x * x];
        let v2 = [v4[2] * v4[2], v4[3] * v4[2]];

        [
            dot4(v4, RED_VEC4) + dot2(v2, RED_VEC2),
            dot4(v4, GREEN_VEC4) + dot2(v2, GREEN_VEC2),
            dot4(v4, BLUE_VEC4) + dot2(v2, BLUE_VEC2),
        ]
    }

    // https://www.asawicki.info/news_1758_an_idea_for_visualization_of_frame_times
    pub fn histogram(ui: &mut ui::Ui, drawer: &mut Drawer, widget: FpsHistogram) {
        let mut cursor = [
            widget.rect.pos[0] + widget.rect.size[0],
            widget.rect.pos[1] + widget.rect.size[1],
        ];

        drawer.draw_colored_rect(
            ColoredRect::new(widget.rect).color(ColorU32::from_f32(0.0, 0.0, 0.0, 0.5)),
        );
        ui.state.add_rect_to_last_container(widget.rect);

        for dt in widget.histogram.frame_times.iter() {
            if cursor[0] < widget.rect.pos[0] {
                break;
            }

            let target_fps: f32 = 144.0;
            let max_frame_time: f32 = 1.0 / 15.0; // in seconds

            let rect_width = dt / (1.0 / target_fps);
            let height_factor = (dt.log2() - (1.0 / target_fps).log2())
                / ((max_frame_time).log2() - (1.0 / target_fps).log2());
            let rect_height = height_factor.clamp(0.1, 1.0) * widget.rect.size[1];
            let rect_color = turbo_colormap(dt / (1.0 / 120.0));
            let rect_color = ColorU32::from_f32(rect_color[0], rect_color[1], rect_color[2], 1.0);

            let rect_width = rect_width.max(1.0);
            let rect_height = rect_height.max(1.0);

            cursor[0] -= rect_width;

            let rect = Rect {
                pos: [cursor[0].ceil(), (cursor[1] - rect_height).ceil()],
                size: [rect_width, rect_height],
            };
            drawer.draw_colored_rect(ColoredRect::new(rect).color(rect_color));
            ui.state.add_rect_to_last_container(rect);
        }

        const FRAMES_FOR_FPS: usize = 30;
        let fps = (widget.histogram.frame_times.len().min(FRAMES_FOR_FPS) as f32)
            / widget
                .histogram
                .frame_times
                .iter()
                .take(FRAMES_FOR_FPS)
                .fold(0.0, |acc, x| acc + x);

        drawer.draw_label(
            &ui.theme.face(),
            &format!("{}", fps),
            widget.rect,
            !0u32,
            ColorU32::greyscale(255),
        );
    }
}
