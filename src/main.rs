use std::{
    collections::{BTreeSet, HashMap},
    fs,
    path::PathBuf,
};

use clap::Parser;
use eframe::egui::{
    self, Align, FontFamily, FontId, Key, RichText, TextStyle, TopBottomPanel, ViewportBuilder,
};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use fontdb::{Database, Source, Style as FontStyle};
use rfd::FileDialog;

#[derive(Parser, Debug)]
#[command(author, version, about = "Lightweight Markdown viewer for Linux")]
struct Args {
    /// Optional markdown file path to open at startup
    path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Light,
    Dark,
    Dracula,
}

struct MarkdownViewerApp {
    cache: CommonMarkCache,
    markdown_raw: String,
    markdown_rendered: String,
    current_path: Option<PathBuf>,
    load_error: Option<String>,
    theme: ThemeMode,
    body_font: String,
    mono_font: String,
    font_size: f32,
    zoom: f32,
    center_content: bool,
    constrain_width: bool,
    content_width: f32,
    system_fonts: Vec<String>,
    font_faces: HashMap<String, Vec<FaceCandidate>>,
    toc_open: bool,
    scroll_to_section: Option<usize>,
}

enum MarkdownBlock {
    Markdown(String),
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
}

struct Section {
    title: String,
    content: String,
    level: u8,
}

#[derive(Clone)]
struct FaceCandidate {
    path: PathBuf,
    style: FontStyle,
    weight: u16,
    monospaced: bool,
}

impl MarkdownViewerApp {
    fn new(cc: &eframe::CreationContext<'_>, initial_path: Option<PathBuf>) -> Self {
        let (system_fonts, font_faces) = discover_system_fonts();
        let body_font = choose_font(&system_fonts, "Adwaita Sans");
        let mono_font = choose_font(&system_fonts, "Source Code Pro");

        let mut app = Self {
            cache: CommonMarkCache::default(),
            markdown_raw: "# Markdown Viewer\n\nOpen a `.md` file to begin.".to_owned(),
            markdown_rendered: "# Markdown Viewer\n\nOpen a `.md` file to begin.".to_owned(),
            current_path: None,
            load_error: None,
            theme: ThemeMode::Dracula,
            body_font,
            mono_font,
            font_size: 17.0,
            zoom: 1.0,
            center_content: true,
            constrain_width: true,
            content_width: 860.0,
            system_fonts,
            font_faces,
            toc_open: true,
            scroll_to_section: None,
        };

        app.apply_theme(&cc.egui_ctx);
        app.apply_font_definitions(&cc.egui_ctx);

        if let Some(path) = initial_path {
            app.open_path(path);
        }

        app
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        match self.theme {
            ThemeMode::Light => ctx.set_visuals(egui::Visuals::light()),
            ThemeMode::Dark => ctx.set_visuals(egui::Visuals::dark()),
            ThemeMode::Dracula => {
                let mut visuals = egui::Visuals::dark();
                visuals.override_text_color = Some(egui::Color32::from_rgb(248, 248, 242));
                visuals.panel_fill = egui::Color32::from_rgb(40, 42, 54);
                visuals.window_fill = egui::Color32::from_rgb(40, 42, 54);
                visuals.extreme_bg_color = egui::Color32::from_rgb(33, 34, 44);
                visuals.faint_bg_color = egui::Color32::from_rgb(68, 71, 90);
                visuals.widgets.noninteractive.fg_stroke.color =
                    egui::Color32::from_rgb(248, 248, 242);
                visuals.widgets.inactive.fg_stroke.color = egui::Color32::from_rgb(248, 248, 242);
                visuals.widgets.hovered.fg_stroke.color = egui::Color32::from_rgb(248, 248, 242);
                visuals.widgets.active.fg_stroke.color = egui::Color32::from_rgb(248, 248, 242);
                visuals.hyperlink_color = egui::Color32::from_rgb(139, 233, 253);
                visuals.selection.bg_fill = egui::Color32::from_rgb(98, 114, 164);
                visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(40, 42, 54);
                visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(68, 71, 90);
                visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(98, 114, 164);
                visuals.widgets.active.bg_fill = egui::Color32::from_rgb(80, 250, 123);
                ctx.set_visuals(visuals);
            }
        }
    }

