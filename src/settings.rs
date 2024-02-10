use directories::ProjectDirs;
use hex_color::HexColor;
use serde::{Deserialize, Serialize};
use slint::platform::Key;
use core::fmt;
use std::cell::OnceCell;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::iter;
use std::ops::Deref;
use std::path::{Display, Path};
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use slint::{Color, SharedString};
use walkdir::WalkDir;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use crate::JsonTheme;

#[derive(Debug, Clone, PartialEq)]
pub struct GKeyCode(Code);

impl From<Code> for GKeyCode {
    fn from(value: Code) -> Self {
        GKeyCode(value)
    }
}

impl From<GKeyCode> for Code {
    fn from(value: GKeyCode) -> Self {
        value.0
    }
}

//So this massive match block to convert to a string is to keep compatibility with the Pomotroid
//preferences / settings json file. Part of me was tempted to break compatibility, then I thought
//well maybe break compatibility but allow for importing the Pomotroid file and converting
//which would still require me manually addjusting the string format of the Code struct instead of
//using the built in version...so I might as well just keep compatibility. Perhaps for a v2 if I add
//other features I'll strip this and break compatibility. Honestly since there is a lot of overlap
//the match block isn't even that big, because for the values that overlap I can just fallback
//to use the Code's built in to/from string functions.
impl fmt::Display for GKeyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use global_hotkey::hotkey::Code::*;
        let tmp_str = self.0.to_string();
        //https://developer.mozilla.org/en-US/docs/Web/API/UI_Events/Keyboard_event_key_values
        write!(
            f,
            "{}",
            match self.0 {
                Backslash => "\\",
                BracketLeft => "[", //I've always known [ ] as brackets, but after coming to NZ I realize this is not universal, so I need to make sure this is the correct char for this enum value
                BracketRight => "]", //I've always known [ ] as brackets, but after coming to NZ I realize this is not universal, so I need to make sure this is the correct char for this enum value
                Comma => ",",
                Digit0 => "0",
                Digit1 => "1",
                Digit2 => "2",
                Digit3 => "3",
                Digit4 => "4",
                Digit5 => "5",
                Digit6 => "6",
                Digit7 => "7",
                Digit8 => "8",
                Digit9 => "9",
                Equal => "=",
                KeyA => "A",
                KeyB => "B",
                KeyC => "C",
                KeyD => "D",
                KeyE => "E",
                KeyF => "F",
                KeyG => "G",
                KeyH => "H",
                KeyI => "I",
                KeyJ => "J",
                KeyK => "K",
                KeyL => "L",
                KeyM => "M",
                KeyN => "N",
                KeyO => "O",
                KeyP => "P",
                KeyQ => "Q",
                KeyR => "R",
                KeyS => "S",
                KeyT => "T",
                KeyU => "U",
                KeyV => "V",
                KeyW => "W",
                KeyX => "X",
                KeyY => "Y",
                KeyZ => "Z",
                Minus => "-",
                Period => ".",
                Quote => "\"",
                Semicolon => ";",
                Slash => "/",
                Space => " ",
                NumpadAdd => "Add",
                NumpadClear => "Clear",
                NumpadDivide => "Divide",
                NumpadSubtract => "Subtract",
                LaunchApp1 => "LaunchApplication1",
                LaunchApp2 => "LaunchApplication2",
                MediaSelect => "LaunchMediaPlayer",
                MicrophoneMuteToggle => "MicrophoneToggle",

                //If I don't have a custom mapping because it wasn't found in the list
                //of key codes from Electron, or the Electron mapping and the default Code
                //mapping are the same just use the default to string for the Global KeyCode
                _ => &tmp_str,
            }
        )
    }
}

