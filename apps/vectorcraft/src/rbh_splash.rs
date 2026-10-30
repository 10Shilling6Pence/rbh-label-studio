//! RBH startup artwork. This is rendered inside the existing editor window.
//! Earlier graphics/device setup finishes before egui can draw; do not claim to
//! report those stages while showing the overlay.

#[derive(Default)]
pub(crate) struct Splash {
    artwork: Option<egui::TextureHandle>,
    started_at: Option<std::time::Instant>,
    frames_rendered: u32,
}

impl Splash {
    /// Keep the branded overlay visible for at least three seconds after its
    /// first rendered frame. Never hold up actual editor initialization.
    pub(crate) fn show(&mut self, ctx: &egui::Context) {
        let started_at = *self.started_at.get_or_insert_with(std::time::Instant::now);
        if started_at.elapsed() >= std::time::Duration::from_secs(3) && self.frames_rendered > 0 {
            return;
        }
        if self.artwork.is_none() {
            let data = include_bytes!("rbh-monogram.png");
            match image::load_from_memory(data) {
                Ok(image) => {
                    let rgba = image.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let color = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                    self.artwork = Some(ctx.load_texture("rbh_startup_monogram", color, egui::TextureOptions::LINEAR));
                }
                Err(error) => log::warn!("RBH startup artwork unavailable: {error}"),
            }
        }

        // Keep the splash independent from the editor's window size: a
        // traditional compact software splash, centered over the workspace.
        // On smaller displays, shrink it while retaining an even outer margin.
        let available = ctx.content_rect();
        let panel_size = egui::vec2(
            680.0_f32.min((available.width() - 32.0).max(1.0)),
            500.0_f32.min((available.height() - 32.0).max(1.0)),
        );
        let screen = egui::Rect::from_center_size(available.center(), panel_size);
        egui::Area::new(egui::Id::unique("rbh_startup_overlay"))
            .order(egui::Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                ui.set_min_size(screen.size());
                let painter = ui.painter();
                let black = egui::Color32::from_rgb(13, 13, 17);
                let red = egui::Color32::from_rgb(234, 11, 86);
                let white = egui::Color32::from_rgb(239, 236, 240);
                let muted = egui::Color32::from_rgb(178, 167, 178);
                painter.rect_filled(screen, 0.0, black);

                // The border is drawn, not part of the bitmap. Its inset stays
                // consistent when the main window is resized or DPI changes.
                let inset = (screen.width().min(screen.height()) * 0.027).clamp(12.0, 29.0);
                let border = screen.shrink(inset);
                painter.rect_stroke(
                    border,
                    egui::CornerRadius::same(24),
                    egui::Stroke::new(2.0, red),
                    egui::StrokeKind::Inside,
                );
                let center = screen.center();
                let scale = (screen.width().min(screen.height()) / 750.0).clamp(0.48, 1.30);
                let logo_side = 280.0 * scale;
                let logo = egui::Rect::from_center_size(
                    egui::pos2(center.x, center.y - 75.0 * scale),
                    egui::vec2(logo_side, logo_side),
                );
                if let Some(image) = &self.artwork {
                    painter.image(
                        image.id(), logo,
                        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                }
                painter.text(
                    egui::pos2(center.x, center.y + 125.0 * scale),
                    egui::Align2::CENTER_CENTER,
                    "RBH LABEL STUDIO",
                    egui::FontId::proportional(30.0 * scale), white,
                );
                painter.text(
                    egui::pos2(center.x, center.y + 161.0 * scale),
                    egui::Align2::CENTER_CENTER,
                    "L U X U R Y   S T A N D A R D",
                    egui::FontId::proportional(13.0 * scale), muted,
                );

                // This is the actual current milestone. Device creation, font
                // registration and preference loading occurred before any egui
                // overlay could be displayed and are deliberately not fabricated.
                let bottom_y = border.max.y - 53.0 * scale;
                let start = egui::pos2(border.min.x + 35.0 * scale, bottom_y);
                painter.text(
                    start, egui::Align2::LEFT_CENTER,
                    if self.frames_rendered == 0 { "Rendering editor workspace..." } else { "Ready" },
                    egui::FontId::proportional(11.0 * scale), white,
                );
                painter.text(
                    egui::pos2(border.max.x - 35.0 * scale, bottom_y),
                    egui::Align2::RIGHT_CENTER,
                    concat!("v", env!("CARGO_PKG_VERSION"), "  |  RBH Edition"),
                    egui::FontId::proportional(11.0 * scale), muted,
                );
                // A short neutral rule is decorative, not a fabricated % meter.
                let rule_y = bottom_y - 23.0 * scale;
                painter.line_segment(
                    [egui::pos2(start.x, rule_y), egui::pos2(border.max.x - 35.0 * scale, rule_y)],
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(72, 54, 63)),
                );
            });
        self.frames_rendered = self.frames_rendered.saturating_add(1);
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}