    fn apply_font_definitions(&mut self, ctx: &egui::Context) {
        let mut defs = egui::FontDefinitions::default();
        if let Some(path) = self.select_font_file(&self.body_font, false)
            && let Ok(bytes) = fs::read(path)
        {
            let font_key = "user_body_font".to_owned();
            defs.font_data
                .insert(font_key.clone(), egui::FontData::from_owned(bytes).into());
            defs.families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, font_key);
        }

        if let Some(path) = self.select_font_file(&self.mono_font, true)
            && let Ok(bytes) = fs::read(path)
        {
            let font_key = "user_mono_font".to_owned();
            defs.font_data
                .insert(font_key.clone(), egui::FontData::from_owned(bytes).into());
            defs.families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, font_key);
        }
        ctx.set_fonts(defs);
    }

    fn select_font_file(&self, family_name: &str, prefer_mono: bool) -> Option<&PathBuf> {
        let faces = self.font_faces.get(family_name)?;
        let candidate = faces.iter().min_by_key(|face| {
            let mut score: i32 = 0;
            score += match face.style {
                FontStyle::Normal => 0,
                FontStyle::Oblique => 40,
                FontStyle::Italic => 60,
            };
            score += (i32::from(face.weight) - 400).abs();
            if prefer_mono {
                if !face.monospaced {
                    score += 80;
                }
            } else if face.monospaced {
                score += 20;
            }
            score
        })?;
        Some(&candidate.path)
    }

    fn markdown_style(&self, ctx: &egui::Context) -> egui::Style {
        let scale = self.font_size * self.zoom;
        let body_family = FontFamily::Proportional;
        let mono_family = FontFamily::Monospace;

        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (
                TextStyle::Heading,
                FontId::new(scale * 1.45, body_family.clone()),
            ),
            (
                TextStyle::Name("Heading2".into()),
                FontId::new(scale * 1.25, body_family.clone()),
            ),
            (
                TextStyle::Name("Context".into()),
                FontId::new(scale, body_family.clone()),
            ),
            (TextStyle::Body, FontId::new(scale, body_family.clone())),
            (TextStyle::Monospace, FontId::new(scale * 0.95, mono_family)),
            (
                TextStyle::Button,
                FontId::new(scale * 0.9, body_family.clone()),
            ),
            (TextStyle::Small, FontId::new(scale * 0.85, body_family)),
        ]
        .into();
        style
    }

    fn open_path(&mut self, path: PathBuf) {
        match fs::read_to_string(&path) {
            Ok(content) => {
                self.markdown_rendered = normalize_markdown_for_viewer(&content);
                self.markdown_raw = content;
                self.current_path = Some(path);
                self.load_error = None;
            }
            Err(err) => {
                self.load_error = Some(format!("Failed to open file: {err}"));
            }
        }
    }

    fn open_dialog(&mut self) {
        if let Some(path) = FileDialog::new()
            .add_filter(
                "Markdown",
                &["md", "markdown", "mdown", "mkd", "mkdn", "txt"],
            )
            .pick_file()
        {
            self.open_path(path);
        }
    }
}