impl std::str::FromStr for GKeyCode {
    type Err = &'static str;//Todo: Get a better Error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use crate::Code::*;
        Ok(
            match s {
                "\\" => Backslash,
                "[" => BracketLeft,
                "]" => BracketRight,
                "," => Comma,
                "0" => Digit0,
                "1" => Digit1,
                "2" => Digit2,
                "3" => Digit3,
                "4" => Digit4,
                "5" => Digit5,
                "6" => Digit6,
                "7" => Digit7,
                "8" => Digit8,
                "9" => Digit9,
                "=" => Equal,
                "A" => KeyA,
                "B" => KeyB,
                "C" => KeyC,
                "D" => KeyD,
                "E" => KeyE,
                "F" => KeyF,
                "G" => KeyG,
                "H" => KeyH,
                "I" => KeyI,
                "J" => KeyJ,
                "K" => KeyK,
                "L" => KeyL,
                "M" => KeyM,
                "N" => KeyN,
                "O" => KeyO,
                "P" => KeyP,
                "Q" => KeyQ,
                "R" => KeyR,
                "S" => KeyS,
                "T" => KeyT,
                "U" => KeyU,
                "V" => KeyV,
                "W" => KeyW,
                "X" => KeyX,
                "Y" => KeyY,
                "Z" => KeyZ,
                "-" => Minus,
                "." => Period,
                "\"" => Quote,
                ";" => Semicolon,
                "/" => Slash,
                " " => Space,
                "Add" => NumpadAdd,
                "Clear" => NumpadClear,
                "Divide" => NumpadDivide,
                "Subtract" => NumpadSubtract,
                "LaunchApplication1" => LaunchApp1,
                "LaunchApplication2" => LaunchApp2,
                "MicrophoneToggle" => MicrophoneMuteToggle,

                _ => Code::from_str(s).map_err(|_|"Failure to convert Key Code")?,
            }.into()
        )
    }
}

impl Serialize for GKeyCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
                serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for GKeyCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
                let s = String::deserialize(deserializer)?;
                Ok(s.parse().unwrap())
    }
}

//Right now serde support in Slint is new and crude, some of the types in the Slint version
//of this struct like Brush don't support serde yet. So for now I'm creating 2 versions
//the slint version and this version to manually convert between them.
#[derive(Clone, Deserialize)]
struct ThemeColors {
    #[serde(rename = "--color-long-round")]
    long_round: HexColor,

    #[serde(rename = "--color-short-round")]
    short_round: HexColor,

    #[serde(rename = "--color-focus-round")]
    focus_round: HexColor,

    #[serde(rename = "--color-background")]
    background: HexColor,

    #[serde(rename = "--color-background-light")]
    background_light: HexColor,

    #[serde(rename = "--color-background-lightest")]
    background_lightest: HexColor,

    #[serde(rename = "--color-foreground")]
    foreground: HexColor,

    #[serde(rename = "--color-foreground-darker")]
    foreground_darker: HexColor,

    #[serde(rename = "--color-foreground-darkest")]
    foreground_darkest: HexColor,

    #[serde(rename = "--color-accent")]
    accent: HexColor,
}

#[derive(Clone, Deserialize)]
pub struct JsonThemeTemp {
    name: String,
    colors: ThemeColors,
}

