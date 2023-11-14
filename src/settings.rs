use serde::{Deserialize, Serialize};
use std::io::BufReader;
use std::fs::File;

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

//so I'm thinking this module has a load settings and save settings
//it handles getting the proper directory etc. and just reads in a file returning a struct
//which then gets stored in the mainWindow
//it gets passed a struct to write back out.
//need to probably include_bytes or include_str a default settings and default theme.
//so that if no settings and/or no theme files are found it has a fallback
pub fn load_settings() -> Settings {
    let set_file = File::open("/path/to/settings.json").unwrap();
    let reader = BufReader::new(set_file);

    serde_json::from_reader(reader).unwrap()
}