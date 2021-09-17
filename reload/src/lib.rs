#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_specialization)]

use baseplug::{Model, Plugin, PluginContext, ProcessContext, UIFloatParam, WindowOpenResult};
use baseview::{Size, WindowOpenOptions, WindowScalePolicy};
use raw_window_handle::HasRawWindowHandle;
use ringbuf::{Consumer, Producer, RingBuffer};
use serde::{Deserialize, Serialize};

use egui::CtxRef;
use egui_baseview::{EguiWindow, Queue, RenderSettings, Settings};

pub mod lib_loader;
pub mod logging;

use lib_loader::{LibLoader, TestTrait};
use logging::init_logging;

use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
    thread,
};

use keyboard_types::KeyboardEvent;

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
        }
    }
}

pub struct GainShared {
    lib_load: Arc<Mutex<LibLoader>>,
    process_trait_prod: Arc<Mutex<Producer<Box<dyn TestTrait>>>>,
    process_trait_cons: RefCell<Consumer<Box<dyn TestTrait>>>,
}

unsafe impl Send for GainShared {}
unsafe impl Sync for GainShared {}

impl PluginContext<Gain> for GainShared {
    fn new() -> Self {
        init_logging("EGUIBaseviewTest.log");
        let mut lib_load = LibLoader::new();
        lib_load.load();

        let rb = RingBuffer::<Box<dyn TestTrait>>::new(2);
        let (prod, cons) = rb.split();
        Self {
            lib_load: Arc::new(Mutex::new(lib_load)),
            process_trait_prod: Arc::new(Mutex::new(prod)),
            process_trait_cons: RefCell::new(cons),
        }
    }
}

struct Gain {
    process_trait: Option<Box<dyn TestTrait>>,
}

impl Plugin for Gain {
    const NAME: &'static str = "egui-baseplug hot gain";
    const PRODUCT: &'static str = "egui-baseplug hot gain";
    const VENDOR: &'static str = "DGriffin";

    const INPUT_CHANNELS: usize = 2;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = GainModel;
    type PluginContext = GainShared;

    #[inline]
    fn new(_sample_rate: f32, _model: &GainModel, _shared: &GainShared) -> Self {
        Self {
            process_trait: None,
        }
    }

    #[inline]
    fn process(
        &mut self,
        model: &GainModelProcess,
        ctx: &mut ProcessContext<Self>,
        shared: &GainShared,
    ) {
        let mut process_trait_cons = shared.process_trait_cons.borrow_mut();
        if !process_trait_cons.is_empty() {
            self.process_trait = Some(process_trait_cons.pop().unwrap());
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
    *value_text = format!("{:.1} {}", param.unit_value(), param.unit_label());
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
        shared_ctx: &GainShared,
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
                lib_load: shared_ctx.lib_load.clone(),
                process_trait_prod: shared_ctx.process_trait_prod.clone(),
            },
            // Called once before the first frame. Allows you to do setup code and to
            // call `ctx.set_fonts()`. Optional.
            |_egui_ctx: &CtxRef, _queue: &mut Queue, _editor_state: &mut EditorState| {},
            // Called before each frame. Here you should update the state of your
            // application and build the UI.
            |egui_ctx: &CtxRef, _queue: &mut Queue, editor_state: &mut EditorState| {
                // Must be called on the top of each frame in order to sync values from the rt thread.

                egui::Window::new("egui-baseplug gain demo").show(&egui_ctx, |ui| {
                    let mut lib_load = editor_state.lib_load.lock().unwrap();
                    let mut process_trait_prod = editor_state.process_trait_prod.lock().unwrap();
                    if ui.button("RELOAD").clicked() {
                        lib_load.load();
                        if !process_trait_prod.is_full() {
                            match process_trait_prod.push(lib_load.get_process_trait().unwrap()) {
                                Ok(_) => ::log::info!("push worked"),
                                Err(_) => ::log::info!("push didn't work"),
                            }
                        }

                        ::log::info!("UI RELOAD thread id {:?}", thread::current().id());
                    }

                    let state = &mut editor_state.state;

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

                    lib_load.ui_func(state, ui);
                });

                // TODO: Add a way for egui-baseview to send a closure that runs every frame without always
                // repainting.
                egui_ctx.request_repaint();
            },
        );

        Ok(())
    }

    fn ui_close(mut _handle: Self::Handle, _ctx: &GainShared) {
        // TODO: Close window once baseview gets the ability to do this.
    }

    fn ui_key_down(_ctx: &GainShared, _ev: KeyboardEvent) -> bool {
        true
    }

    fn ui_key_up(_ctx: &GainShared, _ev: KeyboardEvent) -> bool {
        true
    }

    fn ui_param_notify(
        _handle: &Self::Handle,
        _param: &'static baseplug::Param<Self, <Self::Model as baseplug::Model<Self>>::Smooth>,
        _val: f32,
    ) {
    }
}

pub struct EditorState {
    state: State,
    lib_load: Arc<Mutex<LibLoader>>,
    process_trait_prod: Arc<Mutex<Producer<Box<dyn TestTrait>>>>,
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
