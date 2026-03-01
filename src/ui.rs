use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::MidiChannel;
use crate::patch_manager;

use egui::{
    Align, CentralPanel, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, RichText,
    Slider, Stroke, Ui, Vec2, Widget, style::HandleShape,
};

use crate::Patch;
use crate::dsp::{FilterMode, Waveform};

//  Colour palette
const BG: Color32 = Color32::from_rgb(230, 225, 245); // light lavender
const PANEL_BG: Color32 = Color32::from_rgb(215, 208, 235); // slightly darker lavender
const ACCENT: Color32 = Color32::from_rgb(75, 55, 120); // dark lavender
const ACCENT_DIM: Color32 = Color32::from_rgb(140, 125, 175); // muted lavender
const TEXT_DIM: Color32 = Color32::from_rgb(100, 85, 140); // dim dark lavender
const KNOB_BG: Color32 = Color32::from_rgb(200, 193, 225); // mid lavender

pub struct GregoryApp {
    patch: Arc<Mutex<Patch>>,
    /// Local copy we mutate in the UI, it's written back to the mutex on change.
    local: Patch,
    running: Arc<AtomicBool>,
    current_patch_name: Option<String>,
    midi_channel: Option<Arc<Mutex<MidiChannel>>>,
}

impl GregoryApp {
    pub fn new(
        patch: Arc<Mutex<Patch>>,
        running: Arc<AtomicBool>,
        midi_channel: Option<Arc<Mutex<MidiChannel>>>,
        cc: &eframe::CreationContext,
    ) -> Self {
        setup_fonts(&cc.egui_ctx);
        setup_visuals(&cc.egui_ctx);
        let local = patch.lock().unwrap().clone();
        Self {
            patch,
            local,
            running,
            current_patch_name: None,
            midi_channel,
        }
    }
}

