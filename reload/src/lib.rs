#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_specialization)]

use baseplug::{Model, Plugin, ProcessContext, UIFloatParam, UIModel, WindowOpenResult};
use baseview::{Size, WindowOpenOptions, WindowScalePolicy};
use raw_window_handle::HasRawWindowHandle;
use serde::{Deserialize, Serialize};

use egui::CtxRef;
use egui_baseview::{EguiWindow, Queue, RenderSettings, Settings};

pub mod lib_loader;
pub mod logging;

use lib_loader::{rand_vals, vals_to_string, LibLoader, TestTrait};
use logging::init_logging;

use std::fmt::Write;

baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    pub struct GainModel {
        #[model(min = -90.0, max = 3.0)]
        #[parameter(name = "gain left", unit = "Decibels",
            gradient = "Power(0.15)")]
        pub gain_left: f32,

        #[model(min = -90.0, max = 3.0)]
        #[parameter(name = "gain right", unit = "Decibels",
            gradient = "Power(0.15)")]
        pub gain_right: f32,

        #[model(min = -90.0, max = 3.0)]
        #[parameter(name = "gain master", unit = "Decibels",
            gradient = "Power(0.15)")]
        pub gain_master: f32,


        #[model(min = 0.0, max = 1.0)]
        #[parameter(name = "path_val1", unit = "Generic",
            gradient = "Linear")]
        pub path_val1: f32,

        #[model(min = 0.0, max = 1.0)]
        #[parameter(name = "path_val2", unit = "Generic",
            gradient = "Linear")]
        pub path_val2: f32,


        #[model(min = 0.0, max = 1.0)]
        #[parameter(name = "path_val2", unit = "Generic",
            gradient = "Linear")]
        pub update_event: f32,
    }
}

impl Default for GainModel {
    fn default() -> Self {
        Self {
            // "gain" is converted from dB to coefficient in the parameter handling code,
            // so in the model here it's a coeff.
            // -0dB == 1.0
            gain_left: 1.0,
            gain_right: 1.0,
            gain_master: 1.0,
            path_val1: 0.0,
            path_val2: 0.0,
            update_event: 0.0,
        }
    }
}

struct Gain {
    lib_load: Option<LibLoader>,
    process_trait: Option<Box<dyn TestTrait>>,
}

impl Plugin for Gain {
    const NAME: &'static str = "egui-baseplug hot gain";
    const PRODUCT: &'static str = "egui-baseplug hot gain";
    const VENDOR: &'static str = "DGriffin";

    const INPUT_CHANNELS: usize = 2;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = GainModel;

    #[inline]
    fn new(_sample_rate: f32, _model: &GainModel) -> Self {
        init_logging("EGUIBaseviewTest.log");

        Self {
            lib_load: None,
            process_trait: None,
        }
    }

    #[inline]
    fn process(&mut self, model: &GainModelProcess, ctx: &mut ProcessContext<Self>) {
        let current_vals = (
            model.path_val1[ctx.nframes - 1],
            model.path_val2[ctx.nframes - 1],
        );

        if self.lib_load.is_none() {
            self.lib_load = LibLoader::new_from_vals(current_vals);
            if let Some(lib_load) = self.lib_load.as_mut() {
                lib_load.load_existing();
                self.process_trait = lib_load.get_process_trait();
            }
        }

        if let Some(lib_load) = self.lib_load.as_mut() {
            if lib_load.rnd_path_vals != current_vals
                || lib_load.update_event != model.update_event[ctx.nframes - 1]
            {
                lib_load.update_event = model.update_event[ctx.nframes - 1];
                self.lib_load = LibLoader::new_from_vals(current_vals);
                if let Some(lib_load) = self.lib_load.as_mut() {
                    lib_load.load_existing();
                    self.process_trait = lib_load.get_process_trait();
                }
            }
        }

        let input = &ctx.inputs[0].buffers;
        let output = &mut ctx.outputs[0].buffers;

        for i in 0..ctx.nframes {
            let mut l = input[0][i];
            let mut r = input[1][i];
            if let Some(process_trait) = self.process_trait.as_mut() {
                let (tl, tr) = process_trait.process(l, r, model);
                l = tl;
                r = tr;
            }

            output[0][i] = l * model.gain_left[i] * model.gain_master[i];
            output[1][i] = r * model.gain_right[i] * model.gain_master[i];
            //output[0][i] = input[0][i] * model.gain_left[i] * model.gain_master[i];
            //output[1][i] = input[1][i] * model.gain_right[i] * model.gain_master[i];
        }
    }
}

