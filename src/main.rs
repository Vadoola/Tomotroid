/*#![warn(
    clippy::all,
    clippy::pedantic,
    //clippy::cargo,
)]*/
#![windows_subsystem = "windows"]

#[cfg(unix)]
use std::io::Cursor;

use anyhow::Result;
use hex_color::HexColor;
use i_slint_backend_winit::winit::event::Event;
use i_slint_backend_winit::winit::keyboard::KeyCode;
use notify_rust::{Notification, Hint};
use serde::{Deserialize, Serialize};
use serde_json::json;
use settings::{get_non_print_key_txt, GlobalShortcuts, JsonHotKey, JsonSettings};
use single_instance::SingleInstance;
use slint::{platform::Key, SharedString, Color, JoinHandle, ModelRc, Timer, TimerMode, VecModel, Model, PlatformError};
use slint::private_unstable_api::re_exports::EventResult;
use std::{
    borrow::Borrow, fs::File, io::{BufReader, Read}, rc::Rc, str::FromStr, sync::mpsc
};
use tray_item::{IconSource, TrayItem};
use global_hotkey::GlobalHotKeyEvent;
use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager,
};

mod settings;

slint::include_modules!();

pub const LOGO_BYTES: &str = include_str!("../assets/logo.svg");
pub const PROG_BYTES: &str = include_str!("../assets/ProgressCircle.svg");

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

impl Main {
    fn set_settings(&self, settings: &JsonSettings) {
        self.global::<Settings>()
            .set_always_on_top(settings.always_on_top);
        self.global::<Settings>()
            .set_auto_start_break_timer(settings.auto_start_break_timer);
        self.global::<Settings>()
            .set_auto_start_work_timer(settings.auto_start_work_timer);
        self.global::<Settings>()
            .set_break_always_on_top(settings.break_always_on_top);

        //Global Shortcuts
        self.global::<Settings>()
            .set_tt_ghk(settings.global_shortcuts.call_timer_toggle.to_string().into());
        self.global::<Settings>()
            .set_rst_ghk(settings.global_shortcuts.call_timer_reset.to_string().into());
        self.global::<Settings>()
            .set_skp_ghk(settings.global_shortcuts.call_timer_skip.to_string().into());


        self.global::<Settings>()
            .set_min_to_tray(settings.min_to_tray);
        self.global::<Settings>()
            .set_min_to_tray_on_close(settings.min_to_tray_on_close);
        self.global::<Settings>()
            .set_notifications(settings.notifications);
        self.global::<Settings>()
            .set_theme((&settings.theme).into());
        self.global::<Settings>()
            .set_tick_sounds(settings.tick_sounds);
        self.global::<Settings>()
            .set_tick_sounds_during_break(settings.tick_sounds_during_break);
        self.global::<Settings>()
            .set_time_long_break(settings.time_long_break);
        self.global::<Settings>()
            .set_time_short_break(settings.time_short_break);
        self.global::<Settings>().set_time_work(settings.time_work);
        self.global::<Settings>().set_volume(settings.volume);
        self.global::<Settings>()
            .set_work_rounds(settings.work_rounds);

        self.global::<Settings>().set_is_wayland(settings::is_wayland());
    }

