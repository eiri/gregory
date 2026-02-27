use std::sync::{Arc, Mutex};

use egui::{
    Align, CentralPanel, Color32, CornerRadius, FontFamily, FontId, Frame, Layout, RichText,
    Slider, Stroke, Ui, Vec2, Widget, style::HandleShape,
};

use crate::Patch;
use crate::dsp::{FilterMode, Waveform};

//  Colour palette                                                      //
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
}

impl GregoryApp {
    pub fn new(patch: Arc<Mutex<Patch>>, cc: &eframe::CreationContext) -> Self {
        setup_fonts(&cc.egui_ctx);
        setup_visuals(&cc.egui_ctx);
        let local = patch.lock().unwrap().clone();
        Self { patch, local }
    }
}

impl eframe::App for GregoryApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(p) = self.patch.try_lock() {
            self.local.mod_wheel = p.mod_wheel;
            self.local.filter_cutoff = p.filter_cutoff;
        }

        let before = self.local.clone();

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
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                // Main panel row
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    ui.add_space(12.0);
                    section(ui, "OSCILLATOR", |ui| {
                        osc_section(ui, &mut self.local);
                    });
                    section(ui, "FILTER ENV", |ui| {
                        flt_env_section(ui, &mut self.local);
                    });
                    section(ui, "FILTER", |ui| {
                        filter_section(ui, &mut self.local);
                    });
                    section(ui, "AMP ENV", |ui| {
                        amp_env_section(ui, &mut self.local);
                    });
                    section(ui, "GAIN", |ui| {
                        gain_section(ui, &mut self.local);
                    });
                });
            });

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

    let cutoff_before = patch.filter_cutoff;
    knob(ui, "CUTOFF", &mut patch.filter_cutoff, 10.0..=18000.0, 0);
    knob(ui, "RES", &mut patch.filter_resonance, 0.0..=1.0, 2);
    knob(ui, "ENV", &mut patch.flt_env_amount, 0.0..=10000.0, 0);

    // Keep mod_wheel in sync with CUTOFF knob.
    if patch.filter_cutoff != cutoff_before {
        patch.mod_wheel = (patch.filter_cutoff - 10.0) / (18000.0 - 10.0);
    }
}

fn amp_env_section(ui: &mut Ui, patch: &mut Patch) {
    knob(ui, "ATK", &mut patch.amp_attack, 0.001..=4.0, 3);
    knob(ui, "DEC", &mut patch.amp_decay, 0.001..=4.0, 3);
    knob(ui, "SUS", &mut patch.amp_sustain, 0.0..=1.0, 2);
    knob(ui, "REL", &mut patch.amp_release, 0.001..=4.0, 3);
}

fn flt_env_section(ui: &mut Ui, patch: &mut Patch) {
    knob(ui, "ATK", &mut patch.flt_attack, 0.001..=4.0, 3);
    knob(ui, "DEC", &mut patch.flt_decay, 0.001..=4.0, 3);
    knob(ui, "SUS", &mut patch.flt_sustain, 0.0..=1.0, 2);
    knob(ui, "REL", &mut patch.flt_release, 0.001..=4.0, 3);
}

fn gain_section(ui: &mut Ui, patch: &mut Patch) {
    knob(ui, "GAIN", &mut patch.gain, 0.0..=1.0, 2);
}

//  Widgets                                                             //

fn knob(
    ui: &mut Ui,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    decimals: usize,
) {
    ui.vertical(|ui| {
        ui.set_min_width(120.0);
        ui.label(dimmed(label));
        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.scope(|ui| {
                ui.spacing_mut().slider_width = 100.0;
                ui.spacing_mut().slider_rail_height = 3.0;
                ui.spacing_mut().interact_size = Vec2::new(4.0, 4.0);
                ui.visuals_mut().selection.bg_fill = ACCENT;
                ui.visuals_mut().widgets.active.bg_fill = ACCENT_DIM;
                ui.visuals_mut().widgets.inactive.fg_stroke = Stroke::new(1.0, ACCENT);
                ui.visuals_mut().widgets.hovered.fg_stroke = Stroke::new(1.0, ACCENT);
                ui.add(
                    Slider::new(value, range)
                        .show_value(false)
                        .handle_shape(HandleShape::Circle)
                        .trailing_fill(true),
                );
            });

            ui.add_sized(
                Vec2::new(52.0, 14.0),
                egui::Label::new(
                    RichText::new(format!("{:.prec$}", value, prec = decimals))
                        .font(FontId::new(14.0, FontFamily::Proportional))
                        .color(ACCENT),
                ),
            );
        });

        ui.add_space(4.0);
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

fn section(ui: &mut Ui, title: &str, content: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(PANEL_BG)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Vec2::new(12.0, 10.0))
        .stroke(Stroke::new(1.0, ACCENT_DIM))
        .show(ui, |ui| {
            ui.set_min_width(90.0);
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
        || a.mod_wheel != b.mod_wheel
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
