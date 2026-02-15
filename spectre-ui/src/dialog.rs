use eframe::egui;

pub struct DialogBuilder {
    title: String,
    width_ratio: f32,
    height_ratio: f32,
    min_width: f32,
    max_width: f32,
    min_height: f32,
    max_height: f32,
    margin: f32,
    bottom_margin: f32,
    padding: f32,
    header_footer_space: f32,
    fullscreen: bool,
}

impl Default for DialogBuilder {
    fn default() -> Self {
        Self {
            title: String::new(),
            width_ratio: 0.85,
            height_ratio: 0.9,
            min_width: 700.0,
            max_width: 1000.0,
            min_height: 600.0,
            max_height: 900.0,
            margin: 20.0,
            bottom_margin: 20.0,
            padding: 20.0,
            header_footer_space: 150.0,
            fullscreen: false,
        }
    }
}

impl DialogBuilder {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    pub fn width_ratio(mut self, ratio: f32) -> Self {
        self.width_ratio = ratio;
        self
    }

    pub fn height_ratio(mut self, ratio: f32) -> Self {
        self.height_ratio = ratio;
        self
    }

    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = width;
        self
    }

    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = height;
        self
    }

    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = height;
        self
    }

    pub fn margin(mut self, margin: f32) -> Self {
        self.margin = margin;
        self
    }

    pub fn bottom_margin(mut self, bottom_margin: f32) -> Self {
        self.bottom_margin = bottom_margin;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn header_footer_space(mut self, space: f32) -> Self {
        self.header_footer_space = space;
        self
    }

    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.fullscreen = fullscreen;
        self
    }

    pub fn show<F>(self, ctx: &egui::Context, content: F)
    where
        F: FnOnce(&mut egui::Ui, DialogContext),
    {
        self.show_with_footer(ctx, content, |_| {})
    }

    pub fn show_with_footer<F, G>(self, ctx: &egui::Context, content: F, footer: G)
    where
        F: FnOnce(&mut egui::Ui, DialogContext),
        G: FnOnce(&mut egui::Ui),
    {
        let window_size = ctx.input(|i| {
            if let Some(rect) = i.viewport().inner_rect {
                egui::Vec2::new(rect.width(), rect.height())
            } else {
                ctx.screen_rect().size()
            }
        });
        
        let (dialog_width, dialog_height, dialog_pos) = if self.fullscreen {
            let width = window_size.x;
            let height = window_size.y;
            let pos = egui::pos2(0.0, 0.0);
            (width, height, pos)
        } else {
            let max_allowed_width = window_size.x - self.margin * 2.0;
            let width = (window_size.x * self.width_ratio)
                .min(max_allowed_width)
                .max(self.min_width.min(max_allowed_width))
                .min(self.max_width.min(max_allowed_width));
            let max_allowed_height = window_size.y - self.margin - self.bottom_margin;
            let calculated_height = window_size.y * self.height_ratio;
            let height = calculated_height
                .min(max_allowed_height)
                .max(self.min_height.min(max_allowed_height))
                .min(self.max_height.min(max_allowed_height));
            
            let center_x = (window_size.x - width) / 2.0;
            // Position so dialog bottom is bottom_margin from window bottom (uses more of bottom space)
            let center_y = window_size.y - self.bottom_margin - height;
            let pos = egui::pos2(center_x, center_y);
            (width, height, pos)
        };

        egui::Window::new(&self.title)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .fixed_pos(dialog_pos)
            .fixed_size([dialog_width, dialog_height])
            .show(ctx, |ui| {
                let available_width = ui.available_width();
                let available_height = ui.available_height();

                ui.horizontal(|ui| {
                    ui.add_space(self.padding);

                    ui.vertical(|ui| {
                        let content_width = available_width - self.padding * 2.0;
                        ui.set_width(content_width);

                        let footer_height = 60.0;
                        let min_content_height = (available_height - footer_height - self.header_footer_space).max(400.0);
                        ui.set_min_height(min_content_height);

                        let scroll_height = ui.available_height();
                        let dialog_ctx = DialogContext {
                            content_width,
                            scroll_height,
                            padding: self.padding,
                        };

                        egui::ScrollArea::vertical()
                            .id_source(format!("dialog_scroll_{}", self.title))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.vertical(|ui| {
                                    content(ui, dialog_ctx);
                                });
                            });

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);

                        footer(ui);
                    });

                    ui.add_space(self.padding);
                });
            });
    }
}

pub struct DialogContext {
    pub content_width: f32,
    pub scroll_height: f32,
    pub padding: f32,
}
