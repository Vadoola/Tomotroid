/*#![warn(
    clippy::all,
    clippy::pedantic,
    //clippy::cargo,
)]*/

#[cfg(unix)]
use std::io::Cursor;

use anyhow::Result;
use hex_color::HexColor;
use serde::{Deserialize, Serialize};
use serde_json::json;
use settings::{JsonSettings, GlobalShortcuts};
use slint::{Color, ModelRc, Timer, TimerMode, VecModel};
use std::{
    fs::File,
    io::{BufReader, Read},
    rc::Rc,
    sync::mpsc,
};
use tray_item::{IconSource, TrayItem};
use walkdir::WalkDir;

mod settings;

slint::include_modules!();

pub const LOGO_BYTES: &str = include_str!("../assets/logo.svg");
pub const PROG_BYTES: &str = include_str!("../assets/ProgressCircle.svg");

//Right now serde support in Slint is new and crude, some of the types in the Slint version
//of this struct like Brush don't support serde yet. So for now I'm creating 2 versions
//the slint version and this version to manually convert between them.
#[derive(Deserialize)]
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

#[derive(Deserialize)]
struct JsonThemeTemp {
    name: String,
    colors: ThemeColors,
}

//I realize implemeting From is more idomatic, but that would require creating a newtype for JsonThemeTemp,
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

enum TrayMsg {
    MinRes,
    Quit,
}

fn color_to_hex_string(color: slint::Color) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        color.red(),
        color.green(),
        color.blue()
    )
}

fn update_prg_svg(bg_clr: slint::Color, fg_clr: slint::Color, rem_per: f32) -> slint::Image {
    //     stroke-dasharray="100, 100"
    slint::Image::load_from_svg_data(
        PROG_BYTES
            .replace(
                "stroke:#9ca5b5",
                &format!("stroke:{}", color_to_hex_string(bg_clr)),
            )
            //for now I'll just set this to the focus round, but it actually depends on what timer is active
            .replace(
                "stroke:#ff4e4d",
                &format!("stroke:{}", color_to_hex_string(fg_clr)),
            )
            .replace(
                "stroke-dasharray=\"100, 100\"",
                &format!("stroke-dasharray=\"{}, 100\"", rem_per),
            )
            .as_bytes(),
    )
    .unwrap()
}

impl Main {
    fn load_settings(&self) {
        let settings = settings::load_settings();
        self.global::<Settings>()
            .set_always_on_top(settings.always_on_top);
        self.global::<Settings>()
            .set_auto_start_break_timer(settings.auto_start_break_timer);
        self.global::<Settings>()
            .set_auto_start_work_timer(settings.auto_start_work_timer);
        self.global::<Settings>()
            .set_break_always_on_top(settings.break_always_on_top);
        self.global::<Settings>()
            .set_min_to_tray(settings.min_to_tray);
        self.global::<Settings>()
            .set_min_to_tray_on_close(settings.min_to_tray_on_close);
        self.global::<Settings>()
            .set_notifications(settings.notifications);
        self.global::<Settings>()
            .set_tick_sounds(settings.tick_sounds);
        self.global::<Settings>()
            .set_tick_sounds_during_break(settings.tick_sounds_during_break);
        self.global::<Settings>()
            .set_time_long_break(settings.time_long_break);
        self.global::<Settings>()
            .set_time_short_break(settings.time_short_break);
        self.global::<Settings>()
            .set_time_work(settings.time_work);
        self.global::<Settings>().set_volume(settings.volume);
        self.global::<Settings>()
            .set_work_rounds(settings.work_rounds);
    }

    fn save_settings(&self) {
        settings::save_settings(
            JsonSettings {
                always_on_top: self.global::<Settings>().get_always_on_top(),
                auto_start_break_timer: self.global::<Settings>().get_auto_start_break_timer(),
                auto_start_work_timer: self.global::<Settings>().get_auto_start_work_timer(),
                break_always_on_top: self.global::<Settings>().get_break_always_on_top(),

                //don't have the global shortcuts working yet...just temporarily create some defaults
                global_shortcuts: GlobalShortcuts {
                    call_timer_reset: String::from("Control+F2"),
                    call_timer_skip: String::from("Control+F3"),
                    call_timer_toggle: String::from("Control+F1"),
                },


                min_to_tray: self.global::<Settings>().get_min_to_tray(),
                min_to_tray_on_close: self.global::<Settings>().get_min_to_tray_on_close(),
                notifications: self.global::<Settings>().get_notifications(),

                //Haven't decided how I'm going to handle the theme yet. Just have the theme name here
                //in the settings? Put it into the global theme objects? etc
                //for now I'll just set this statically
                theme: String::from("Pomotroid"),

                tick_sounds: self.global::<Settings>().get_tick_sounds(),
                tick_sounds_during_break: self.global::<Settings>().get_tick_sounds_during_break(),
                time_long_break: self.global::<Settings>().get_time_long_break(),
                time_short_break: self.global::<Settings>().get_time_short_break(),
                time_work: self.global::<Settings>().get_time_work(),
                volume: self.global::<Settings>().get_volume(),
                work_rounds: self.global::<Settings>().get_work_rounds(),
            }
        );
    }
}

