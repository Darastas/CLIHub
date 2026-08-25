//! 字体加载与全局 Visuals 视觉样式配置。

use egui::Color32;

/// 加载字体：内嵌 JetBrains Mono（终端等宽，含加粗族）+ 艺术标题字体 + 系统中英文字体回退链。
pub fn setup_fonts(ctx: &egui::Context) {
    use egui::FontFamily;

    let mut fonts = egui::FontDefinitions::default();
    let arc = |data: &'static [u8]| std::sync::Arc::new(egui::FontData::from_static(data));

    // 内嵌 JetBrains Mono（Regular + Bold）
    fonts.font_data.insert(
        "jbmono".into(),
        arc(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")),
    );
    fonts.font_data.insert(
        "jbmono-bold".into(),
        arc(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")),
    );
    // 优雅艺术标题字体 (Playfair Display)
    fonts.font_data.insert(
        "title_font".into(),
        arc(include_bytes!("../assets/fonts/PlayfairDisplay.ttf")),
    );

    // 自定义加粗族（供终端粗体字形使用）
    fonts
        .families
        .insert(FontFamily::Name("jbmono-bold".into()), vec!["jbmono-bold".into()]);

    // 自定义标题族
    fonts
        .families
        .insert(FontFamily::Name("title".into()), vec!["title_font".into()]);
    // 终端等宽族：JetBrains Mono 打头
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.insert(0, "jbmono".into());
    }

    let load = |path: &str| -> Option<(String, Vec<u8>)> {
        std::fs::read(path).ok().map(|data| {
            let name = path
                .split(['\\', '/'])
                .last()
                .unwrap_or("font")
                .to_owned();
            (name, data)
        })
    };

    // Nerd Font（Powerline / 图标字形）—— 从用户字体目录加载
    let user_fonts = format!(
        r"{}\AppData\Local\Microsoft\Windows\Fonts",
        std::env::var("USERPROFILE").unwrap_or_default()
    );
    for nf_file in [
        "JetBrainsMonoNerdFontMono-Regular.ttf",
        "JetBrainsMonoNerdFontMono-Bold.ttf",
    ] {
        let nf_path = format!(r"{}\{}", user_fonts, nf_file);
        if let Some((name, data)) = load(&nf_path) {
            fonts
                .font_data
                .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
                mono.push(name.clone());
            }
            if nf_file.contains("Bold") {
                if let Some(bold) = fonts.families.get_mut(&FontFamily::Name("jbmono-bold".into())) {
                    bold.push(name);
                }
            }
        }
    }
    // 从系统字体目录加载
    for nf_file in [
        "JetBrainsMonoNerdFontMono-Regular.ttf",
        "JetBrainsMonoNerdFontMono-Bold.ttf",
    ] {
        let nf_path = format!(r"C:\Windows\Fonts\{}", nf_file);
        if !fonts.font_data.contains_key(nf_file) {
            if let Some((name, data)) = load(&nf_path) {
                fonts
                    .font_data
                    .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
                if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
                    mono.push(name.clone());
                }
                if nf_file.contains("Bold") {
                    if let Some(bold) = fonts.families.get_mut(&FontFamily::Name("jbmono-bold".into())) {
                        bold.push(name);
                    }
                }
            }
        }
    }

    // UI 拉丁字体：Segoe UI 放最前
    if let Some((name, data)) = load(r"C:\Windows\Fonts\segoeui.ttf") {
        fonts
            .font_data
            .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        if let Some(prop) = fonts.families.get_mut(&FontFamily::Proportional) {
            prop.insert(0, name);
        }
    }
    // 终端等宽兜底：Consolas
    if let Some((name, data)) = load(r"C:\Windows\Fonts\consola.ttf") {
        fonts
            .font_data
            .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
            mono.push(name);
        }
    }
    // CJK 兜底链：微软雅黑 → DengXian → SimHei
    for path in [
        r"C:\Windows\Fonts\msyh.ttc",
        r"C:\Windows\Fonts\Deng.ttf",
        r"C:\Windows\Fonts\simhei.ttf",
    ] {
        if let Some((name, data)) = load(path) {
            fonts
                .font_data
                .insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            for family in [FontFamily::Monospace, FontFamily::Proportional] {
                if let Some(list) = fonts.families.get_mut(&family) {
                    list.push(name.clone());
                }
            }
        }
    }
    ctx.set_fonts(fonts);
}

/// 配置全局视觉风格（圆角、无侵入边框、阴影）。
pub fn app_visuals(dark: bool) -> egui::Visuals {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    v.widgets.noninteractive.corner_radius = egui::CornerRadius::same(14);
    v.widgets.inactive.corner_radius = egui::CornerRadius::same(14);
    v.widgets.hovered.corner_radius = egui::CornerRadius::same(14);
    v.widgets.active.corner_radius = egui::CornerRadius::same(14);
    v.window_corner_radius = egui::CornerRadius::same(16);

    v.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
    v.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    v.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    v.widgets.active.bg_stroke = egui::Stroke::NONE;

    if dark {
        v.panel_fill = Color32::from_rgb(24, 24, 27);
        v.window_fill = Color32::from_rgb(9, 9, 11);
        v.selection.bg_fill = Color32::from_rgb(0, 111, 238);
        v.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);

        v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
        v.widgets.hovered.bg_fill = Color32::from_rgb(39, 39, 42);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(39, 39, 42);
        v.widgets.active.bg_fill = Color32::from_rgb(63, 63, 70);
        v.widgets.noninteractive.bg_fill = Color32::from_rgb(24, 24, 27);

        v.window_stroke = egui::Stroke::NONE;
    } else {
        v.panel_fill = Color32::from_rgb(250, 250, 250);
        v.window_fill = Color32::from_rgb(255, 255, 255);
        v.selection.bg_fill = Color32::from_rgb(0, 111, 238);
        v.selection.stroke = egui::Stroke::new(1.0, Color32::WHITE);

        v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
        v.widgets.hovered.bg_fill = Color32::from_rgb(228, 228, 231);
        v.widgets.hovered.weak_bg_fill = Color32::from_rgb(228, 228, 231);
        v.widgets.active.bg_fill = Color32::from_rgb(212, 212, 216);
        v.widgets.noninteractive.bg_fill = Color32::from_rgb(250, 250, 250);

        v.window_stroke = egui::Stroke::NONE;
    }

    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 12],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 120 } else { 40 }),
    };
    v
}