//I realize implemeting From is more idomatic, but that would require creating a newtype for JsonTheme,
//due to the orphan rule, and then having to convert that (or maybe deref) that into JsonThemeTemp. I think this is a
//good and straight forward stop gap, until slint adds support for Serde to more types
impl Into<JsonTheme> for JsonThemeTemp {
    fn into(self) -> JsonTheme {
        JsonTheme {
            name: self.name.into(),
            long_round: Color::from_rgb_u8(
                self.colors.long_round.r,
                self.colors.long_round.g,
                self.colors.long_round.b,
            )
            .into(),
            short_round: Color::from_rgb_u8(
                self.colors.short_round.r,
                self.colors.short_round.g,
                self.colors.short_round.b,
            )
            .into(),
            focus_round: Color::from_rgb_u8(
                self.colors.focus_round.r,
                self.colors.focus_round.g,
                self.colors.focus_round.b,
            )
            .into(),
            background: Color::from_rgb_u8(
                self.colors.background.r,
                self.colors.background.g,
                self.colors.background.b,
            )
            .into(),
            background_light: Color::from_rgb_u8(
                self.colors.background_light.r,
                self.colors.background_light.g,
                self.colors.background_light.b,
            )
            .into(),
            background_lightest: Color::from_rgb_u8(
                self.colors.background_lightest.r,
                self.colors.background_lightest.g,
                self.colors.background_lightest.b,
            )
            .into(),
            foreground: Color::from_rgb_u8(
                self.colors.foreground.r,
                self.colors.foreground.g,
                self.colors.foreground.b,
            )
            .into(),
            foreground_darker: Color::from_rgb_u8(
                self.colors.foreground_darker.r,
                self.colors.foreground_darker.g,
                self.colors.foreground_darker.b,
            )
            .into(),
            foreground_darkest: Color::from_rgb_u8(
                self.colors.foreground_darkest.r,
                self.colors.foreground_darkest.g,
                self.colors.foreground_darkest.b,
            )
            .into(),
            accent: Color::from_rgb_u8(
                self.colors.accent.r,
                self.colors.accent.g,
                self.colors.accent.b,
            )
            .into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct JsonHotKey {
    pub modifiers: Modifiers,
    pub key: GKeyCode,
}

impl fmt::Display for JsonHotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.ctrl() {
            write!(f, "Control+")?;
        }
        if self.modifiers.alt() {
            write!(f, "Alt+")?;
        }
        if self.modifiers.shift() {
            write!(f, "Shift+")?;
        }
        if self.modifiers.meta() {
            write!(f, "Super+")?;
        }

        write!(f, "{}", &self.key)
    }
}

impl std::str::FromStr for JsonHotKey {
    type Err = &'static str;//Todo: Get a better Error type

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut mods = Modifiers::empty();
        let mut tokens_it = s.split('+').peekable();
        while let Some(key) = tokens_it.next() {
            if tokens_it.peek().is_some() {
                match key {
                    "Control" => mods.set(Modifiers::CONTROL, true),
                    "Alt" => mods.set(Modifiers::ALT, true),
                    "Shift" => mods.set(Modifiers::SHIFT, true),
                    "Super" => mods.set(Modifiers::META, true),
                    _ => panic!("No Other modifier keys currently supported"),
                };
            } else {
                return Ok(JsonHotKey {
                    modifiers: mods,
                    key: key.parse().unwrap(),
                })
            }
        }
        Err("Something failed")
    }
}

impl Serialize for JsonHotKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
                serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for JsonHotKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de> {
                let s = String::deserialize(deserializer)?;
                JsonHotKey::from_str(&s).map_err(serde::de::Error::custom)
    }
}

//I realize implemeting From is more idomatic, but that would require creating a newtype for HotKey,
//due to the orphan rule, and then having to convert that (or maybe deref) that into JsonHotKey.
//I feel like there is a better way to do this...but for now just to get the GlobalHotkeys up and working
//I'll put this in.
impl Into<HotKey> for JsonHotKey {
    fn into(self) -> HotKey {
        let mods = if self.modifiers.is_empty() {
            None
        } else {
            Modifiers::from_bits(self.modifiers.iter().fold(0, |acc, val| {
                acc | val.bits()
            }))
        };

        HotKey::new(mods, self.key.0)
    }
}

impl Into<HotKey> for &JsonHotKey {
    fn into(self) -> HotKey {
        let mods = if self.modifiers.is_empty() {
            None
        } else {
            Modifiers::from_bits(self.modifiers.iter().fold(0, |acc, val| {
                acc | val.bits()
            }))
        };

        HotKey::new(mods, self.key.0)
    }
}


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OldSettings {
    pub always_on_top: bool,
    pub auto_start_break_timer: bool,
    pub auto_start_work_timer: bool,
    pub break_always_on_top: bool,
    pub global_shortcuts: OldGlobalShortcuts,
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


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

//#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OldGlobalShortcuts {
    #[serde(rename = "call-timer-reset")]
    pub call_timer_reset: String,
    #[serde(rename = "call-timer-skip")]
    pub call_timer_skip: String,
    #[serde(rename = "call-timer-toggle")]
    pub call_timer_toggle: String,
}

//#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalShortcuts {
    #[serde(rename = "call-timer-reset")]
    pub call_timer_reset: JsonHotKey,
    #[serde(rename = "call-timer-skip")]
    pub call_timer_skip: JsonHotKey,
    #[serde(rename = "call-timer-toggle")]
    pub call_timer_toggle: JsonHotKey,
}

static CFG_DIR: OnceLock<Option<ProjectDirs>> = OnceLock::new();
static DEF_THEME: OnceLock<JsonThemeTemp> = OnceLock::new();

