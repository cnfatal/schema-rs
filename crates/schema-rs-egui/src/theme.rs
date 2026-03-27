use egui::{
    Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, Style, TextStyle,
    Visuals,
};

/// Apply the default schema-rs theme (Material Dark) and CJK fonts to the given egui context.
pub fn apply_default_theme(ctx: &egui::Context) {
    ctx.set_style(material_dark_style());
    ctx.set_fonts(default_fonts());
}

/// Material Design 3 dark theme style.
pub fn material_dark_style() -> Style {
    let mut style = Style::default();

    // Typography
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(20.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));

    // Spacing
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size = egui::vec2(40.0, 24.0);

    // Visuals
    let mut visuals = Visuals::dark();
    let primary = Color32::from_rgb(130, 177, 255);
    let surface = Color32::from_rgb(30, 30, 36);
    let surface_variant = Color32::from_rgb(44, 44, 52);
    let on_surface = Color32::from_rgb(230, 225, 229);
    let on_surface_variant = Color32::from_rgb(196, 192, 197);
    let outline = Color32::from_rgb(73, 69, 79);
    let outline_variant = Color32::from_rgb(55, 52, 61);
    let error = Color32::from_rgb(242, 184, 181);

    let corner = CornerRadius::same(8);

    // Window / panel
    visuals.window_fill = surface;
    visuals.panel_fill = Color32::from_rgb(25, 25, 30);
    visuals.window_corner_radius = CornerRadius::same(16);
    visuals.window_stroke = Stroke::new(1.0, outline_variant);

    // Selection
    visuals.selection.bg_fill = primary.linear_multiply(0.3);
    visuals.selection.stroke = Stroke::new(1.0, primary);

    // Inactive
    visuals.widgets.inactive.bg_fill = surface_variant;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, outline);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, on_surface_variant);
    visuals.widgets.inactive.corner_radius = corner;

    // Hovered
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(54, 54, 62);
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, primary);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, on_surface);
    visuals.widgets.hovered.corner_radius = corner;

    // Active
    visuals.widgets.active.bg_fill = primary.linear_multiply(0.25);
    visuals.widgets.active.bg_stroke = Stroke::new(1.5, primary);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5, primary);
    visuals.widgets.active.corner_radius = corner;

    // Non-interactive
    visuals.widgets.noninteractive.bg_fill = surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, outline_variant);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, on_surface);
    visuals.widgets.noninteractive.corner_radius = corner;

    // Open
    visuals.widgets.open.bg_fill = surface_variant;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0, primary);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0, primary);
    visuals.widgets.open.corner_radius = corner;

    visuals.error_fg_color = error;
    visuals.hyperlink_color = primary;

    style.visuals = visuals;
    style
}

/// Default font definitions with CJK support.
pub fn default_fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    let cjk_font_paths = [
        "/System/Library/Fonts/Hiragino Sans GB.ttc", // macOS
        "/System/Library/Fonts/STHeiti Medium.ttc",   // macOS fallback
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", // Linux
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", // Linux alt
        "C:\\Windows\\Fonts\\msyh.ttc",               // Windows
    ];

    for path in &cjk_font_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts.font_data.insert(
                "cjk".to_owned(),
                std::sync::Arc::new(FontData::from_owned(data)),
            );
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .push("cjk".to_owned());
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .push("cjk".to_owned());
            break;
        }
    }

    fonts
}