impl eframe::App for GregoryApp {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.running.store(false, Ordering::SeqCst);
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.running.load(Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if let Ok(p) = self.patch.try_lock() {
            self.local.filter_cutoff = p.filter_cutoff;
        }

        let before = self.local.clone();

        let mut load_clicked = false;
        let mut save_clicked = false;
        let mut new_clicked = false;

        let new_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::N);
        let open_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O);
        let save_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S);

        if ctx.input_mut(|i| i.consume_shortcut(&new_shortcut)) {
            new_clicked = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&open_shortcut)) {
            load_clicked = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&save_shortcut)) {
            save_clicked = true;
        }

        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("New")
                                    .font(FontId::new(14.0, FontFamily::Proportional)),
                            )
                            .shortcut_text(ctx.format_shortcut(&new_shortcut)),
                        )
                        .clicked()
                    {
                        new_clicked = true;
                        ui.close();
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Open...")
                                    .font(FontId::new(14.0, FontFamily::Proportional)),
                            )
                            .shortcut_text(ctx.format_shortcut(&open_shortcut)),
                        )
                        .clicked()
                    {
                        load_clicked = true;
                        ui.close();
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("Save...")
                                    .font(FontId::new(14.0, FontFamily::Proportional)),
                            )
                            .shortcut_text(ctx.format_shortcut(&save_shortcut)),
                        )
                        .clicked()
                    {
                        save_clicked = true;
                        ui.close();
                    }

                    ui.separator();

                    if ui
                        .button(
                            RichText::new("Quit").font(FontId::new(14.0, FontFamily::Proportional)),
                        )
                        .clicked()
                    {
                        self.running.store(false, Ordering::SeqCst);
                        ui.close();
                    }
                });

                ui.menu_button("MIDI", |ui| {
                    if let Some(ch_mutex) = &self.midi_channel {
                        let mut ch = ch_mutex.lock().unwrap();

                        let omni_selected = *ch == MidiChannel::Omni;
                        if ui
                            .button(RichText::new("Omni").font(FontId::new(
                                14.0,
                                if omni_selected {
                                    FontFamily::Name("SpecialGothicBold".into())
                                } else {
                                    FontFamily::Proportional
                                },
                            )))
                            .clicked()
                        {
                            *ch = MidiChannel::Omni;
                        }

                        ui.separator();

                        for n in 1u8..=16 {
                            let selected = *ch == MidiChannel::Channel(n);
                            if ui
                                .button(RichText::new(format!("Channel {}", n)).font(FontId::new(
                                    14.0,
                                    if selected {
                                        FontFamily::Name("SpecialGothicBold".into())
                                    } else {
                                        FontFamily::Proportional
                                    },
                                )))
                                .clicked()
                            {
                                *ch = MidiChannel::Channel(n);
                            }
                        }
                    } else {
                        ui.label(dimmed("No MIDI device"));
                    }
                });
            });
        });

        CentralPanel::default()
            .frame(Frame::new().fill(BG))
            .show(ctx, |ui| {
                ui.add_space(12.0);

                // Title bar
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new("GREGORY")
                            .font(FontId::new(
                                24.0,
                                FontFamily::Name("SpecialGothicBold".into()),
                            ))
                            .color(ACCENT),
                    );
                    ui.add_space(1.0);
                    ui.label(
                        RichText::new("monosynth")
                            .font(FontId::new(14.0, FontFamily::Proportional))
                            .color(TEXT_DIM),
                    );

                    let available = ui.available_width() - 32.0;
                    ui.add_space(available);

                    if dice_button(ui).clicked() {
                        self.local = Patch::random();
                        self.current_patch_name = None;
                    }

                    ui.add_space(16.0);
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(10.0);

                // Main panel row
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    ui.add_space(20.0);
                    section(ui, "OSCILLATOR", None, |ui| {
                        osc_section(ui, &mut self.local);
                    });
                    section(ui, "FILTER ENV", None, |ui| {
                        flt_env_section(ui, &mut self.local);
                    });
                    section(ui, "FILTER", Some(180.0), |ui| {
                        filter_section(ui, &mut self.local);
                    });
                    section(ui, "AMP ENV", None, |ui| {
                        amp_env_section(ui, &mut self.local);
                    });
                    section(ui, "GAIN", Some(0.0), |ui| {
                        gain_section(ui, &mut self.local);
                    });
                });
            });

        // Act on menu actions after all UI is drawn.
        if new_clicked {
            self.local = Patch::default();
            self.current_patch_name = None;
        }

        if load_clicked
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Load Patch")
                .add_filter("TOML", &["toml"])
                .set_directory(patch_manager::patches_dir())
                .pick_file()
            && let Ok(p) = patch_manager::load_patch_from_path(&path)
        {
            self.local = p;
            self.current_patch_name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.to_owned());
        }

        if save_clicked
            && let Some(path) = rfd::FileDialog::new()
                .set_title("Save Patch")
                .add_filter("TOML", &["toml"])
                .set_directory(patch_manager::patches_dir())
                .set_file_name(match &self.current_patch_name {
                    Some(n) => format!("{}.toml", n),
                    None => "new patch.toml".to_owned(),
                })
                .save_file()
        {
            if let Err(e) = patch_manager::save_patch_to_path(&self.local, &path) {
                eprintln!("Failed to save patch: {e}");
            } else {
                self.current_patch_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_owned());
            }
        }

        // Write back only if something changed.
        if patch_changed(&before, &self.local)
            && let Ok(mut p) = self.patch.lock()
        {
            *p = self.local.clone();
        }

        // Drive continuous repaints so MIDI-driven parameter changes show up.
        ctx.request_repaint();
    }
}

