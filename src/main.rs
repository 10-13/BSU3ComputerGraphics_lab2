use std::ops::Deref;
use eframe::egui;
use image::{DynamicImage, GenericImageView};
use std::sync::Arc;

fn rgb_to_hsv(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let r_ = r as f32 / 255.0;
    let g_ = g as f32 / 255.0;
    let b_ = b as f32 / 255.0;

    let c_max = r_.max(g_).max(b_);
    let c_min = r_.min(g_).min(b_);
    let delta = c_max - c_min;
    
    let hue = if delta == 0.0 {
        0.0
    } else if c_max == r_ {
        60.0 * (((g_ - b_) / delta) % 6.0)
    } else if c_max == g_ {
        60.0 * (((b_ - r_) / delta) + 2.0)
    } else { // c_max == b_
        60.0 * (((r_ - g_) / delta) + 4.0)
    };
    let h = if hue < 0.0 { hue + 360.0 } else { hue };
    
    let s = if c_max == 0.0 { 0.0 } else { delta / c_max };
    
    let v = c_max;

    (h, s, v)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r_, g_, b_) = if (0.0..60.0).contains(&h) {
        (c, x, 0.0)
    } else if (60.0..120.0).contains(&h) {
        (x, c, 0.0)
    } else if (120.0..180.0).contains(&h) {
        (0.0, c, x)
    } else if (180.0..240.0).contains(&h) {
        (0.0, x, c)
    } else if (240.0..300.0).contains(&h) {
        (x, 0.0, c)
    } else { // 300.0..360.0
        (c, 0.0, x)
    };

    let r = ((r_ + m) * 255.0) as u8;
    let g = ((g_ + m) * 255.0) as u8;
    let b = ((b_ + m) * 255.0) as u8;

    (r, g, b)
}

fn apply_linear_contrast(image: &DynamicImage) -> DynamicImage {
    let mut img = image.to_rgb8();
    
    let mut min_v: f32 = 1.0;
    let mut max_v: f32 = 0.0;

    for pixel in img.pixels() {
        let (_, _, v) = rgb_to_hsv(pixel[0], pixel[1], pixel[2]);
        min_v = min_v.min(v);
        max_v = max_v.max(v);
    }
    
    for pixel in img.pixels_mut() {
        let (h, s, mut v) = rgb_to_hsv(pixel[0], pixel[1], pixel[2]);
        
        if max_v > min_v {
            v = (v - min_v) / (max_v - min_v);
        }

        let (r, g, b) = hsv_to_rgb(h, s, v);
        pixel[0] = r;
        pixel[1] = g;
        pixel[2] = b;
    }

    DynamicImage::ImageRgb8(img)
}

fn apply_manual_threshold(image: &DynamicImage, threshold: u8) -> DynamicImage {
    let mut gray_image = image.to_luma8();
    
    for pixel in gray_image.pixels_mut() {
        if pixel[0] > threshold {
            pixel[0] = 255; // Белый
        } else {
            pixel[0] = 0;   // Черный
        }
    }

    DynamicImage::ImageLuma8(gray_image)
}

fn apply_otsu_threshold(image: &DynamicImage) -> DynamicImage {
    let gray_image = image.to_luma8();
    let pixels = gray_image.as_raw();
    
    let mut histogram = [0u64; 256];
    for &p in pixels {
        histogram[p as usize] += 1;
    }

    let total_pixels = pixels.len() as u64;
    if total_pixels == 0 {
        return image.clone();
    }
    
    let mut sum = 0.0;
    for (i, &h) in histogram.iter().enumerate() {
        sum += (i as f64) * (h as f64);
    }

    let mut sum_b = 0.0;
    let mut w_b = 0.0;
    let mut w_f;

    let mut max_variance = 0.0;
    let mut optimal_threshold = 0;

    for t in 0..256 {
        w_b += histogram[t] as f64;
        if w_b == 0.0 { continue; }

        w_f = (total_pixels as f64) - w_b;
        if w_f == 0.0 { break; }

        sum_b += (t as f64) * (histogram[t] as f64);

        let mean_b = sum_b / w_b;
        let mean_f = (sum - sum_b) / w_f;
        
        let variance = w_b * w_f * (mean_b - mean_f).powi(2);
        
        if variance > max_variance {
            max_variance = variance;
            optimal_threshold = t as u8;
        }
    }

    apply_manual_threshold(image, optimal_threshold)
}

fn apply_inversion(image: &DynamicImage) -> DynamicImage {
    let mut img = image.to_rgb8();
    for pixel in img.pixels_mut() {
        pixel[0] = 255 - pixel[0]; // R
        pixel[1] = 255 - pixel[1]; // G
        pixel[2] = 255 - pixel[2]; // B
    }
    DynamicImage::ImageRgb8(img)
}

fn apply_brightness(image: &DynamicImage, value: i16) -> DynamicImage {
    let mut img = image.to_rgb8();
    for pixel in img.pixels_mut() {
        for i in 0..3 {
            pixel[i] = ((pixel[i] as i16 + value).clamp(0, 255)) as u8;
        }
    }
    DynamicImage::ImageRgb8(img)
}

