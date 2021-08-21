use egui::Ui;
use hotlib::{libloading, TempLibrary, Watch};
use std::any::Any;
use std::cell::RefCell;
use std::time::{Duration, SystemTime};

use crate::GainModelProcess;
use crate::State;

pub trait TestTrait: Any + Send + Sync {
    fn process(&mut self, l: f32, r: f32, model: &GainModelProcess) -> (f32, f32);
}

pub struct LibLoader {
    lib: Vec<TempLibrary>,
    watch: Option<Watch>,
    pub trait_object: Option<Box<dyn TestTrait>>,
    last_load: SystemTime,
}

impl LibLoader {
    pub fn new() -> Self {
        LibLoader {
            lib: Vec::new(),
            watch: None,
            trait_object: None,
            last_load: SystemTime::now(),
        }
    }

    pub fn setup_watch(&mut self) {
        ::log::info!("Loading CARGO_MANIFEST_DIR");
        let test_crate_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("reloaded")
            .join("Cargo.toml");
        ::log::info!("Begin watching for changes to {:?}", test_crate_path);
        let watch = hotlib::watch(&test_crate_path).unwrap();
        match watch.package().build() {
            Ok(build) => match build.load() {
                Ok(lib) => {
                    self.lib.push(lib);
                    self.last_load = SystemTime::now();
                }
                Err(e) => ::log::info!("{}", e),
            },
            Err(e) => ::log::info!("{}", e),
        }
        self.watch = Some(watch);
    }

    pub fn check_load(&mut self) -> bool {
        if let Some(watch) = &self.watch {
            if let Some(pkg) = watch.try_next().unwrap() {
                if SystemTime::now()
                    .duration_since(self.last_load)
                    .unwrap()
                    .as_secs()
                    < 2
                {
                    return false;
                }
                match pkg.build() {
                    Ok(build) => match build.load() {
                        Ok(lib) => {
                            self.lib.push(lib);
                            self.last_load = SystemTime::now();
                            return true;
                        }
                        Err(e) => ::log::info!("{}", e),
                    },
                    Err(e) => ::log::info!("{}", e),
                }
            }
        }
        return false;
    }

    pub fn init_test_trait(&mut self) {
        if !self.lib.is_empty() {
            unsafe {
                let get_test_trait: libloading::Symbol<fn() -> *mut dyn TestTrait> =
                    self.lib.last().unwrap().get(b"_plugin_create").unwrap();

                self.trait_object = Some(Box::from_raw(get_test_trait()));
            }
        }
    }

    pub fn ui_func(&mut self, state: &mut State, ui: &mut Ui) {
        if !self.lib.is_empty() {
            unsafe {
                let editor: libloading::Symbol<fn(ui: RefCell<&mut Ui>, state: &mut State)> =
                    self.lib.last().unwrap().get(b"editor_init").unwrap();

                editor(RefCell::new(ui), state);
            }
        }
    }
}
