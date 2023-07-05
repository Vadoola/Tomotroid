#[cfg(unix)]
use std::io::Cursor;

use anyhow::Result;
use std::sync::mpsc;
use tray_item::{IconSource, TrayItem};

slint::include_modules!();

pub const LOGO_BYTES: &str = include_str!("../assets/logo.svg");

enum TrayMsg {
    MinRes,
    Quit,
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
        TrayItem::new(
            "Tomotroid\nClick to Restore",
            logo_icon
        ).unwrap()
    };

    #[cfg(windows)]
    let mut tray = TrayItem::new(
        "Tomotroid\nClick to Restore",
        IconSource::Resource("logo-icon"),
    ).unwrap();

    let (tray_tx, tray_rx) = mpsc::sync_channel(1);

    let minres_tx = tray_tx.clone();
    tray.add_menu_item("Minimize / Restore", move || {
        minres_tx.send(TrayMsg::MinRes).unwrap();
    }).unwrap();

    let quit_tx = tray_tx;//.clone();
    tray.add_menu_item("Quit", move || {
        quit_tx.send(TrayMsg::Quit).unwrap();
    }).unwrap();

    slint::platform::set_platform(Box::new(i_slint_backend_winit::Backend::new())).unwrap();

    let main = Main::new()?;

    let close_handle = main.as_weak();
    main.on_close_window(move ||{
        close_handle.upgrade().unwrap().hide().unwrap();

        //After I get the system tray working I'm going to want to hide the window instead of actually close it
        //i_slint_backend_winit::WinitWindowAccessor::with_winit_window(min_handle.window(), |win| win.set_visible(false));
    });

    

    let min_handle = main.as_weak();
    main.on_minimize_window(move ||{
        let min_handle = min_handle.upgrade().unwrap();
        i_slint_backend_winit::WinitWindowAccessor::with_winit_window(min_handle.window(), |win| {
            win.set_minimized(true);
        });
    });

    let move_handle = main.as_weak();
    main.on_move_window(move ||{
        let move_handle = move_handle.upgrade().unwrap();
        i_slint_backend_winit::WinitWindowAccessor::with_winit_window(move_handle.window(), |win| win.drag_window());
    });

    let tray_handle = main.as_weak();
    let _tray_rec_thread = std::thread::spawn(move || {
        loop {
            match tray_rx.recv() {
                Ok(TrayMsg::MinRes) => {
                    let tray_handle_copy = tray_handle.clone();
                    slint::invoke_from_event_loop(move || {
                        let main = tray_handle_copy.upgrade().unwrap();
                        i_slint_backend_winit::WinitWindowAccessor::with_winit_window(main.window(),
                            |win| {
                                win.set_minimized(!win.is_minimized().unwrap());
                                win.focus_window();
                            }
                        );
                    }).unwrap();
                }
                Ok(TrayMsg::Quit) => {
                    let tray_handle_copy = tray_handle.clone();
                    slint::invoke_from_event_loop(move || {
                        tray_handle_copy.upgrade().unwrap().hide().unwrap();
                    }).unwrap();
                }
                _ => {}
            }
        }
    });

    main.global::<HLClick>().on_hl_clicked(|url| {
        open::that(url.as_str()).unwrap();
    });

    let thm_handle = main.as_weak();
    main.global::<ThemeChanged>().on_theme_changed(move |theme| {
        println!("New Theme: {theme}");
        let thm_handle = thm_handle.upgrade().unwrap();
        //just testing that this works...later will need to replace the logo colors with ones from the theme.
        thm_handle.set_logo(slint::Image::load_from_svg_data(LOGO_BYTES.replace("fill:#f6f2eb", "file:#000000").as_bytes()).unwrap());
    });

    main.run()?;
    Ok(())
}