fn osc_section(ui: &mut Ui, patch: &mut Patch) {
    ui.label(dimmed("WAVE"));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let saw_selected = patch.waveform == Waveform::Sawtooth;
        if toggle_button(ui, "SAW", saw_selected).clicked() {
            patch.waveform = Waveform::Sawtooth;
        }
        let sq_selected = patch.waveform == Waveform::Square;
        if toggle_button(ui, "SQR", sq_selected).clicked() {
            patch.waveform = Waveform::Square;
        }
    });

    ui.add_space(8.0);

    let is_square = patch.waveform == Waveform::Square;
    ui.add_enabled_ui(is_square, |ui| {
        ui.label(dimmed("PW"));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let is_narrow = (patch.pulse_width - 0.25).abs() < 0.01;
            if toggle_button(ui, "1/4", is_narrow).clicked() {
                patch.pulse_width = 0.25;
            }
            let is_half = (patch.pulse_width - 0.5).abs() < 0.01;
            if toggle_button(ui, "1/2", is_half).clicked() {
                patch.pulse_width = 0.5;
            }
        });
    });
}

fn filter_section(ui: &mut Ui, patch: &mut Patch) {
    ui.label(dimmed("MODE"));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let lp_selected = patch.filter_mode == FilterMode::LowPass;
        if toggle_button(ui, "LP4", lp_selected).clicked() {
            patch.filter_mode = FilterMode::LowPass;
        }
        let lp2_selected = patch.filter_mode == FilterMode::LowPass2Pole;
        if toggle_button(ui, "LP2", lp2_selected).clicked() {
            patch.filter_mode = FilterMode::LowPass2Pole;
        }
    });

    ui.add_space(8.0);

    let mut cutoff_norm = (patch.filter_cutoff - 10.0) / (18000.0 - 10.0);
    let mut res_norm = patch.filter_resonance;
    let mut env_norm = patch.flt_env_amount / 10000.0;

    let cutoff_before = cutoff_norm;
    let res_before = res_norm;
    let env_before = env_norm;

    let col_width = 56.0;
    let knob_size = 44.0;

    ui.horizontal(|ui| {
        for (label, norm, display) in [
            (
                "CUTOFF",
                &mut cutoff_norm,
                format!("{:.0}", patch.filter_cutoff),
            ),
            (
                "RES",
                &mut res_norm,
                format!("{:.2}", patch.filter_resonance),
            ),
            ("ENV", &mut env_norm, format!("{:.0}", patch.flt_env_amount)),
        ] {
            ui.vertical(|ui| {
                ui.set_min_width(col_width);
                ui.set_max_width(col_width);
                ui.vertical_centered(|ui| {
                    ui.label(dimmed(label));
                    ui.add_space(2.0);
                    rotary_knob(ui, norm, knob_size);
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(display)
                            .font(FontId::new(12.0, FontFamily::Proportional))
                            .color(ACCENT),
                    );
                });
            });
            ui.add_space(4.0);
        }
    });

    if cutoff_norm != cutoff_before {
        patch.filter_cutoff = 10.0 + cutoff_norm * (18000.0 - 10.0);
    }
    if res_norm != res_before {
        patch.filter_resonance = res_norm;
    }
    if env_norm != env_before {
        patch.flt_env_amount = env_norm * 10000.0;
    }
}

fn amp_env_section(ui: &mut Ui, patch: &mut Patch) {
    ui.horizontal(|ui| {
        fader(ui, "A", &mut patch.amp_attack, 0.001..=4.0, 3);
        fader(ui, "D", &mut patch.amp_decay, 0.001..=4.0, 3);
        fader(ui, "S", &mut patch.amp_sustain, 0.0..=1.0, 2);
        fader(ui, "R", &mut patch.amp_release, 0.001..=4.0, 3);
    });
}

fn flt_env_section(ui: &mut Ui, patch: &mut Patch) {
    ui.horizontal(|ui| {
        fader(ui, "A", &mut patch.flt_attack, 0.001..=4.0, 3);
        fader(ui, "D", &mut patch.flt_decay, 0.001..=4.0, 3);
        fader(ui, "S", &mut patch.flt_sustain, 0.0..=1.0, 2);
        fader(ui, "R", &mut patch.flt_release, 0.001..=4.0, 3);
    });
}

fn gain_section(ui: &mut Ui, patch: &mut Patch) {
    rotary_knob(ui, &mut patch.gain, 40.0);
}

//  Widgets

