#![warn(
    clippy::all,
    clippy::pedantic,
    //clippy::cargo,
)]
#![windows_subsystem = "windows"]

mod settings;
mod setup;

use crate::setup::TrayMsg;

use anyhow::Result;
use global_hotkey::GlobalHotKeyEvent;
use global_hotkey::{
    hotkey::{Code, HotKey},
    GlobalHotKeyManager,
};
use notify_rust::Notification;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use settings::{get_non_print_key_txt, GlobalShortcuts, JsonHotKey, JsonSettings};
use single_instance::SingleInstance;
use slint::{
    platform::Key, Model, ModelRc, PlatformError, SharedString, Timer, TimerMode, VecModel,
};
use std::io::Cursor;
use std::{borrow::Borrow, rc::Rc, str::FromStr};

use log::{error, info, warn};

slint::include_modules!();

pub const LOGO_BYTES: &str = include_str!("../assets/logo.svg");

pub const ALERT_LONG_BREAK: &[u8] = include_bytes!("../assets/audio/alert-long-break.ogg");
pub const ALERT_SHORT_BREAK: &[u8] = include_bytes!("../assets/audio/alert-short-break.ogg");
pub const ALERT_WORK: &[u8] = include_bytes!("../assets/audio/alert-work.ogg");
pub const TICK: &[u8] = include_bytes!("../assets/audio/tick.ogg");

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
            .set_tt_ghk(settings.global_shortcuts.toggle.to_string().into());
        self.global::<Settings>()
            .set_rst_ghk(settings.global_shortcuts.reset.to_string().into());
        self.global::<Settings>()
            .set_skp_ghk(settings.global_shortcuts.skip.to_string().into());

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

        self.global::<Settings>()
            .set_is_wayland(settings::is_wayland());
    }

    fn save_settings(&self) {
        settings::save_settings(&JsonSettings {
            always_on_top: self.global::<Settings>().get_always_on_top(),
            auto_start_break_timer: self.global::<Settings>().get_auto_start_break_timer(),
            auto_start_work_timer: self.global::<Settings>().get_auto_start_work_timer(),
            break_always_on_top: self.global::<Settings>().get_break_always_on_top(),

            global_shortcuts: GlobalShortcuts {
                reset: JsonHotKey::from_str(
                    self.global::<Settings>().get_rst_ghk().to_string().as_str(),
                )
                .expect("a valid Timer Reset GHK"),
                skip: JsonHotKey::from_str(
                    self.global::<Settings>().get_skp_ghk().to_string().as_str(),
                )
                .expect("a valid Timer Skip GHK"),
                toggle: JsonHotKey::from_str(
                    self.global::<Settings>().get_tt_ghk().to_string().as_str(),
                )
                .expect("a valid Timer Toggle GHK"),
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
        });
    }
}

struct Tomotroid {
    pub window: Main,
    settings: JsonSettings,
    reset: Option<HotKey>,
    skip: Option<HotKey>,
    toggle: Option<HotKey>,
    ghk_manager: GlobalHotKeyManager,
    audio_stream: OutputStream,
    audio_handle: OutputStreamHandle,
    audio_sink: Rc<Sink>,
    config_model: Rc<VecModel<ConfigData>>,
}