impl eframe::App for MarkdownViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::O)) {
            self.open_dialog();
        }
        if ctx
            .input(|i| i.modifiers.ctrl && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)))
        {
            self.zoom = (self.zoom + 0.1).min(3.0);
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::Minus)) {
            self.zoom = (self.zoom - 0.1).max(0.5);
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(Key::Num0)) {
            self.zoom = 1.0;
        }

        TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button(if self.toc_open { "◧" } else { "☰" })
                    .on_hover_text("Toggle sections panel")
                    .clicked()
                {
                    self.toc_open = !self.toc_open;
                }
                if ui.button("Open").clicked() {
                    self.open_dialog();
                }
                if ui.button("Reload").clicked() {
                    if let Some(path) = self.current_path.clone() {
                        self.open_path(path);
                    }
                }
                ui.separator();

                ui.label("Theme");
                egui::ComboBox::from_id_salt("theme_combo")
                    .selected_text(match self.theme {
                        ThemeMode::Light => "Light",
                        ThemeMode::Dark => "Dark",
                        ThemeMode::Dracula => "Dracula",
                    })
                    .show_ui(ui, |ui| {
                        for mode in [ThemeMode::Light, ThemeMode::Dark, ThemeMode::Dracula] {
                            let label = match mode {
                                ThemeMode::Light => "Light",
                                ThemeMode::Dark => "Dark",
                                ThemeMode::Dracula => "Dracula",
                            };
                            if ui.selectable_value(&mut self.theme, mode, label).changed() {
                                self.apply_theme(ctx);
                            }
                        }
                    });

                ui.separator();
                ui.label("Body");
                egui::ComboBox::from_id_salt("body_font_combo")
                    .selected_text(self.body_font.clone())
                    .show_ui(ui, |ui| {
                        let fonts = self.system_fonts.clone();
                        for font in fonts {
                            if ui
                                .selectable_value(&mut self.body_font, font.clone(), font)
                                .changed()
                            {
                                self.apply_font_definitions(ctx);
                            }
                        }
                    });

                ui.separator();
                ui.label("Code");
                egui::ComboBox::from_id_salt("mono_font_combo")
                    .selected_text(self.mono_font.clone())
                    .show_ui(ui, |ui| {
                        let fonts = self.system_fonts.clone();
                        for font in fonts {
                            if ui
                                .selectable_value(&mut self.mono_font, font.clone(), font)
                                .changed()
                            {
                                self.apply_font_definitions(ctx);
                            }
                        }
                    });

                ui.separator();
                ui.label("Font Size");
                ui.add(egui::Slider::new(&mut self.font_size, 12.0..=30.0));

                ui.separator();
                if ui.button("−").clicked() {
                    self.zoom = (self.zoom - 0.1).max(0.5);
                }
                ui.label(format!("Zoom: {}%", (self.zoom * 100.0).round() as i32));
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom + 0.1).min(3.0);
                }
                if ui.button("Reset").clicked() {
                    self.zoom = 1.0;
                }

                ui.separator();
                ui.checkbox(&mut self.center_content, "Center");
                ui.checkbox(&mut self.constrain_width, "Max Width");
                ui.add_enabled(
                    self.constrain_width,
                    egui::Slider::new(&mut self.content_width, 520.0..=1800.0).text("Width"),
                );
            });
        });

        let sections = split_markdown_sections(&self.markdown_rendered);
        if self.toc_open {
            egui::SidePanel::left("toc_panel")
                .resizable(true)
                .default_width(230.0)
                .min_width(180.0)
                .show(ctx, |ui| {
                    ui.heading("Sections");
                    ui.separator();
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for (idx, section) in sections.iter().enumerate() {
                            let indent = f32::from(section.level.saturating_sub(1)) * 12.0;
                            ui.horizontal(|ui| {
                                ui.add_space(indent);
                                if ui.button(&section.title).clicked() {
                                    self.scroll_to_section = Some(idx);
                                }
                            });
                        }
                    });
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(path) = &self.current_path {
                ui.label(
                    RichText::new(path.display().to_string())
                        .italics()
                        .weak()
                        .size(13.0),
                );
                ui.separator();
            }

            if let Some(err) = &self.load_error {
                ui.colored_label(egui::Color32::RED, err);
                ui.separator();
            }

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let available = ui.available_width();
                    let target_width = if self.constrain_width {
                        self.content_width.min(available)
                    } else {
                        available
                    };
                    let markdown_style = self.markdown_style(ctx);
                    let mut section_counter = 0usize;

                    if self.center_content && target_width < available {
                        let side = ((available - target_width) * 0.5).max(0.0);
                        ui.horizontal(|ui| {
                            ui.add_space(side);
                            ui.allocate_ui_with_layout(
                                egui::vec2(target_width, 0.0),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    ui.set_style(markdown_style);
                                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                    render_sections(
                                        ui,
                                        &mut self.cache,
                                        &sections,
                                        target_width,
                                        &mut self.scroll_to_section,
                                        &mut section_counter,
                                        "centered",
                                    );
                                },
                            );
                        });
                    } else {
                        ui.allocate_ui_with_layout(
                            egui::vec2(target_width, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.set_style(markdown_style);
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                render_sections(
                                    ui,
                                    &mut self.cache,
                                    &sections,
                                    target_width,
                                    &mut self.scroll_to_section,
                                    &mut section_counter,
                                    "normal",
                                );
                            },
                        );
                    }
                });
        });
    }
}