fn rotary_knob(ui: &mut Ui, value: &mut f64, size: f32) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click_and_drag());

    if response.dragged() {
        let delta = -response.drag_delta().y / 100.0;
        *value = (*value + delta as f64).clamp(0.0, 1.0);
    }

    let painter = ui.painter();
    let center = rect.center();
    let radius = size / 2.0 - 4.0;

    // Background circle
    painter.circle_filled(center, radius, PANEL_BG);
    painter.circle_stroke(center, radius, Stroke::new(1.5, ACCENT_DIM));

    // Arc showing value — 270° total sweep, starting at bottom-left
    let start_angle = std::f32::consts::PI * 0.75;
    let end_angle = start_angle + std::f32::consts::PI * 1.5 * *value as f32;
    let arc_radius = radius - 3.0;
    let steps = 32;
    let points: Vec<egui::Pos2> = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let angle = start_angle + t * (end_angle - start_angle);
            egui::Pos2::new(
                center.x + arc_radius * angle.cos(),
                center.y + arc_radius * angle.sin(),
            )
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(2.0, ACCENT)));

    // Indicator dot at current value position
    let indicator_angle = start_angle + std::f32::consts::PI * 1.5 * *value as f32;
    let dot_pos = egui::Pos2::new(
        center.x + arc_radius * indicator_angle.cos(),
        center.y + arc_radius * indicator_angle.sin(),
    );
    painter.circle_filled(dot_pos, 3.0, ACCENT);
}

fn fader(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    decimals: usize,
) {
    let col_width = 40.0;
    let slider_height = 120.0;

    ui.vertical(|ui| {
        ui.set_min_width(col_width);
        ui.set_max_width(col_width);

        // Value at top, centered
        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.set_min_size(Vec2::new(col_width, 20.0));
            ui.set_max_size(Vec2::new(col_width, 20.0));
            ui.label(
                RichText::new(format!("{:.prec$}", value, prec = decimals))
                    .font(FontId::new(12.0, FontFamily::Proportional))
                    .color(ACCENT),
            );
        });

        // Vertical slider, centered
        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.scope(|ui| {
                ui.spacing_mut().slider_rail_height = 3.0;
                ui.spacing_mut().interact_size = Vec2::new(4.0, 4.0);
                ui.visuals_mut().selection.bg_fill = ACCENT;
                ui.visuals_mut().widgets.active.bg_fill = ACCENT_DIM;
                ui.visuals_mut().widgets.inactive.fg_stroke = Stroke::new(1.0, ACCENT);
                ui.visuals_mut().widgets.hovered.fg_stroke = Stroke::new(1.0, ACCENT);

                let slider_width = 20.0;
                let padding = (col_width - slider_width) / 2.0;
                ui.horizontal(|ui| {
                    ui.add_space(padding);
                    ui.add_sized(
                        Vec2::new(slider_width, slider_height),
                        Slider::new(value, range)
                            .vertical()
                            .show_value(false)
                            .handle_shape(HandleShape::Circle)
                            .trailing_fill(true),
                    );
                });
            });
        });

        // Label at bottom, centered
        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.set_min_size(Vec2::new(col_width, 20.0));
            ui.set_max_size(Vec2::new(col_width, 20.0));
            ui.label(dimmed(label));
        });
    });
}

fn toggle_button(ui: &mut Ui, label: &str, active: bool) -> egui::Response {
    let (fg, bg) = if active {
        (BG, ACCENT)
    } else {
        (TEXT_DIM, PANEL_BG)
    };

    let btn = egui::Button::new(
        RichText::new(label)
            .font(FontId::new(14.0, FontFamily::Proportional))
            .color(fg),
    )
    .fill(bg)
    .stroke(Stroke::new(1.0, if active { ACCENT } else { ACCENT_DIM }))
    .min_size(Vec2::new(36.0, 22.0));

    btn.ui(ui)
}

/// Dimmed label for control names.
fn dimmed(text: &str) -> RichText {
    RichText::new(text)
        .font(FontId::new(14.0, FontFamily::Proportional))
        .color(TEXT_DIM)
}

