use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::cell::OnceCell;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use crate::JsonThemeTemp;

slint::include_modules!();

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JsonSettings {
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
    pub time_long_break: i32,
    pub time_short_break: i32,
    pub time_work: i32,
    pub volume: i32,
    pub work_rounds: i32,
}

//Need to look into if the serialization of the Slint structs in better in the newer release
//haven't tested in a bit, and I might not need to do this back and forth marshalling to use
//serde on this and the Theme struct anymore....
/*impl From<JsonSettings> for Settings {
    fn from(other: JsonSettings) -> Self {
        Settings {
            always_on_top: other.always_on_top,
            auto_start_break_timer: other.auto_start_break_timer,
            auto_start_work_timer: other.auto_start_work_timer,
            break_always_on_top: other.break_always_on_top,
            //global_shortcuts: other.global_shortcuts,
            min_to_tray: other.min_to_tray,
            min_to_tray_on_close: other.min_to_tray_on_close,
            notifications: other.notifications,
            //theme: other.theme,
            tick_sounds: other.tick_sounds,
            tick_sounds_during_break: other.tick_sounds_during_break,
            time_long_break: other.time_long_break.into(),
            time_short_break: other.time_short_break.into(),
            time_work: other.time_work.into(),
            volume: other.volume.into(),
            work_rounds: other.work_rounds.into(),
        }
    }
}*/

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
static DEF_THEME: OnceLock<JsonThemeTemp> = OnceLock::new();

//This really probably shouldn't be public. But for now as a quick way to get the theme loading
//working from the correct directory I'm making it public. I need to move the theme loading
//from the main.rs file into the settings module, then I can make this private again
//fn get_dir() -> Option<&'static Path> {
pub fn get_dir() -> Option<&'static Path> {
    if let Some(dirs) = CFG_DIR.get_or_init(|| ProjectDirs::from("org", "Vadoola", "Tomotroid")) {
        return Some(dirs.config_dir());
    } else {
        None
    }
}

pub fn default_theme() -> &'static JsonThemeTemp {
    DEF_THEME.get_or_init(|| {
        let def_theme = r##"{
            "name": "Rangitoto",
            "colors": {
            "--color-long-round": "#af486d",
            "--color-short-round": "#719002",
            "--color-focus-round": "#3c73b8",
            "--color-background": "#1a191e",
            "--color-background-light": "#343132",
            "--color-background-lightest": "#837c7e",
            "--color-foreground": "#dfdfd7",
            "--color-foreground-darker": "#bec0c0",
            "--color-foreground-darkest": "#adadae",
            "--color-accent": "#cd7a0c"
            }
        }"##;
        serde_json::from_str::<JsonThemeTemp>(&def_theme).unwrap()
    })
}

//so I'm thinking this module has a load settings and save settings
//it handles getting the proper directory etc. and just reads in a file returning a PathBuf
//need to probably include_bytes or include_str a default settings and default theme.
//so that if no settings and/or no theme files are found it has a fallback
//would it make any sense to use something like Figment(https://crates.io/crates/figment) instead of
//just looking at the raw Json? Could it provide any benefit or flexibility?
//pub fn load_settings() -> Settings {
pub fn load_settings() -> JsonSettings {
    //if reading the files fails use default settings
    //need to start adding some logging probably
    //actually probably need to restructure this a bit
    //if the cfg dir doesn't exist, need to load defaults,
    //then if reading the file from the dir doesn't exist
    //need to load defualts
    if let Some(cfg_dir) = get_dir() {
        let file = cfg_dir.join("preferences.json");
        if let Ok(set_file) = File::open(file) {
            let reader = BufReader::new(set_file);
            serde_json::from_reader(reader).unwrap()
        } else {
            default_settings()
        }
    } else {
        default_settings()
    }
}

//fn default_settings() -> Settings {
fn default_settings() -> JsonSettings {
    let def_set = include_bytes!("../assets/default-preferences.json");
    serde_json::from_reader(&def_set[..]).unwrap()
}

//Use https://docs.rs/serde_json/latest/serde_json/fn.to_writer_pretty.html
//for writing out the json when I save the settings

//what's the best way to call this...calling save settings every time a setting change
//would be the safest from the perspective of ensuring the settings are updated
//but that could be a lot of saving the file over and over.
//for example they change a timer slider form say 5m to 10m
//will the slint slider trigger the callback once, saying it was changed to 10,
//or will it trigger on every value update (ie, 6, 7, 8, 9, 10) triggering 5 updates
//to save settings? Since the settings file is pretty small/simple, I'm not sure it's worth
//trying to update just the value that's changed, probably just easier to rewrite the whole
//file every time. I could just save the settings on program exit...but if the program does crash
//the settings woin't get saved. Is there some good middle ground? Every X minutes check if there
//is a mismatch and save the settings? But then I have the overhead of some sort of timer to
//check every so often....Actually could I save the settings only when on the main screen somehow?
//So if the volume is changed, it saves right away (may not be super effecient, if it triggers for
//update of the slider and they change the volume a large amount), but if it changes something on the
//slidover screen, ie, timer, theme, etc it only saves when the slideover goes away? The logic might be
//a bit trickier, but might be a good middle ground of ensuring the settings get saved without quite
//writing out the file quite as much.
pub fn save_settings(settings: JsonSettings) {
    if let Some(cfg_dir) = get_dir() {
        let file = cfg_dir.join("preferences.json");
        let set_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file)
            .unwrap();
        let writer = BufWriter::new(set_file);

        serde_json::to_writer_pretty(writer, &settings).unwrap();
    }
}
