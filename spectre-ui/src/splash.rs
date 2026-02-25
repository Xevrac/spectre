use eframe::egui;
use std::time::{Duration, Instant};

pub struct SplashScreen {
    start_time: Instant,
    banner_texture: Option<egui::TextureHandle>,
    fade_duration: Duration,
    show_duration: Duration,
}

impl SplashScreen {
    pub fn is_fading_out(&self) -> bool {
        let elapsed = self.start_time.elapsed();
        elapsed >= self.show_duration && elapsed < self.show_duration + self.fade_duration
    }
    
    pub fn get_fade_out_alpha(&self) -> f32 {
        let elapsed = self.start_time.elapsed();
        if elapsed >= self.show_duration {
            let fade_out_start = self.show_duration;
            let fade_out_end = self.show_duration + self.fade_duration;
            if elapsed < fade_out_end {
                return 1.0 - ((elapsed - fade_out_start).as_secs_f32() / self.fade_duration.as_secs_f32());
            }
        }
        0.0
    }
}

impl SplashScreen {
    pub fn new(ctx: &egui::Context) -> Self {
        println!("[Spectre.dbg] Initializing splash screen...");
        let mut splash = Self {
            start_time: Instant::now(),
            banner_texture: None,
            fade_duration: Duration::from_millis(800),
            show_duration: Duration::from_millis(2000),
        };
        
        splash.load_banner(ctx);
        println!("[Spectre.dbg] Splash screen initialized (fade: {}ms, show: {}ms)", 
                 splash.fade_duration.as_millis(), splash.show_duration.as_millis());
        splash
    }
    
    fn load_banner(&mut self, ctx: &egui::Context) {
        let banner_bytes = include_bytes!("../spectre-banner.png");
        
        if let Ok(image) = image::load_from_memory(banner_bytes) {
            let rgba = image.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let pixels = rgba.as_flat_samples();
            
            println!("[Spectre.dbg] Banner image loaded: {}x{}", size[0], size[1]);
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            self.banner_texture = Some(ctx.load_texture("banner", color_image, Default::default()));
            println!("[Spectre.dbg] Banner texture created");
        } else {
            println!("[Spectre.dbg] Warning: Failed to load banner image from embedded bytes");
        }
    }
    
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let elapsed = self.start_time.elapsed();
        let fade_progress = if elapsed < self.fade_duration {
            elapsed.as_secs_f32() / self.fade_duration.as_secs_f32()
        } else if elapsed < self.show_duration {
            1.0
        } else {
            let fade_out_start = self.show_duration;
            let fade_out_end = self.show_duration + self.fade_duration;
            if elapsed < fade_out_end {
                1.0 - ((elapsed - fade_out_start).as_secs_f32() / self.fade_duration.as_secs_f32())
            } else {
                return false; // Splash screen is done
            }
        };
        
        egui::Area::new(egui::Id::new("splash_area"))
            .interactable(false)
            .show(ctx, |ui| {
                let screen_rect = ctx.screen_rect();
                let painter = ui.painter();
                
                painter.rect_filled(
                    screen_rect,
                    0.0,
                    egui::Color32::BLACK,
                );
                
                if let Some(ref texture) = self.banner_texture {
                    let banner_size = texture.size();
                    let banner_size_vec = egui::vec2(banner_size[0] as f32 / 2.0 * 1.03, banner_size[1] as f32 / 2.0 * 1.03);
                    let banner_rect = egui::Rect::from_center_size(
                        screen_rect.center(),
                        banner_size_vec,
                    );
                    
                    painter.image(
                        texture.id(),
                        banner_rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::from_white_alpha((255.0 * fade_progress) as u8),
                    );
                    
                    let loader_y = banner_rect.bottom() - 20.0;
                    let loader_x = banner_rect.center().x + 10.0;
                    self.draw_orbit_loader(ui, loader_x, loader_y, fade_progress);
                } else {
                    self.draw_orbit_loader(ui, screen_rect.center().x, screen_rect.center().y, fade_progress);
                }
            });
        
        ctx.request_repaint();
        true
    }
    
    fn draw_orbit_loader(&self, ui: &egui::Ui, center_x: f32, center_y: f32, alpha: f32) {
        let painter = ui.painter();
        let time = self.start_time.elapsed().as_secs_f32();
        let size = 17.5;
        let speed = 1.5;
        let dot_size_base = size * 0.4;
        
        let progress1 = (time * speed) % 1.0;
        let mut progress2 = (time * speed - 0.5) % 1.0;
        if progress2 < 0.0 {
            progress2 += 1.0;
        }
        
        let get_transform = |progress: f32| -> (f32, f32, f32) {
            let (translate_x, scale, opacity) = if progress <= 0.25 {
                let t = progress / 0.25;
                let translate_x = size * 0.25 * (1.0 - t);
                let scale = 0.73684 + (0.47368 - 0.73684) * t;
                let opacity = 0.65 + (0.3 - 0.65) * t;
                (translate_x, scale, opacity)
            } else if progress <= 0.5 {
                let t = (progress - 0.25) / 0.25;
                let translate_x = size * -0.25 * t;
                let scale = 0.47368 + (0.73684 - 0.47368) * t;
                let opacity = 0.3 + (0.65 - 0.3) * t;
                (translate_x, scale, opacity)
            } else if progress <= 0.75 {
                let t = (progress - 0.5) / 0.25;
                let translate_x = size * -0.25 * (1.0 - t);
                let scale = 0.73684 + (1.0 - 0.73684) * t;
                let opacity = 0.65 + (1.0 - 0.65) * t;
                (translate_x, scale, opacity)
            } else {
                let t = (progress - 0.75) / 0.25;
                let translate_x = size * 0.25 * t;
                let scale = 1.0 + (0.73684 - 1.0) * t;
                let opacity = 1.0 + (0.65 - 1.0) * t;
                (translate_x, scale, opacity)
            };
            (translate_x, scale, opacity)
        };
        
        let (tx1, scale1, opacity1) = get_transform(progress1);
        let x1 = center_x + tx1;
        let dot_size1 = dot_size_base * scale1;
        let base_color = 200u8;
        let color1 = egui::Color32::from_rgba_unmultiplied(
            base_color,
            base_color,
            base_color,
            ((255.0 * opacity1 * alpha) as u8).min(255),
        );
        painter.circle_filled(
            egui::pos2(x1, center_y),
            dot_size1 / 2.0,
            color1,
        );
        
        let (tx2, scale2, opacity2) = get_transform(progress2);
        let x2 = center_x + tx2;
        let dot_size2 = dot_size_base * scale2;
        let color2 = egui::Color32::from_rgba_unmultiplied(
            base_color,
            base_color,
            base_color,
            ((255.0 * opacity2 * alpha) as u8).min(255),
        );
        painter.circle_filled(
            egui::pos2(x2, center_y),
            dot_size2 / 2.0,
            color2,
        );
    }
}