fn get_dir() -> Option<&'static Path> {
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
        serde_json::from_str::<JsonThemeTemp>(def_theme).unwrap()
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
            serde_json::from_reader(reader).expect("To be able to load the settings from json")
        } else {
            default_settings()
        }
    } else {
        default_settings()
    }
}

pub fn load_themes() -> Vec<JsonTheme> {
    let theme_dir = {
        let mut theme_dir = std::path::PathBuf::from(get_dir().unwrap());
        theme_dir.push("themes");
        theme_dir
    };
    let mut themes: Vec<JsonTheme> = WalkDir::new(theme_dir)
        .into_iter()
        .filter(|e| {
            return e.as_ref().map_or(false, |f| {
                f.file_name()
                    .to_str()
                    .map(|s| s.to_lowercase().ends_with(".json"))
                    .unwrap_or(false)
            });
        })
        .filter_map(|e| {
            e.map(|e| {
                let reader = BufReader::new(File::open(e.path()).unwrap());
                let theme = std::io::read_to_string(reader).unwrap();
                serde_json::from_str::<JsonThemeTemp>(&theme)
                    .unwrap()
                    .into()
            })
            .ok()
        })
        .collect();
    if themes.is_empty() {
        themes.push((*default_theme()).clone().into())
    }
    themes.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap());
    themes
}