fn main() -> Result<()> {
    //TODO: I'm not seeing an obvious way to mimic the Pomotroid behavoir
    //where it just minimizes or restores by clicking the tray icon
    //because I don't see any way to capture when the tray icon is clicked
    //I'll need to dig into this more. For now I'll just add some menu items
    //to get some basic functionality and test minimzing to the tray etc
    #[cfg(unix)]
    let mut tray = {
        let logo_cursor = Cursor::new(include_bytes!("../assets/icons/logo.png"));
        let logo_decoder = png::Decoder::new(logo_cursor);
        let mut logo_reader = logo_decoder.read_info().unwrap();
        let mut logo_buff = vec![0; logo_reader.output_buffer_size()];
        logo_reader.next_frame(&mut logo_buff).unwrap();

        let logo_icon = IconSource::Data {
            data: logo_buff,
            height: 32,
            width: 32,
        };
        TrayItem::new("Tomotroid\nClick to Restore", logo_icon).unwrap()
    };

    #[cfg(windows)]
    let mut tray = TrayItem::new(
        "Tomotroid\nClick to Restore",
        IconSource::Resource("logo-icon"),
    )
    .unwrap();

    let (tray_tx, tray_rx) = mpsc::sync_channel(1);

    let minres_tx = tray_tx.clone();
    tray.add_menu_item("Minimize / Restore", move || {
        minres_tx.send(TrayMsg::MinRes).unwrap();
    })
    .unwrap();

    let quit_tx = tray_tx; //.clone();
    tray.add_menu_item("Quit", move || {
        quit_tx.send(TrayMsg::Quit).unwrap();
    })
    .unwrap();

    slint::platform::set_platform(Box::new(i_slint_backend_winit::Backend::new().unwrap())).unwrap();

    let main = Main::new()?;

    main.load_settings();
    let set_handle = main.as_weak();
    //if this is being called when the value changes....why is it passing me the old value?
    //I guess this is being called instead of the Touch Area's callback? So the value isn't updating
    //until I do it here? But how will that work with the sliders? I can't just invert the value
    //like I can with the bools.
    main.global::<Settings>().on_bool_changed(move |set_type, val| {
        let set_handle = set_handle.upgrade().unwrap();
        match set_type {
            BoolSettTypes::AlwOnTop => {
                set_handle.global::<Settings>().set_always_on_top(!val);
            },
            BoolSettTypes::AutoStrtBreakTim => {
                set_handle.global::<Settings>().set_auto_start_break_timer(!val);
            },
            BoolSettTypes::AutoStrtWrkTim => {
                set_handle.global::<Settings>().set_auto_start_work_timer(!val);
            },
            BoolSettTypes::BrkAlwOnTop => {
                set_handle.global::<Settings>().set_break_always_on_top(!val);
            },
            BoolSettTypes::MinToTray => {
                set_handle.global::<Settings>().set_min_to_tray(!val);
            },
            BoolSettTypes::MinToTryCls => {
                set_handle.global::<Settings>().set_min_to_tray_on_close(!val);
            },
            BoolSettTypes::Notifications => {
                set_handle.global::<Settings>().set_notifications(!val);
            },
            BoolSettTypes::TickSounds => {
                set_handle.global::<Settings>().set_tick_sounds(!val);
            },
            BoolSettTypes::TickSoundsBreak => {
                set_handle.global::<Settings>().set_tick_sounds_during_break(!val);
            },
        }
        //write out settings?...not the most effecient way every change..but for now should be fine
        set_handle.save_settings();
    });

    let close_handle = main.as_weak();
    main.on_close_window(move || {
        //for now to test that saving the settings are working, I'm just going to set it to save on close
        let close_handle = close_handle.upgrade().unwrap();
        close_handle.save_settings();

        close_handle.hide().unwrap();

        //After I get the system tray working I'm going to want to hide the window instead of actually close it
        //if it's set to hide on close
        //i_slint_backend_winit::WinitWindowAccessor::with_winit_window(min_handle.window(), |win| win.set_visible(false));
    });

    let min_handle = main.as_weak();
    main.on_minimize_window(move || {
        let min_handle = min_handle.upgrade().unwrap();
        i_slint_backend_winit::WinitWindowAccessor::with_winit_window(min_handle.window(), |win| {
            win.set_minimized(true);
        });
    });

    let move_handle = main.as_weak();
    main.on_move_window(move || {
        let move_handle = move_handle.upgrade().unwrap();
        i_slint_backend_winit::WinitWindowAccessor::with_winit_window(
            move_handle.window(),
            |win| win.drag_window(),
        );
    });

    let tray_handle = main.as_weak();
    let _tray_rec_thread = std::thread::spawn(move || loop {
        match tray_rx.recv() {
            Ok(TrayMsg::MinRes) => {
                let tray_handle_copy = tray_handle.clone();
                slint::invoke_from_event_loop(move || {
                    let main = tray_handle_copy.upgrade().unwrap();
                    i_slint_backend_winit::WinitWindowAccessor::with_winit_window(
                        main.window(),
                        |win| {
                            win.set_minimized(!win.is_minimized().unwrap());
                            win.focus_window();
                        },
                    );
                })
                .unwrap();
            }
            Ok(TrayMsg::Quit) => {
                let tray_handle_copy = tray_handle.clone();
                slint::invoke_from_event_loop(move || {
                    tray_handle_copy.upgrade().unwrap().hide().unwrap();
                })
                .unwrap();
            }
            _ => {}
        }
    });

    main.global::<HLClick>().on_hl_clicked(|url| {
        open::that(url.as_str()).unwrap();
    });

    let thm_handle = main.as_weak();
    main.global::<ThemeCallbacks>()
        .on_theme_changed(move |theme| {
            let thm_handle = thm_handle.upgrade().unwrap();
            thm_handle.set_logo(
                slint::Image::load_from_svg_data(
                    LOGO_BYTES
                        .replace(
                            "stroke:#2f384b",
                            &format!("stroke:{}", color_to_hex_string(theme.background.color())),
                        )
                        .replace(
                            "fill:#ff4e4d",
                            &format!("fill:{}", color_to_hex_string(theme.focus_round.color())),
                        )
                        .replace(
                            "fill:#992e2e",
                            &format!(
                                "fill:{}",
                                color_to_hex_string(theme.focus_round.color().darker(0.4))
                            ),
                        )
                        .replace(
                            "fill:#f6f2eb",
                            &format!("fill:{}", color_to_hex_string(theme.foreground.color())),
                        )
                        .replace(
                            "fill:#05ec8c",
                            &format!("fill:{}", color_to_hex_string(theme.accent.color())),
                        )
                        .as_bytes(),
                )
                .unwrap(),
            );

            thm_handle.set_circ_progress(update_prg_svg(
                theme.background_lightest.color(),
                theme.focus_round.color(),
                75.0,
            ));
        });

    main.global::<ThemeCallbacks>().on_load_themes(move || {
        let mut theme_dir = std::env::current_dir().unwrap();
        theme_dir.push("themes");
        let themes: Vec<JsonTheme> = {
            //I'm thinking I need to move this into the settings modules maybe?
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
                        let theme = serde_json::from_str::<JsonThemeTemp>(&theme)
                            .unwrap()
                            .into();
                        theme
                    })
                    .ok()
                })
                .collect();
            themes.sort_by(|a, b| a.name.partial_cmp(&b.name).unwrap());
            themes
        };

        let model: Rc<VecModel<JsonTheme>> = Rc::new(VecModel::from(themes));

        ModelRc::from(model.clone())
    });

    main.global::<ThemeCallbacks>().invoke_load_themes();

    let timer = Timer::default();
    let timer_handle = main.as_weak();
    main.on_action_timer(move |action| {
        let tmrstrt_handle = timer_handle.clone();
        let timer_handle = timer_handle.upgrade().unwrap();
        match action {
            TimerAction::Start => {
                timer_handle.set_running(true);
                timer.start(
                    TimerMode::Repeated,
                    std::time::Duration::from_millis(50),
                    move || {
                        let tmrstrt_handle = tmrstrt_handle.unwrap();
                        tmrstrt_handle.invoke_tick(50);
                        let rem_per = tmrstrt_handle.get_remaining_time() as f32
                            / tmrstrt_handle.get_current_timer() as f32
                            * 100.0;

                        let fg_clr = match tmrstrt_handle.get_active_timer() {
                            ActiveTimer::Focus => {
                                tmrstrt_handle.global::<Theme>().get_focus_round().color()
                            }
                            ActiveTimer::ShortBreak => {
                                tmrstrt_handle.global::<Theme>().get_short_round().color()
                            }
                            ActiveTimer::LongBreak => {
                                tmrstrt_handle.global::<Theme>().get_long_round().color()
                            }
                        };

                        tmrstrt_handle.set_circ_progress(update_prg_svg(
                            tmrstrt_handle
                                .global::<Theme>()
                                .get_background_lightest()
                                .color(),
                            fg_clr,
                            rem_per,
                        ));
                    },
                )
            }
            TimerAction::Stop => {
                timer_handle.set_running(false);
                timer.stop();
            }
            TimerAction::Reset => {
                timer.stop();
                timer_handle.set_running(false);
                timer_handle.set_remaining_time(timer_handle.get_current_timer());

                let fg_clr = match timer_handle.get_active_timer() {
                    ActiveTimer::Focus => timer_handle.global::<Theme>().get_focus_round().color(),
                    ActiveTimer::ShortBreak => {
                        timer_handle.global::<Theme>().get_short_round().color()
                    }
                    ActiveTimer::LongBreak => {
                        timer_handle.global::<Theme>().get_long_round().color()
                    }
                };

                timer_handle.set_circ_progress(update_prg_svg(
                    timer_handle
                        .global::<Theme>()
                        .get_background_lightest()
                        .color(),
                    fg_clr,
                    100.0,
                ));
                //need to be updating the running status from Rust not slint
            }
        }
    });

    main.run()?;
    Ok(())
}
