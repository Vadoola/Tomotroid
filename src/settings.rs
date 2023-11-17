use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::fs::File;
use std::path::Path;
use directories::ProjectDirs;
use std::cell::OnceCell;
use std::sync::OnceLock;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub always_on_top: bool,
    pub auto_start_break_timer: bool,
    pub auto_start_work_timer: bool,
    pub break_always_on_top: bool,
    pub global_shortcuts: GlobalShortcuts,
    pub min_to_tray: bool,
    pub min_to_tray_on_close: bool,
    pub notifications: bool,
    pub theme: String,
    pub tick_sounds: bool,
    pub tick_sounds_during_break: bool,
    //in Pomotroid these have a max value of 90..and are stored in minutes...that's reasonable, so an i8 should be fine
    pub time_long_break: i8,
    pub time_short_break: i8,
    pub time_work: i8,
    pub volume: i8,
    pub work_rounds: i8,
}

//I'm thinking later, I probably need to store this in some sort of specific struct?
//Maybe 2 key type enums or something?
//A Modifier key, and a main key?...not really sure
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalShortcuts {
    #[serde(rename = "call-timer-reset")]
    pub call_timer_reset: String,
    #[serde(rename = "call-timer-skip")]
    pub call_timer_skip: String,
    #[serde(rename = "call-timer-toggle")]
    pub call_timer_toggle: String,
}

static CFG_DIR: OnceLock<Option<ProjectDirs>> = OnceLock::new();

fn get_dir() -> Option<&'static Path> {
    if let Some(dirs) = CFG_DIR.get_or_init(|| {
        ProjectDirs::from("org", "Vadoola",  "Tomotroid")
    }) {
        return Some(dirs.config_dir())
    } else {
        None
    }
}


//so I'm thinking this module has a load settings and save settings
//it handles getting the proper directory etc. and just reads in a file returning a struPathBuf
//need to probably include_bytes or include_str a default settings and default theme.
//so that if no settings and/or no theme files are found it has a fallback
pub fn load_settings() -> Settings {
    //if reading the files failes use default settings
    //need to start adding some logging probably
    //actually probably need to restructure this a bit
    //if the cfg dir doesn't exist, need to load defaults,
    //then if reading the file from the dir doesn't exist
    //need to load defualts
    if let Some(cfg_dir) = get_dir() {
        let file = cfg_dir.join("preferences.json");
        let set_file = File::open(file).unwrap();
        let reader = BufReader::new(set_file);

        serde_json::from_reader(reader).unwrap()
    } else {
        default_setings()
    }
}

fn default_setings() -> Settings {
    let def_set = include_bytes!("../assets/default-preferences.json");
    serde_json::from_reader(&def_set[..]).unwrap()
}

//Use https://docs.rs/serde_json/latest/serde_json/fn.to_writer_pretty.html
//for writing out the json when I save the settings