impl Tomotroid {
    fn new() -> Self {
        let settings = settings::load_settings();
        let themes = settings::load_themes();

        let ghk_manager = GlobalHotKeyManager::new().unwrap();
        let toggle = &settings.global_shortcuts.toggle.borrow().into();
        let reset = &settings.global_shortcuts.reset.borrow().into();
        let skip = &settings.global_shortcuts.skip.borrow().into();

        let toggle = ghk_manager
            .register(*toggle)
            .map_or(None, |()| Some(*toggle));
        let reset = ghk_manager.register(*reset).map_or(None, |()| Some(*reset));
        let skip = ghk_manager.register(*skip).map_or(None, |()| Some(*skip));

        let (audio_stream, audio_handle) = OutputStream::try_default().unwrap();
        let audio_sink = Rc::new(Sink::try_new(&audio_handle).unwrap());
        audio_sink.set_volume(settings.volume as f32 / 100.0);

        let window = Main::new().unwrap();
        window.set_settings(&settings);

        let theme_model: Rc<VecModel<JsonTheme>> = Rc::new(VecModel::from(themes));
        window
            .global::<ThemeCallbacks>()
            .set_themes(ModelRc::from(theme_model.clone()));

        let config_model = Rc::new(VecModel::from(vec![
            ConfigData {
                name: "Always On Top".into(),
                state: settings.always_on_top,
                sett_param: BoolSettTypes::AlwOnTop,
                enabled: !settings::is_wayland(),
                animate_in: false,
                animate_out: false,
            },
            ConfigData {
                name: "Deactivate Always On Top on Breaks".into(),
                state: settings.break_always_on_top,
                sett_param: BoolSettTypes::BrkAlwOnTop,
                enabled: !settings::is_wayland() && settings.always_on_top,
                animate_in: false,
                animate_out: false,
            }, //only shown when "Always On Top" is selected
            ConfigData {
                name: "Auto-start Work Timer".into(),
                state: settings.auto_start_work_timer,
                sett_param: BoolSettTypes::AutoStrtWrkTim,
                enabled: true,
                animate_in: false,
                animate_out: false,
            },
            ConfigData {
                name: "Auto-start Break Timer".into(),
                state: settings.auto_start_break_timer,
                sett_param: BoolSettTypes::AutoStrtBreakTim,
                enabled: true,
                animate_in: false,
                animate_out: false,
            },
            ConfigData {
                name: "Tick Sounds - Work".into(),
                state: settings.tick_sounds,
                sett_param: BoolSettTypes::TickSounds,
                enabled: true,
                animate_in: false,
                animate_out: false,
            },
            ConfigData {
                name: "Tick Sounds - Break".into(),
                state: settings.tick_sounds_during_break,
                sett_param: BoolSettTypes::TickSoundsBreak,
                enabled: true,
                animate_in: false,
                animate_out: false,
            },
            ConfigData {
                name: "Desktop Notifications".into(),
                state: settings.notifications,
                sett_param: BoolSettTypes::Notifications,
                enabled: true,
                animate_in: false,
                animate_out: false,
            },
            ConfigData {
                name: "Minimize to Tray".into(),
                state: settings.min_to_tray,
                sett_param: BoolSettTypes::MinToTray,
                enabled: true,
                animate_in: false,
                animate_out: false,
            },
            ConfigData {
                name: "Minimize to Tray on Close".into(),
                state: settings.min_to_tray_on_close,
                sett_param: BoolSettTypes::MinToTryCls,
                enabled: true,
                animate_in: false,
                animate_out: false,
            },
        ]));

        //window.global::<ConfigCallbacks>().set_configs(ModelRc::new(config_model.clone().filter(|cf| cf.enabled)));

        Self {
            window,
            settings,
            reset,
            skip,
            toggle,
            ghk_manager,
            audio_stream,
            audio_handle,
            audio_sink,
            config_model,
        }
    }

    fn run(&self) -> Result<(), PlatformError> {
        let thm_name = &self.settings.theme;
        let themes = self.window.global::<ThemeCallbacks>().get_themes();
        let (idx, cur_theme) = themes
            .iter()
            .enumerate()
            .find(|(_, thm)| thm.name == thm_name)
            .unwrap();
        self.window
            .global::<ThemeCallbacks>()
            .invoke_theme_changed(idx as i32, cur_theme.clone());

        self.window.run()
    }
}

impl BoolSettTypes {
    #[must_use]
    pub fn to_usize(&self) -> usize {
        match self {
            BoolSettTypes::AlwOnTop => 0,
            BoolSettTypes::BrkAlwOnTop => 1,
            BoolSettTypes::AutoStrtWrkTim => 2,
            BoolSettTypes::AutoStrtBreakTim => 3,
            BoolSettTypes::TickSounds => 4,
            BoolSettTypes::TickSoundsBreak => 5,
            BoolSettTypes::Notifications => 6,
            BoolSettTypes::MinToTray => 7,
            BoolSettTypes::MinToTryCls => 8,
        }
    }
}