//fn default_settings() -> Settings {
fn default_settings() -> JsonSettings {
    let def_set = include_bytes!("../assets/default-preferences.json");
    serde_json::from_reader(&def_set[..]).unwrap()

    //JsonSettings using keycode
    /*JsonSettings {
        always_on_top: false,
        auto_start_break_timer: true,
        auto_start_work_timer: true,
        break_always_on_top: false,
        global_shortcuts: GlobalShortcuts {
            call_timer_reset: JsonHotKey {
                modifiers: vec![KeyCode::ControlLeft],
                key: KeyCode::F2,
            },
            call_timer_skip: JsonHotKey {
                modifiers: vec![KeyCode::ControlLeft],
                key: KeyCode::F3,
            },
            call_timer_toggle: JsonHotKey {
                modifiers: vec![KeyCode::ControlLeft],
                key: KeyCode::F1,
            },
        },
        min_to_tray: false,
        min_to_tray_on_close: false,
        notifications: true,
        theme: "Rangitoto".to_string(),
        tick_sounds: true,
        tick_sounds_during_break: true,
        time_long_break: 1,
        time_short_break: 1,
        time_work: 1,
        volume: 100,
        work_rounds: 2,
    }*/

    //JsonSettings using Code
    /*JsonSettings {
        always_on_top: false,
        auto_start_break_timer: true,
        auto_start_work_timer: true,
        break_always_on_top: false,
        global_shortcuts: GlobalShortcuts {
            call_timer_reset: JsonHotKey {
                modifiers: Modifiers::CONTROL,
                key: Code::F2.into(),
            },
            call_timer_skip: JsonHotKey {
                modifiers: Modifiers::CONTROL,
                key: Code::F3.into(),
            },
            call_timer_toggle: JsonHotKey {
                modifiers: Modifiers::CONTROL,
                //key: Code::F1,
                key: Code::KeyD.into(),
            },
        },
        min_to_tray: false,
        min_to_tray_on_close: false,
        notifications: true,
        theme: "Rangitoto".to_string(),
        tick_sounds: true,
        tick_sounds_during_break: true,
        time_long_break: 1,
        time_short_break: 1,
        time_work: 1,
        volume: 100,
        work_rounds: 2,
    }*/
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
//a bit trickier, but might be a good middle ground of ensuring the settings get saved without
//writing out the file quite as much.
pub fn save_settings(settings: JsonSettings) {
    if let Some(cfg_dir) = get_dir() {
        std::fs::create_dir_all(cfg_dir).unwrap();

        let file = cfg_dir.join("preferences.json");
        let set_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(file)
            .unwrap();
        let writer = BufWriter::new(set_file);

        serde_json::to_writer_pretty(writer, &settings).expect("To be able to write the settings back out to json");
    }
}

pub fn get_non_print_key_txt(text: SharedString) -> Option<&'static str> {
    //the way Slint returns the key pressed is as a SharedString
    //For non-printable characters they do some sort of unicode encoding
    //and you can compare it against the Key enum, by converting the Key
    //enum to a Shared String or a char. It looks like the idea is this is
    //to be used when creating a slint platform, and sending key events to the window
    //It's not really designed for what I'm using it for it seems.
    //I can't convert the shared string from the key event into a Key enum it seems,
    //and then match against the Key enum...so that means I need to convert every
    //instance of Key enum into a SharedString and then match against that....except I
    //can't actually "match" a SharedString against a variable SharedString...so this becomes a
    //massive if / else if block....
    //I have to be missing something...there has to be a better way than this massive if/else if block

    if text == SharedString::from(Key::Backspace) {
        Some("Bcksp")
    } else if text == SharedString::from(Key::Tab) {
        Some("Tab")
    } else if text == SharedString::from(Key::Return) {
        Some("Return")
    } else if text == SharedString::from(Key::Escape) {
        Some("Esc")
    } else if text == SharedString::from(Key::Backtab) {
        Some("BckTab")
    } else if text == SharedString::from(Key::Delete) {
        Some("Del")
    } else if text == SharedString::from(Key::CapsLock) {
        Some("CapsLk")
    } else if text == SharedString::from(Key::UpArrow) {
        Some("↑")
    } else if text == SharedString::from(Key::DownArrow) {
        Some("↓")
    } else if text == SharedString::from(Key::LeftArrow) {
        Some("→")
    } else if text == SharedString::from(Key::RightArrow) {
        Some("←")
    } else if text == SharedString::from(Key::F1) {
        Some("F1")
    } else if text == SharedString::from(Key::F2) {
        Some("F2")
    } else if text == SharedString::from(Key::F3) {
        Some("F3")
    } else if text == SharedString::from(Key::F4) {
        Some("F4")
    } else if text == SharedString::from(Key::F5) {
        Some("F5")
    } else if text == SharedString::from(Key::F6) {
        Some("F6")
    } else if text == SharedString::from(Key::F7) {
        Some("F7")
    } else if text == SharedString::from(Key::F8) {
        Some("F8")
    } else if text == SharedString::from(Key::F9) {
        Some("F9")
    } else if text == SharedString::from(Key::F10) {
        Some("F10")
    } else if text == SharedString::from(Key::F11) {
        Some("F11")
    } else if text == SharedString::from(Key::F12) {
        Some("F12")
    } else if text == SharedString::from(Key::F13) {
        Some("F13")
    } else if text == SharedString::from(Key::F14) {
        Some("F14")
    } else if text == SharedString::from(Key::F15) {
        Some("F15")
    } else if text == SharedString::from(Key::F16) {
        Some("F16")
    } else if text == SharedString::from(Key::F17) {
        Some("F17")
    } else if text == SharedString::from(Key::F18) {
        Some("F18")
    } else if text == SharedString::from(Key::F19) {
        Some("F19")
    } else if text == SharedString::from(Key::F20) {
        Some("F20")
    } else if text == SharedString::from(Key::F21) {
        Some("F21")
    } else if text == SharedString::from(Key::F22) {
        Some("F22")
    } else if text == SharedString::from(Key::F23) {
        Some("F23")
    } else if text == SharedString::from(Key::F24) {
        Some("F24")
    } else if text == SharedString::from(Key::Insert) {
        Some("Ins")
    } else if text == SharedString::from(Key::Home) {
        Some("Home")
    } else if text == SharedString::from(Key::End) {
        Some("End")
    } else if text == SharedString::from(Key::PageUp) {
        Some("PgUp")
    } else if text == SharedString::from(Key::PageDown) {
        Some("PgDwn")
    } else if text == SharedString::from(Key::ScrollLock) {
        Some("ScrLk")
    } else if text == SharedString::from(Key::Pause) {
        Some("Pause")
    } else if text == SharedString::from(Key::SysReq) {
        Some("SysReq")
    } else if text == SharedString::from(Key::Stop) {
        Some("Stop")
    } else if text == SharedString::from(Key::Menu) {
        Some("Menu")
    } else {
        None
    }
}