fn section(ui: &mut Ui, title: &str, min_width: Option<f32>, content: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(PANEL_BG)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Vec2::new(12.0, 10.0))
        .stroke(Stroke::new(1.0, ACCENT_DIM))
        .show(ui, |ui| {
            ui.set_min_width(min_width.unwrap_or(90.0));
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(title)
                        .font(FontId::new(
                            14.0,
                            FontFamily::Name("SpecialGothicBold".into()),
                        ))
                        .color(ACCENT),
                );
                ui.add_space(6.0);
                content(ui);
            });
        });

    ui.add_space(8.0);
}

fn dice_button(ui: &mut Ui) -> egui::Response {
    let size = Vec2::new(16.0, 16.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    let painter = ui.painter();
    let center = rect.center();
    let r = 2.0; // corner radius

    let bg = if response.hovered() { ACCENT } else { PANEL_BG };
    let fg = if response.hovered() { BG } else { ACCENT };

    painter.rect(
        rect,
        CornerRadius::same(r as u8),
        bg,
        Stroke::new(1.0, ACCENT_DIM),
        egui::StrokeKind::Outside,
    );

    // Five dots in a dice-5 pattern
    let dot_r = 1.5;
    let pad = 4.0;
    let dots = [
        egui::Pos2::new(rect.min.x + pad, rect.min.y + pad), // top-left
        egui::Pos2::new(rect.max.x - pad, rect.min.y + pad), // top-right
        egui::Pos2::new(center.x, center.y),                 // center
        egui::Pos2::new(rect.min.x + pad, rect.max.y - pad), // bottom-left
        egui::Pos2::new(rect.max.x - pad, rect.max.y - pad), // bottom-right
    ];
    for dot in dots {
        painter.circle_filled(dot, dot_r, fg);
    }

    response
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "SpecialGothic".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/SpecialGothic-Regular.ttf"))
            .into(),
    );

    fonts.font_data.insert(
        "SpecialGothic-Bold".to_owned(),
        egui::FontData::from_static(include_bytes!("../assets/fonts/SpecialGothic-Bold.ttf"))
            .into(),
    );

    fonts
        .families
        .insert(FontFamily::Proportional, vec!["SpecialGothic".to_owned()]);

    fonts.families.insert(
        FontFamily::Name("SpecialGothicBold".into()),
        vec!["SpecialGothic-Bold".to_owned()],
    );

    ctx.set_fonts(fonts);
}

fn setup_visuals(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::light();
    visuals.window_fill = BG;
    visuals.panel_fill = BG;
    visuals.widgets.noninteractive.bg_fill = PANEL_BG;
    visuals.widgets.inactive.bg_fill = KNOB_BG;
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(190, 182, 218);
    visuals.widgets.active.bg_fill = Color32::from_rgb(175, 165, 205);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, ACCENT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5, ACCENT);
    visuals.widgets.active.fg_stroke = Stroke::new(2.0, ACCENT);
    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, BG);
    visuals.handle_shape = HandleShape::Circle;
    visuals.slider_trailing_fill = true;
    ctx.set_visuals(visuals);
}

/// Cheap structural equality check, helps to avoid writing to the mutex every frame.
fn patch_changed(a: &Patch, b: &Patch) -> bool {
    a.waveform != b.waveform
        || a.pulse_width != b.pulse_width
        || a.filter_mode != b.filter_mode
        || a.filter_cutoff != b.filter_cutoff
        || a.filter_resonance != b.filter_resonance
        || a.flt_env_amount != b.flt_env_amount
        || a.amp_attack != b.amp_attack
        || a.amp_decay != b.amp_decay
        || a.amp_sustain != b.amp_sustain
        || a.amp_release != b.amp_release
        || a.flt_attack != b.flt_attack
        || a.flt_decay != b.flt_decay
        || a.flt_sustain != b.flt_sustain
        || a.flt_release != b.flt_release
        || a.gain != b.gain
}