fn split_markdown_sections(markdown: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current_title = "Document".to_owned();
    let mut current_level: u8 = 1;
    let mut current = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim_start();
        let heading = if let Some(rest) = trimmed.strip_prefix("# ") {
            Some((1u8, rest))
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            Some((2u8, rest))
        } else if let Some(rest) = trimmed.strip_prefix("### ") {
            Some((3u8, rest))
        } else if let Some(rest) = trimmed.strip_prefix("#### ") {
            Some((4u8, rest))
        } else {
            None
        };

        if let Some((level, rest)) = heading {
            if !current.is_empty() {
                sections.push(Section {
                    title: current_title,
                    content: current.join("\n"),
                    level: current_level,
                });
                current.clear();
            }
            current_title = rest.trim().to_owned();
            current_level = level;
        }
        current.push(line.to_owned());
    }

    if !current.is_empty() {
        sections.push(Section {
            title: current_title,
            content: current.join("\n"),
            level: current_level,
        });
    }

    if sections.is_empty() {
        sections.push(Section {
            title: "Document".to_owned(),
            content: markdown.to_owned(),
            level: 1,
        });
    }

    sections
}

fn render_sections(
    ui: &mut egui::Ui,
    cache: &mut CommonMarkCache,
    sections: &[Section],
    target_width: f32,
    scroll_to_section: &mut Option<usize>,
    section_counter: &mut usize,
    scope: &str,
) {
    for (idx, section) in sections.iter().enumerate() {
        let anchor =
            ui.allocate_response(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        if scroll_to_section.as_ref() == Some(&idx) {
            ui.scroll_to_rect(anchor.rect, Some(Align::TOP));
            *scroll_to_section = None;
        }

        let blocks = split_markdown_blocks(&section.content);
        let mut table_idx = 0usize;
        for block in blocks {
            match block {
                MarkdownBlock::Markdown(text) => {
                    if !text.trim().is_empty() {
                        CommonMarkViewer::new()
                            .default_width(Some(target_width as usize))
                            .syntax_theme_dark("Dracula")
                            .syntax_theme_light("Solarized (light)")
                            .show(ui, cache, &text);
                    }
                }
                MarkdownBlock::Table { headers, rows } => {
                    table_idx += 1;
                    render_table_block(
                        ui,
                        &format!("md_table_{scope}_{}_{}", *section_counter, table_idx),
                        &headers,
                        &rows,
                        target_width,
                    );
                    ui.add_space(10.0);
                }
            }
        }
        *section_counter += 1;
    }
}

fn discover_system_fonts() -> (Vec<String>, HashMap<String, Vec<FaceCandidate>>) {
    let mut db = Database::new();
    db.load_system_fonts();

    let mut names = BTreeSet::new();
    let mut faces = HashMap::<String, Vec<FaceCandidate>>::new();

    for face in db.faces() {
        let path = match &face.source {
            Source::File(path) | Source::SharedFile(path, _) => path.clone(),
            Source::Binary(_) => continue,
        };
        for (family_name, _) in &face.families {
            if family_name.trim().is_empty() {
                continue;
            }
            names.insert(family_name.clone());
            faces
                .entry(family_name.clone())
                .or_default()
                .push(FaceCandidate {
                    path: path.clone(),
                    style: face.style,
                    weight: face.weight.0,
                    monospaced: face.monospaced,
                });
        }
    }

    let mut fonts: Vec<String> = names.into_iter().collect();
    if fonts.is_empty() {
        fonts.push("Proportional".to_owned());
    }
    (fonts, faces)
}

fn choose_font(fonts: &[String], preferred: &str) -> String {
    if let Some(found) = fonts
        .iter()
        .find(|name| name.eq_ignore_ascii_case(preferred))
        .cloned()
    {
        return found;
    }

    if let Some(first) = fonts.first() {
        return first.clone();
    }

    preferred.to_owned()
}

fn normalize_markdown_for_viewer(markdown: &str) -> String {
    let mut out = Vec::new();
    let mut in_shifted_fence = false;
    let mut in_fence = false;

    for line in markdown.lines() {
        let trimmed = line.trim_start();

        if line.starts_with("  ```") {
            out.push(trimmed.to_owned());
            in_shifted_fence = !in_shifted_fence;
            continue;
        }

        if in_shifted_fence {
            if let Some(rest) = line.strip_prefix("  ") {
                out.push(rest.to_owned());
            } else {
                out.push(line.to_owned());
            }
            if trimmed.starts_with("```") {
                in_shifted_fence = false;
            }
            continue;
        }

        if trimmed.starts_with("```") {
            if !in_fence {
                out.push(line.to_owned());
                out.push(String::new());
                in_fence = true;
            } else {
                out.push(String::new());
                out.push(line.to_owned());
                in_fence = false;
            }
            continue;
        }

        if in_fence {
            out.push(format!("  {line}"));
            continue;
        }

        out.push(line.to_owned());
    }

    out.join("\n")
}

fn is_table_row(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.contains('|') && !trimmed.is_empty()
}

fn is_table_separator(line: &str) -> bool {
    let cells = parse_table_row(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let t = c.trim();
            !t.is_empty() && t.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
        })
}