    fn save_settings(&self) {
        settings::save_settings(
            JsonSettings {
                always_on_top: self.global::<Settings>().get_always_on_top(),
                auto_start_break_timer: self.global::<Settings>().get_auto_start_break_timer(),
                auto_start_work_timer: self.global::<Settings>().get_auto_start_work_timer(),
                break_always_on_top: self.global::<Settings>().get_break_always_on_top(),

                global_shortcuts: GlobalShortcuts {
                    call_timer_reset: JsonHotKey::from_str(self.global::<Settings>().get_rst_ghk().to_string().as_str()).expect("a valid Timer Reset GHK"),
                    call_timer_skip: JsonHotKey::from_str(self.global::<Settings>().get_skp_ghk().to_string().as_str()).expect("a valid Timer Skip GHK"),
                    call_timer_toggle: JsonHotKey::from_str(self.global::<Settings>().get_tt_ghk().to_string().as_str()).expect("a valid Timer Toggle GHK"),
                },

                min_to_tray: self.global::<Settings>().get_min_to_tray(),
                min_to_tray_on_close: self.global::<Settings>().get_min_to_tray_on_close(),
                notifications: self.global::<Settings>().get_notifications(),

                theme: self.global::<Settings>().get_theme().to_string(),

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

struct Tomotroid {
    pub window: Main,
    settings: JsonSettings,
    reset: Option<HotKey>,
    skip: Option<HotKey>,
    toggle: Option<HotKey>,
    ghk_manager: GlobalHotKeyManager,
}

impl Tomotroid {
    fn new() -> Self {
        let settings = settings::load_settings();
        let themes = settings::load_themes();

        let ghk_manager = GlobalHotKeyManager::new().unwrap();
        let toggle = &settings.global_shortcuts.call_timer_toggle.borrow().into();
        let reset = &settings.global_shortcuts.call_timer_reset.borrow().into();
        let skip = &settings.global_shortcuts.call_timer_skip.borrow().into();

        let toggle = ghk_manager.register(*toggle).map_or(None, |_|Some(*toggle));
        let reset = ghk_manager.register(*reset).map_or(None, |_|Some(*reset));
        let skip = ghk_manager.register(*skip).map_or(None, |_|Some(*skip));

        let window = Main::new().unwrap();
        window.set_settings(&settings);

        let model: Rc<VecModel<JsonTheme>> = Rc::new(VecModel::from(themes));
        window.global::<ThemeCallbacks>().set_themes(ModelRc::from(model.clone()));

        Self {
            window,
            settings,
            reset,
            skip,
            toggle,
            ghk_manager,
        }
    }

    fn update_prg_svg(bg_clr: slint::Color, fg_clr: slint::Color, rem_per: f32) -> slint::Image {
        slint::Image::load_from_svg_data(
            PROG_BYTES
                .replace(
                    "stroke:#9ca5b5",
                    &format!("stroke:{}", color_to_hex_string(bg_clr)),
                )
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

    fn run(&self) -> Result<(), PlatformError> {
        let thm_name = &self.settings.theme;
        let themes = self.window.global::<ThemeCallbacks>().get_themes();
        let (idx, cur_theme) = themes
            .iter()
            .enumerate()
            .find(|(_, thm)| thm.name == thm_name)
            .unwrap();
        self.window.global::<ThemeCallbacks>().invoke_theme_changed(idx as i32, cur_theme.clone());

        self.window.run()
    }
}

fn main() -> Result<()> {
    let instance = SingleInstance::new("org.vadoola.tomotroid").unwrap();
    if !instance.is_single() {
        return Err(anyhow::anyhow!(
            "Only one instance of Tomotroid is allowed to run"
        ));
    }

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

    let quit_tx = tray_tx;
    tray.add_menu_item("Quit", move || {
        quit_tx.send(TrayMsg::Quit).unwrap();
    })
    .unwrap();

    let backend = {
        #[cfg(target_os = "macos")]
        {
            use i_slint_backend_winit::winit::platform::macos::WindowBuilderExtMacOS;

            let mut backend = i_slint_backend_winit::Backend::new().unwrap();
            backend.window_builder_hook = Some(Box::new(|builder| {
                builder.with_decorations(false)
            }));
            backend
        }

        #[cfg(not(target_os = "macos"))]
        i_slint_backend_winit::Backend::new().unwrap()
    };
    
    slint::platform::set_platform(Box::new(backend))
        .unwrap();

    let tomotroid = Tomotroid::new();

    let set_bool_handle = tomotroid.window.as_weak();
    //if this is being called when the value changes....why is it passing me the old value?
    //I guess this is being called instead of the Touch Area's callback? So the value isn't updating
    //until I do it here? But how will that work with the sliders? I can't just invert the value
    //like I can with the bools.
    tomotroid
        .window
        .global::<Settings>()
        .on_bool_changed(move |set_type, val| {
            let set_handle = set_bool_handle.upgrade().unwrap();
            match set_type {
                BoolSettTypes::AlwOnTop => {
                    set_handle.global::<Settings>().set_always_on_top(!val);
                }
                BoolSettTypes::AutoStrtBreakTim => {
                    set_handle
                        .global::<Settings>()
                        .set_auto_start_break_timer(!val);
                }
                BoolSettTypes::AutoStrtWrkTim => {
                    set_handle
                        .global::<Settings>()
                        .set_auto_start_work_timer(!val);
                }
                BoolSettTypes::BrkAlwOnTop => {
                    set_handle
                        .global::<Settings>()
                        .set_break_always_on_top(!val);
                }
                BoolSettTypes::MinToTray => {
                    set_handle.global::<Settings>().set_min_to_tray(!val);
                }
                BoolSettTypes::MinToTryCls => {
                    set_handle
                        .global::<Settings>()
                        .set_min_to_tray_on_close(!val);
                }
                BoolSettTypes::Notifications => {
                    set_handle.global::<Settings>().set_notifications(!val);
                }
                BoolSettTypes::TickSounds => {
                    set_handle.global::<Settings>().set_tick_sounds(!val);
                }
                BoolSettTypes::TickSoundsBreak => {
                    set_handle
                        .global::<Settings>()
                        .set_tick_sounds_during_break(!val);
                }
            }
            //write out settings?...not the most effecient way every change..but for now should be fine
            set_handle.save_settings();
        });

    let ghk_handle = tomotroid.window.as_weak();
    let ghk_receiver = GlobalHotKeyEvent::receiver();
    let _thread = std::thread::spawn(move || loop {
        let ghk_handle2 = ghk_handle.clone();
        slint::invoke_from_event_loop(move || {
            if let Ok(event) = ghk_receiver.try_recv() {
                if event.state() == global_hotkey::HotKeyState::Released {
                    let ghk_handle2 = ghk_handle2.upgrade().unwrap();
                    match event.id() {
                        tg_id if tomotroid.toggle.is_some_and(|toggle| toggle.id() == tg_id) => {
                            let action = if ghk_handle2.get_running() {
                                TimerAction::Stop
                            } else {
                                TimerAction::Start
                            };
                            ghk_handle2.invoke_action_timer(action);
                        }
                        rst_id if tomotroid.reset.is_some_and(|reset| reset.id() == rst_id) => {
                            ghk_handle2.invoke_action_timer(TimerAction::Reset);
                        }
                        skp_id if tomotroid.skip.is_some_and(|skip| skip.id() == skp_id) => {
                            ghk_handle2.invoke_action_timer(TimerAction::Skip);
                        }
                        _ => {}
                    }
                }
            }
        })
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(500));
    });

    let set_int_handle = tomotroid.window.as_weak();
    tomotroid
        .window
        .global::<Settings>()
        .on_int_changed(move |set_type, val| {
            let set_handle = set_int_handle.upgrade().unwrap();
            match set_type {
                IntSettTypes::LongBreak => {
                    set_handle.global::<Settings>().set_time_long_break(val);
                }
                IntSettTypes::ShortBreak => {
                    set_handle.global::<Settings>().set_time_short_break(val);
                }
                IntSettTypes::Work => {
                    set_handle.global::<Settings>().set_time_work(val);
                }
                IntSettTypes::Volume => {
                    set_handle.global::<Settings>().set_volume(val);
                }
                IntSettTypes::Rounds => {
                    set_handle.global::<Settings>().set_work_rounds(val);
                }
            }

            //write out settings?...not the most effecient way every change..but for now should be fine
            set_handle.save_settings();
        });

    // settings not currently being saved
    //    * Global Shortcuts (which can't even be set currently)

    let close_handle = tomotroid.window.as_weak();
    tomotroid.window.on_close_window(move || {
        let close_handle = close_handle.upgrade().unwrap();
        close_handle.save_settings();

        close_handle.hide().unwrap();

        //After I get the system tray working I'm going to want to hide the window instead of actually close it
        //if it's set to hide on close
        //i_slint_backend_winit::WinitWindowAccessor::with_winit_window(min_handle.window(), |win| win.set_visible(false));
    });

    let min_handle = tomotroid.window.as_weak();
    tomotroid.window.on_minimize_window(move || {
        let min_handle = min_handle.upgrade().unwrap();
        i_slint_backend_winit::WinitWindowAccessor::with_winit_window(min_handle.window(), |win| {
            win.set_minimized(true);
        });
    });

    let move_handle = tomotroid.window.as_weak();
    tomotroid.window.on_move_window(move || {
        let move_handle = move_handle.upgrade().unwrap();
        i_slint_backend_winit::WinitWindowAccessor::with_winit_window(
            move_handle.window(),
            |win| win.drag_window(),
        );
    });

    let tray_handle = tomotroid.window.as_weak();
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

    tomotroid.window.global::<HLClick>().on_hl_clicked(|url| {
        open::that(url.as_str()).unwrap();
    });

    let thm_handle = tomotroid.window.as_weak();
    tomotroid
        .window
        .global::<ThemeCallbacks>()
        .on_theme_changed(move |idx, theme| {
            //how the hell have my changes to enable GHK support broken the ThemeChanged callback?
            let thm_handle = thm_handle.upgrade().unwrap();
            thm_handle.global::<Settings>().set_theme(theme.name);
            thm_handle.save_settings();

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

            let rem_per = thm_handle.get_remaining_time() as f32
                / thm_handle.get_target_time() as f32
                * 100.0;
            thm_handle.set_circ_progress(Tomotroid::update_prg_svg(
                theme.background_lightest.color(),
                theme.focus_round.color(),
                rem_per,
            ));

            thm_handle.global::<Theme>().set_theme_idx(idx);
            thm_handle
                .global::<Theme>()
                .set_long_round(theme.long_round);
            thm_handle
                .global::<Theme>()
                .set_short_round(theme.short_round);
            thm_handle
                .global::<Theme>()
                .set_focus_round(theme.focus_round);
            thm_handle
                .global::<Theme>()
                .set_background(theme.background);
            thm_handle
                .global::<Theme>()
                .set_background_light(theme.background_light);
            thm_handle
                .global::<Theme>()
                .set_background_lightest(theme.background_lightest);
            thm_handle
                .global::<Theme>()
                .set_foreground(theme.foreground);
            thm_handle
                .global::<Theme>()
                .set_foreground_darker(theme.foreground_darker);
            thm_handle
                .global::<Theme>()
                .set_foreground_darkest(theme.foreground_darkest);
            thm_handle.global::<Theme>().set_accent(theme.accent);
        });

    let timer = Timer::default();
    let timer_handle = tomotroid.window.as_weak();
    tomotroid.window.on_action_timer(move |action| {
        //Notification::new().summary("Performing an Action").show().unwrap();
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
                            / tmrstrt_handle.get_target_time() as f32
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

                        tmrstrt_handle.set_circ_progress(Tomotroid::update_prg_svg(
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
                timer_handle.set_remaining_time(timer_handle.get_target_time());

                let fg_clr = match timer_handle.get_active_timer() {
                    ActiveTimer::Focus => timer_handle.global::<Theme>().get_focus_round().color(),
                    ActiveTimer::ShortBreak => {
                        timer_handle.global::<Theme>().get_short_round().color()
                    }
                    ActiveTimer::LongBreak => {
                        timer_handle.global::<Theme>().get_long_round().color()
                    }
                };

                timer_handle.set_circ_progress(Tomotroid::update_prg_svg(
                    timer_handle
                        .global::<Theme>()
                        .get_background_lightest()
                        .color(),
                    fg_clr,
                    100.0,
                ));
                //need to be updating the running status from Rust not slint
            }
            TimerAction::Skip => {
                //timer_handle.set_remaining_time(0);
                timer_handle.invoke_change_timer();
            }
        }
    });

    let chg_tmr_handle = tomotroid.window.as_weak();
    tomotroid.window.on_change_timer(move || {
        let chg_tmr_handle = chg_tmr_handle.upgrade().unwrap();
        match chg_tmr_handle.get_active_timer() {
            ActiveTimer::Focus => {
                if !chg_tmr_handle.global::<Settings>().get_auto_start_break_timer() {
                    chg_tmr_handle.invoke_action_timer(TimerAction::Stop);
                }
                let body_str = if chg_tmr_handle.get_active_round() == chg_tmr_handle.get_tmr_config().rounds {
                    let lgbrk_time = chg_tmr_handle.get_tmr_config().lgbrk_time;

                    chg_tmr_handle.set_active_round(1);
                    chg_tmr_handle.set_active_timer(ActiveTimer::LongBreak);

                    chg_tmr_handle.set_target_time(lgbrk_time);
                    chg_tmr_handle.set_remaining_time(lgbrk_time);
                    format!("Begin a {} minute long break.", lgbrk_time / 60000)
                } else {
                    let shbrk_time = chg_tmr_handle.get_tmr_config().shbrk_time;
                    chg_tmr_handle.set_active_timer(ActiveTimer::ShortBreak);
                    chg_tmr_handle.set_target_time(shbrk_time);
                    chg_tmr_handle.set_remaining_time(shbrk_time);
                    format!("Begin a {} minute short break.", shbrk_time / 60000)
                };
                Notification::new()
                    //.appname("Tomotroid")
                    //.icon("../assets/logo.png")
                    .summary("Focus Round Complete")
                    .body(&body_str)
                    .show().unwrap();
            }
            brk_type => {
                if !chg_tmr_handle.global::<Settings>().get_auto_start_work_timer() {
                    chg_tmr_handle.invoke_action_timer(TimerAction::Stop);
                }

                let focus_time = chg_tmr_handle.get_tmr_config().focus_time;
                chg_tmr_handle.set_active_round(
                    if brk_type == ActiveTimer::ShortBreak {
                        chg_tmr_handle.get_active_round() + 1
                    } else {
                        1
                    }
                );
                chg_tmr_handle.set_active_timer(ActiveTimer::Focus);
                chg_tmr_handle.set_target_time(focus_time);
                chg_tmr_handle.set_remaining_time(focus_time);
                Notification::new()
                    //.appname("Tomotroid")
                    //.icon("../assets/logo.png")
                    .summary("Break Finished")
                    .body(&format!("Begin focusing for {} minutes.", focus_time / 60000))
                    .show().unwrap();
            }
        }
    });

    let ghk_handle = tomotroid.window.as_weak();
    tomotroid
        .window
        .global::<ConfigCallbacks>()
        .on_new_ghk(move |ghk, event| {
            //ok so the basic concept of this is working, I'm getting the keys on release
            //and I'm getting the keys with the modifier...but of course I also get another firing
            //when I release the modifiers afterward. IE if I press left Ctrl+D, and release the D
            //first I get the correct combination I would expect. But then when I release the Ctrl
            //I get an event where the modifiers are all false, and then event text is "\x11" which
            //according to an Ascii chart is "Device Control 1". If I do the same with the right Ctrl
            //key I actually get a text of "\x16"...which is "Synchronous idle"? either way I need to
            //be able to filter these out?, but what I need to filter depends on the modifer used
            //It make sense that any global hotkey would use a modifier. It would cause problems to
            //just set a global hotkey to the D key, anytime you needed to type D and the program was
            //running you couldn't. So I had the thought to just reject all events where there are no modifiers
            //but what if they want the GHK to just be F8 for example....that wouldn't have any modifiers
            //I can't look at the repeat key, because per the documentation and my testing repeat is always
            //false for key release....might I have to do some sort of odd handshaking? Put in a key pressed
            //event look at what's pressed look for repeat, set some sort of flag and then look for a set flag
            //on the release? Another Option I guess would be to filter to some sort of good characters?
            //IE if the text isn't a-z, F-keys, etc reject it....but I could end up rejecting valid combinations easily
            //especially perhaps if someone has an international keyboard I would assume I would miss also sorts of valid
            //key combinations that I'm just unaware of for other language keyboards.
            
            //Ok this current code may not have as many issues as I thought
            //I added in some crude support for a setting string tied to each global shortcut, and assigned that to the
            //text on the Config page. This mostly seems to work, The Focus Scope even looses focus after I accept the input
            //There are some odities
            //  * If I use the F1 keys I get a unicode replacement character...fair enough, that will probably need special handling
            //      * It looks like all the Function keys come back as "xEF" though....Might need to open a discussion
            //      * or ticket with Slint about how this would work?
            //  * Ctrl+Shift+Char works, Shift+Alt+Char works, but Ctrl+Alt seems to add other characters...this could be a windows
            //      * thing for example Ctrl-Alt+A inserts á...but if I hit Ctrl+Alt+A in VSCodium here I also get a á, so that must
            //      * be something Windows 11 is doing for alternate character support.
            //      * I haven't tested any of this on Linux yets

            //I guess it's reasonable to assume a global hotkey is going to be 1 or more modifiers + 1 non-modifier key?
            //which would eliminate this problem (by eliminate it I mean sweep it under the rug)
            //I guess that would also make it reasonable for the FocusScope to just reject anything where a modifier wasn't pressed

            let ghk_handle = ghk_handle.upgrade().unwrap();
            if (!event.modifiers.control && !event.modifiers.alt && !event.modifiers.shift && !event.modifiers.meta) || event.text == SharedString::from(Key::Tab) {
                //this below seems wasteful resource wise. I'm setting the string to blank, and then setting it back
                //to the original string. In the FocusScope the focused property is out, so I can't edit it, I can only read it
                //which means I can't tell the FocusScope to loose focus after a new GHK is accepted, or the Esc key is pressed.
                //It seems to loose focus when I set new text....but it has to be NEW text. If I just call set and use the current
                //value it actually doesn't work...so I have to blank the string then reset it to what it was.
                //I tried moving the EventResult return from inside the slint code to here, and rejecting it if Esc was pressed
                //etc thinking that would work...but it didn't.
                //While wastefull from an execution and performance perspective as of right now this is the only way I can get it
                //working the way I want. I Might need to dig deeper into how the Focus Handling works, I'm sure there is
                //a better way to do this
                match ghk {
                    GHKShortcuts::ToggleTimer => {
                        let pre = ghk_handle.global::<Settings>().get_tt_ghk();
                        ghk_handle.global::<Settings>().set_tt_ghk(SharedString::new());
                        ghk_handle.global::<Settings>().set_tt_ghk(pre);
                    },
                    GHKShortcuts::ResetTimer => {
                        let pre = ghk_handle.global::<Settings>().get_rst_ghk();
                        ghk_handle.global::<Settings>().set_rst_ghk(SharedString::new());
                        ghk_handle.global::<Settings>().set_rst_ghk(pre)
                    },
                    GHKShortcuts::SkipRound => {
                        let pre = ghk_handle.global::<Settings>().get_skp_ghk();
                        ghk_handle.global::<Settings>().set_skp_ghk(SharedString::new());
                        ghk_handle.global::<Settings>().set_skp_ghk(pre);
                    },
                }
            } else {
                let mut text = String::new();
                if event.modifiers.control {
                    text = "Control+".to_string();
                }
                if event.modifiers.alt {
                    text.push_str("Alt+");
                }
                if event.modifiers.shift {
                    text.push_str("Shift+");
                }
                if event.modifiers.meta {
                    text.push_str("Super+");
                }

                if let Some(non_pr_char) = get_non_print_key_txt(event.text.clone()) {
                    text.push_str(non_pr_char);
                } else {
                    text.push_str(&event.text.to_uppercase());
                }

                match ghk {
                    GHKShortcuts::ToggleTimer => ghk_handle.global::<Settings>().set_tt_ghk(text.into()),
                    GHKShortcuts::ResetTimer => ghk_handle.global::<Settings>().set_rst_ghk(text.into()),
                    GHKShortcuts::SkipRound => ghk_handle.global::<Settings>().set_skp_ghk(text.into()),
                }
                ghk_handle.save_settings();
            }
        });

    tomotroid.run()?;
    Ok(())
}