struct ImageApp {
    original_image: Option<Arc<DynamicImage>>,
    processed_image: Option<Arc<DynamicImage>>,
    original_texture: Option<egui::TextureHandle>,
    processed_texture: Option<egui::TextureHandle>,
    manual_threshold_value: u8,
    manual_brightness_value: i16,
}

impl Default for ImageApp {
    fn default() -> Self {
        Self {
            original_image: None,
            processed_image: None,
            original_texture: None,
            processed_texture: None,
            manual_threshold_value: 128,
            manual_brightness_value: 0,
        }
    }
}

/// Реализация основного цикла приложения
impl eframe::App for ImageApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Загрузить изображение").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        if let Ok(img) = image::open(path) {
                            let image_arc = Arc::new(img);
                            self.original_image = Some(image_arc.clone());
                            self.processed_image = Some(image_arc.clone()); // Сразу копируем для сброса
                            self.original_texture = None; // Сбрасываем текстуры, чтобы они пересоздались
                            self.processed_texture = None;
                        }
                    }
                }

                let has_image = self.processed_image.is_some();

                ui.add_enabled_ui(has_image, |ui| {
                    if ui.button("Сохранить результат").clicked() {
                        if let Some(image) = &self.processed_image {
                            if let Some(path) = rfd::FileDialog::new().save_file() {
                                // Добавляем расширение, если его нет
                                let path = if path.extension().is_none() {
                                    path.with_extension("png")
                                } else {
                                    path
                                };
                                let _ = image.save(path);
                            }
                        }
                    }

                    if ui.button("Сбросить").clicked() {
                        if let Some(original) = &self.original_image {
                            self.processed_image = Some(original.clone());
                            self.processed_texture = None; // Сброс для пересоздания
                        }
                    }
                });
            });

            ui.separator();

            let main_rect = ui.available_rect_before_wrap();
            let image_width = main_rect.width() / 2.0 - ui.spacing().item_spacing.x;

            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Оригинал");
                    if let Some(original) = &self.original_image {
                        let texture = self.original_texture.get_or_insert_with(|| {
                            image_to_texture(original, "original", ctx)
                        });
                        ui.image(texture.deref());
                    } else {
                        ui.label("(изображение не загружено)");
                    }
                });

                ui.vertical(|ui| {
                    ui.label("Результат");
                    if let Some(processed) = &self.processed_image {
                        let texture = self.processed_texture.get_or_insert_with(|| {
                            image_to_texture(processed, "processed", ctx)
                        });
                        ui.image(texture.deref());
                    } else {
                        ui.label("(изображение не загружено)");
                    }
                });
            });

            ui.separator();

            // --- Панель с кнопками алгоритмов ---
            ui.add_enabled_ui(self.original_image.is_some(), |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Линейное контрастирование").clicked() {
                        if let Some(original) = &self.original_image {
                            let result = apply_linear_contrast(original);
                            self.processed_image = Some(Arc::new(result));
                            self.processed_texture = None;
                        }
                    }

                    if ui.button("Порог (метод Оцу)").clicked() {
                        if let Some(original) = &self.original_image {
                            let result = apply_otsu_threshold(original);
                            self.processed_image = Some(Arc::new(result));
                            self.processed_texture = None;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut self.manual_threshold_value, 0..=255).text("Ручной порог"));
                    if ui.button("Применить").clicked() {
                        if let Some(original) = &self.original_image {
                            let result = apply_manual_threshold(original, self.manual_threshold_value);
                            self.processed_image = Some(Arc::new(result));
                            self.processed_texture = None;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    if ui.button("Инверсия").clicked() {
                        if let Some(original) = &self.original_image {
                            let result = apply_inversion(original);
                            self.processed_image = Some(Arc::new(result));
                            self.processed_texture = None;
                        }
                    }
                    ui.add(egui::Slider::new(&mut self.manual_brightness_value, -255..=255).text("Ручной порог"));
                    if ui.button("Яркость").clicked() {
                        if let Some(original) = &self.original_image {
                            let result = apply_brightness(original, self.manual_brightness_value);
                            self.processed_image = Some(Arc::new(result));
                            self.processed_texture = None;
                        }
                    }
                });
            });
        });
    }
}

/// Вспомогательная функция для конвертации `DynamicImage` в `egui::TextureHandle`
fn image_to_texture(image: &DynamicImage, name: &'static str, ctx: &egui::Context) -> egui::TextureHandle {
    let (width, height) = image.dimensions();
    let rgba_image = image.to_rgba8();
    let pixels = rgba_image.into_raw();

    let egui_image = egui::ColorImage::from_rgba_unmultiplied([width as usize, height as usize], &pixels);

    ctx.load_texture(name, egui_image, Default::default())
}


fn main() {
    let native_options = eframe::NativeOptions::default();
    let _ = eframe::run_native(
        "Лабораторная работа №2",
        native_options,
        Box::new(|_cc| Ok(Box::<ImageApp>::default())),
    );
}