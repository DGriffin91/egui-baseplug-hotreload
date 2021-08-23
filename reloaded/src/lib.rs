use std::{any::Any, cell::RefCell};

use egui::Ui;
use reload::{param_slider, GainModelProcess, State};

#[no_mangle]
pub fn editor_init(ui: RefCell<&mut Ui>, state: &mut State) {
    match ui.try_borrow_mut() {
        Ok(mut ui) => {
            editor(&mut ui, state);
        }
        Err(_e) => todo!(),
    }
}

#[no_mangle]
pub fn editor(ui: &mut Ui, state: &mut State) {
    param_slider(
        ui,
        "Gain Left",
        &mut state.gain_left_value,
        &mut state.model.gain_left,
    );
    param_slider(
        ui,
        "Gain Right",
        &mut state.gain_right_value,
        &mut state.model.gain_right,
    );
    //ui.label(&format!("STUFFFFFFFFFFFFFFFFFFF{}", state.gain_left_value));
}

#[derive(Debug, Default)]
pub struct SomeData {
    x: f32,
    y: f32,
}

trait TestTrait: Any + Send + Sync {
    fn process(&mut self, l: f32, r: f32, model: &GainModelProcess) -> (f32, f32);
}

impl TestTrait for SomeData {
    #[no_mangle]
    fn process(&mut self, l: f32, r: f32, _model: &GainModelProcess) -> (f32, f32) {
        //(l, r)
        //((l * 10.0).sin() * 0.2, (r * 10.0).sin() * 0.2)
        ((l * 10.0).tanh() * 0.2, (r * 10.0).tanh() * 0.2)
    }
}

#[macro_export]
macro_rules! declare_plugin {
    ($plugin_type:ty, $constructor:path) => {
        #[no_mangle]
        pub fn _plugin_create() -> *mut dyn TestTrait {
            // make sure the constructor is the correct type.
            let constructor: fn() -> $plugin_type = $constructor;

            let object = constructor();
            let boxed: Box<dyn TestTrait> = Box::new(object);
            Box::into_raw(boxed)
        }
    };
}

declare_plugin!(SomeData, SomeData::default);