//eventually I want to clean this main up and make it smaller, but for now I'll just
//surpress this clippy warning
#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    setup::logging();
    info!("Starting up");

    let instance = SingleInstance::new("org.vadoola.tomotroid").unwrap();
    if !instance.is_single() {
        error!("Only one instance of Tomotroid is allowed to run");
        return Err(anyhow::anyhow!(
            "Only one instance of Tomotroid is allowed to run"
        ));
    }

    let tray_rx = setup::tray().unwrap();

    setup::backend();

    let tomotroid = Tomotroid::new();
    let config_model = tomotroid.config_model.clone();
    let set_handle = tomotroid.window.as_weak();
    let filt_mod = Rc::new(ModelRc::from(tomotroid.config_model.clone()).filter(|cf| cf.enabled));
    tomotroid
        .window
        .global::<ConfigCallbacks>()
        .set_configs(ModelRc::from(filt_mod.clone()));

    tomotroid
        .window
        .global::<Settings>()
        .on_bool_changed(move |set_type, val| {
            settings::bool_changed(&set_handle, &config_model, set_type, val);
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

    let vol_sink = tomotroid.audio_sink.clone();
    let set_int_handle = tomotroid.window.as_weak();
    tomotroid
        .window
        .global::<Settings>()
        .on_int_changed(move |set_type, val| {
            settings::int_changed(&set_int_handle, &vol_sink, set_type, val)
        });

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
        min_handle.window().set_minimized(true);
    });

    let move_handle = tomotroid.window.as_weak();
    tomotroid.window.on_move_window(move || {
        let move_handle = move_handle.upgrade().unwrap();
        i_slint_backend_winit::WinitWindowAccessor::with_winit_window(
            move_handle.window(),
            i_slint_backend_winit::winit::window::Window::drag_window,
        );
    });

    let tray_handle = tomotroid.window.as_weak();
    let _tray_rec_thread = std::thread::spawn(move || loop {
        match tray_rx.recv() {
            Ok(TrayMsg::MinRes) => {
                let tray_handle_copy = tray_handle.clone();
                slint::invoke_from_event_loop(move || {
                    let main = tray_handle_copy.upgrade().unwrap();
                    main.window().set_minimized(false);
                    i_slint_backend_winit::WinitWindowAccessor::with_winit_window(
                        main.window(),
                        |win| {
                            //win.set_minimized(!win.is_minimized().unwrap());
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

    //This tick count is a quick hacky way to keep track of how many times
    //the timer has been called...it's really not ideal, but I need to know
    //so I can call the tick sound every 20 calls of the timer (ie 1s)
    //If slint ever adds the rounded line caps, and I update the progress circle
    //to be a Slint component instead of an SVG I can use animation
    //to make this circle animate smoothly but only have the time trigger
    //once per second. Until then I need to track how many times the timer
    //is called. I will use this quick hacky method for now to get the basic
    //sound logic in and working. There is probably a better way to handle this even
    //before Slint adds support for line caps in paths, but I'll come back to it.
    let mut tick_count = 0u32;
    let tick_sink = tomotroid.audio_sink.clone();
    let timer_handle = tomotroid.window.as_weak();
    tomotroid.window.on_action_timer(move |action| {
        //Notification::new().summary("Performing an Action").show().unwrap();
        let tmrstrt_handle = timer_handle.clone();
        let timer_handle = timer_handle.upgrade().unwrap();
        let tick_sink = tick_sink.clone();
        match action {
            TimerAction::Start => {
                timer_handle.set_running(true);
                timer.start(
                    TimerMode::Repeated,
                    std::time::Duration::from_millis(1000),
                    move || {
                        let tmrstrt_handle = tmrstrt_handle.unwrap();

                        if tick_count >= 20 {
                            tick_count = 0;
                            let is_work_timer =
                                tmrstrt_handle.get_active_timer() == ActiveTimer::Focus;
                            if tmrstrt_handle.global::<Settings>().get_tick_sounds()
                                && is_work_timer
                            {
                                let source = Decoder::new(Cursor::new(TICK)).unwrap();
                                tick_sink.append(source);
                            } else if tmrstrt_handle
                                .global::<Settings>()
                                .get_tick_sounds_during_break()
                                && !is_work_timer
                            {
                                let source = Decoder::new(Cursor::new(TICK)).unwrap();
                                tick_sink.append(source);
                            }
                        } else {
                            tick_count += 1;
                        }

                        tmrstrt_handle.invoke_tick(1000);
                    },
                );
            }
            TimerAction::Stop => {
                timer_handle.set_running(false);
                timer.stop();
            }
            TimerAction::Reset => {
                timer.stop();
                timer_handle.set_running(false);
                timer_handle.set_remaining_time(timer_handle.get_target_time());
            }
            TimerAction::Skip => {
                //timer_handle.set_remaining_time(0);
                timer_handle.invoke_change_timer();
            }
        }
    });

    let tmr_change_sink = tomotroid.audio_sink.clone();
    let chg_tmr_handle = tomotroid.window.as_weak();
    tomotroid.window.on_change_timer(move || {
        let chg_tmr_handle = chg_tmr_handle.upgrade().unwrap();
        match chg_tmr_handle.get_active_timer() {
            ActiveTimer::Focus => {
                if !chg_tmr_handle
                    .global::<Settings>()
                    .get_auto_start_break_timer()
                {
                    chg_tmr_handle.invoke_action_timer(TimerAction::Stop);
                }
                let body_str = if chg_tmr_handle.get_active_round()
                    == chg_tmr_handle.get_tmr_config().rounds
                {
                    let source = Decoder::new(Cursor::new(ALERT_LONG_BREAK)).unwrap();
                    tmr_change_sink.append(source);
                    let lgbrk_time = chg_tmr_handle.get_tmr_config().lgbrk_time;

                    chg_tmr_handle.set_active_round(1);
                    chg_tmr_handle.set_active_timer(ActiveTimer::LongBreak);

                    chg_tmr_handle.set_target_time(lgbrk_time);
                    chg_tmr_handle.set_remaining_time(lgbrk_time);
                    format!("Begin a {} minute long break.", lgbrk_time / 60000)
                } else {
                    let source = Decoder::new(Cursor::new(ALERT_SHORT_BREAK)).unwrap();
                    tmr_change_sink.append(source);
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
                    .show()
                    .unwrap();
            }
            brk_type => {
                if !chg_tmr_handle
                    .global::<Settings>()
                    .get_auto_start_work_timer()
                {
                    chg_tmr_handle.invoke_action_timer(TimerAction::Stop);
                }

                let focus_time = chg_tmr_handle.get_tmr_config().focus_time;
                let source = Decoder::new(Cursor::new(ALERT_WORK)).unwrap();
                tmr_change_sink.append(source);
                chg_tmr_handle.set_active_round(if brk_type == ActiveTimer::ShortBreak {
                    chg_tmr_handle.get_active_round() + 1
                } else {
                    1
                });
                chg_tmr_handle.set_active_timer(ActiveTimer::Focus);
                chg_tmr_handle.set_target_time(focus_time);
                chg_tmr_handle.set_remaining_time(focus_time);
                Notification::new()
                    //.appname("Tomotroid")
                    //.icon("../assets/logo.png")
                    .summary("Break Finished")
                    .body(&format!(
                        "Begin focusing for {} minutes.",
                        focus_time / 60000
                    ))
                    .show()
                    .unwrap();
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
            if (!event.modifiers.control
                && !event.modifiers.alt
                && !event.modifiers.shift
                && !event.modifiers.meta)
                || event.text == SharedString::from(Key::Tab)
            {
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
                        ghk_handle
                            .global::<Settings>()
                            .set_tt_ghk(SharedString::new());
                        ghk_handle.global::<Settings>().set_tt_ghk(pre);
                    }
                    GHKShortcuts::ResetTimer => {
                        let pre = ghk_handle.global::<Settings>().get_rst_ghk();
                        ghk_handle
                            .global::<Settings>()
                            .set_rst_ghk(SharedString::new());
                        ghk_handle.global::<Settings>().set_rst_ghk(pre);
                    }
                    GHKShortcuts::SkipRound => {
                        let pre = ghk_handle.global::<Settings>().get_skp_ghk();
                        ghk_handle
                            .global::<Settings>()
                            .set_skp_ghk(SharedString::new());
                        ghk_handle.global::<Settings>().set_skp_ghk(pre);
                    }
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

                if let Some(non_pr_char) = get_non_print_key_txt(&event.text) {
                    text.push_str(non_pr_char);
                } else {
                    text.push_str(&event.text.to_uppercase());
                }

                match ghk {
                    GHKShortcuts::ToggleTimer => {
                        ghk_handle.global::<Settings>().set_tt_ghk(text.into());
                    }
                    GHKShortcuts::ResetTimer => {
                        ghk_handle.global::<Settings>().set_rst_ghk(text.into());
                    }
                    GHKShortcuts::SkipRound => {
                        ghk_handle.global::<Settings>().set_skp_ghk(text.into());
                    }
                }
                ghk_handle.save_settings();
            }
        });

    tomotroid.run()?;
    Ok(())
}