pub fn param_slider(
    ui: &mut egui::Ui,
    label: &str,
    value_text: &mut String,
    param: &mut UIFloatParam,
) {
    ui.label(label);
    ui.label(value_text.as_str());

    // Use the normalized value of the param so we can take advantage of baseplug's value curves.
    //
    // You could opt to use your own custom widget if you wish, as long as it can operate with
    // a normalized range from [0.0, 1.0].
    let mut normal = param.normalized();
    if ui.add(egui::Slider::new(&mut normal, 0.0..=1.0)).changed() {
        param.set_from_normalized(normal);
        format_value(value_text, param);
    };
}

pub fn format_value(value_text: &mut String, param: &UIFloatParam) {
    *value_text = format!("{:.1} {}", param.value(), param.unit_label());
}

pub fn update_value_text(value_text: &mut String, param: &UIFloatParam) {
    if param.did_change() {
        format_value(value_text, param)
    }
}

impl baseplug::PluginUI for Gain {
    type Handle = ();

    fn ui_size() -> (i16, i16) {
        (500, 300)
    }

    fn ui_open(
        parent: &impl HasRawWindowHandle,
        model: <Self::Model as Model<Self>>::UI,
    ) -> WindowOpenResult<Self::Handle> {
        let settings = Settings {
            window: WindowOpenOptions {
                title: String::from("egui-baseplug-examples gain"),
                size: Size::new(Self::ui_size().0 as f64, Self::ui_size().1 as f64),
                scale: WindowScalePolicy::SystemScaleFactor,
            },
            render_settings: RenderSettings::default(),
        };

        EguiWindow::open_parented(
            parent,
            settings,
            EditorState {
                state: State::new(model),
                lib_load: LibLoader::new(),
            },
            // Called once before the first frame. Allows you to do setup code and to
            // call `ctx.set_fonts()`. Optional.
            |_egui_ctx: &CtxRef, _queue: &mut Queue, _editor_state: &mut EditorState| {},
            // Called before each frame. Here you should update the state of your
            // application and build the UI.
            |egui_ctx: &CtxRef, _queue: &mut Queue, editor_state: &mut EditorState| {
                // Must be called on the top of each frame in order to sync values from the rt thread.

                egui::Window::new("egui-baseplug gain demo").show(&egui_ctx, |ui| {
                    if ui.button("RELOAD").clicked() {
                        editor_state.lib_load.load();
                        let v = editor_state.state.model.update_event.value();
                        editor_state
                            .state
                            .model
                            .update_event
                            .set_from_normalized(v + 0.0001);
                    }
                    editor_state
                        .state
                        .model
                        .path_val1
                        .set_from_normalized(editor_state.lib_load.rnd_path_vals.0);
                    editor_state
                        .state
                        .model
                        .path_val2
                        .set_from_normalized(editor_state.lib_load.rnd_path_vals.1);

                    let state = &mut editor_state.state;

                    ui.label(&editor_state.lib_load.rnd_path_str);

                    ui.label(editor_state.lib_load.rnd_path_vals.0.to_string());
                    ui.label(editor_state.lib_load.rnd_path_vals.1.to_string());

                    // Sync text values if there was automation.
                    update_value_text(&mut state.gain_master_value, &state.model.gain_master);
                    update_value_text(&mut state.gain_left_value, &state.model.gain_left);
                    update_value_text(&mut state.gain_right_value, &state.model.gain_right);

                    param_slider(
                        ui,
                        "Gain Master",
                        &mut state.gain_master_value,
                        &mut state.model.gain_master,
                    );

                    editor_state.lib_load.ui_func(state, ui);
                });

                // TODO: Add a way for egui-baseview to send a closure that runs every frame without always
                // repainting.
                egui_ctx.request_repaint();
            },
        );

        Ok(())
    }

    fn ui_close(mut _handle: Self::Handle) {
        // TODO: Close window once baseview gets the ability to do this.
    }
}

pub struct EditorState {
    state: State,
    lib_load: LibLoader,
}

pub struct State {
    pub model: GainModelUI,

    pub gain_master_value: String,
    pub gain_left_value: String,
    pub gain_right_value: String,
}

impl State {
    pub fn new(model: GainModelUI) -> State {
        State {
            model,
            gain_master_value: String::new(),
            gain_left_value: String::new(),
            gain_right_value: String::new(),
        }
    }
}

baseplug::vst2!(Gain, b"tANa");
