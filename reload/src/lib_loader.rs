use egui::Ui;
use libloading::Library;
use rand::thread_rng;
use rand::Rng;
use std::any::Any;
use std::cell::RefCell;
use std::path::PathBuf;

use crate::GainModelProcess;
use crate::State;

use std::fs;

use std::time::{SystemTime, UNIX_EPOCH};

pub fn vals_to_string(vals: (f32, f32)) -> String {
    format!("{}{}", from_floatval(vals.0), from_floatval(vals.1))
}

pub fn to_floatval(val: u64) -> f32 {
    let n = (1000000000.0 / (val as f64)) as f32;
    n - n.floor()
}

pub fn from_floatval(val: f32) -> u64 {
    (val * 10000000.0) as u64
}

pub fn rand_vals() -> (f32, f32) {
    let rand_num: f32 = thread_rng().gen_range(0.0..0.9999999);

    let since_the_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    (to_floatval(since_the_epoch), rand_num)
}

pub trait TestTrait: Any + Send + Sync {
    fn process(&mut self, l: f32, r: f32, model: &GainModelProcess) -> (f32, f32);
}

pub struct LibLoader {
    lib: Vec<Library>,
    pub trait_object: Option<Box<dyn TestTrait>>,
    tempdir: PathBuf,
    current_lib_file: String,
}

impl LibLoader {
    pub fn new() -> Self {
        let vals = rand_vals();
        let s = vals_to_string(vals);
        let tempdir = std::env::temp_dir().join(format!("vsthotload{}", &s));
        fs::create_dir(&tempdir).unwrap();

        LibLoader {
            lib: Vec::new(),
            trait_object: None,
            tempdir,
            current_lib_file: String::new(),
        }
    }

    pub fn load(&mut self) {
        let lib_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("target")
            .join("release")
            .join("reloaded.dll");

        let new_filename = format!("reloaded{}.dll", thread_rng().gen_range(0..65536) as u32);
        self.current_lib_file = new_filename;

        ::log::info!(
            "from {:?} to {:?}",
            &lib_path,
            &self.tempdir.join(&self.current_lib_file)
        );
        fs::copy(&lib_path, &self.tempdir.join(&self.current_lib_file)).unwrap();

        unsafe {
            match Library::new(&self.tempdir.join(&self.current_lib_file)) {
                Ok(lib) => {
                    self.lib.push(lib);
                }
                Err(_) => {
                    ::log::info!(
                        "Failed to load library '{:?}'",
                        self.tempdir.join(&self.current_lib_file)
                    );
                }
            }
        }
    }

    pub fn get_process_trait(&mut self) -> Option<Box<dyn TestTrait>> {
        unsafe {
            let get_test_trait: libloading::Symbol<fn() -> *mut dyn TestTrait> =
                self.lib.last().unwrap().get(b"_plugin_create").unwrap();

            Some(Box::from_raw(get_test_trait()))
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