fn parse_table_row(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|s| s.trim().to_owned())
        .collect()
}

fn split_markdown_blocks(markdown: &str) -> Vec<MarkdownBlock> {
    let lines: Vec<&str> = markdown.lines().collect();
    let mut i = 0usize;
    let mut md_buf: Vec<String> = Vec::new();
    let mut blocks = Vec::new();

    while i < lines.len() {
        if i + 1 < lines.len()
            && is_table_row(lines[i])
            && is_table_separator(lines[i + 1])
            && parse_table_row(lines[i]).len() >= 2
        {
            if !md_buf.is_empty() {
                blocks.push(MarkdownBlock::Markdown(md_buf.join("\n")));
                md_buf.clear();
            }
            let headers = parse_table_row(lines[i]);
            i += 2;
            let mut rows = Vec::new();
            while i < lines.len() && is_table_row(lines[i]) {
                rows.push(parse_table_row(lines[i]));
                i += 1;
            }
            blocks.push(MarkdownBlock::Table { headers, rows });
            continue;
        }

        md_buf.push(lines[i].to_owned());
        i += 1;
    }

    if !md_buf.is_empty() {
        blocks.push(MarkdownBlock::Markdown(md_buf.join("\n")));
    }

    blocks
}

fn render_table_block(
    ui: &mut egui::Ui,
    _table_id: &str,
    headers: &[String],
    rows: &[Vec<String>],
    target_width: f32,
) {
    if headers.is_empty() {
        return;
    }

    let cols = headers.len();
    let spacing = ui.spacing().item_spacing.x;
    let usable_width = (target_width - spacing * (cols.saturating_sub(1) as f32)).max(200.0);
    let mut col_widths = vec![usable_width / cols as f32; cols];
    if cols == 2 {
        col_widths[0] = usable_width * 0.42;
        col_widths[1] = usable_width * 0.58;
    }

    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            let left_w = col_widths.first().copied().unwrap_or(220.0);
            let right_w = col_widths.get(1).copied().unwrap_or(420.0);

            let render_row = |ui: &mut egui::Ui, left: &str, right: &str, is_header: bool| {
                ui.horizontal_top(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(left_w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_max_width(left_w);
                            let txt = if is_header {
                                RichText::new(left).strong()
                            } else {
                                RichText::new(left)
                            };
                            ui.add(egui::Label::new(txt).wrap());
                        },
                    );

                    ui.add_space(18.0);

                    ui.allocate_ui_with_layout(
                        egui::vec2(right_w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_max_width(right_w);
                            let txt = if is_header {
                                RichText::new(right).strong()
                            } else {
                                RichText::new(right)
                            };
                            ui.add(egui::Label::new(txt).wrap());
                        },
                    );
                });
            };

            let left_header = headers.first().map(String::as_str).unwrap_or("Column 1");
            let right_header = headers.get(1).map(String::as_str).unwrap_or("Column 2");
            render_row(ui, left_header, right_header, true);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            for (idx, row) in rows.iter().enumerate() {
                let left = row.first().map(String::as_str).unwrap_or("");
                let right = row.get(1).map(String::as_str).unwrap_or("");
                render_row(ui, left, right, false);
                ui.add_space(8.0);
                if idx + 1 < rows.len() {
                    ui.separator();
                    ui.add_space(8.0);
                }
            }
        });
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();
    let native_options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([1024.0, 760.0])
            .with_min_inner_size([700.0, 480.0])
            .with_title("MD Viewer RS"),
        ..Default::default()
    };

    eframe::run_native(
        "MD Viewer RS",
        native_options,
        Box::new(move |cc| Ok(Box::new(MarkdownViewerApp::new(cc, args.path.clone())))),
    )
}
