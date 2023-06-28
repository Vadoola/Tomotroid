use anyhow::Result;
use slint::LogicalPosition;

slint::include_modules!();

fn main() -> Result<()> {
    slint::platform::set_platform(Box::new(i_slint_backend_winit::Backend::new())).unwrap();

    let main = Main::new()?;

    let close_handle = main.as_weak();
    main.on_close_window(move || {
        close_handle.upgrade().unwrap().hide().unwrap();

        //After I get the system tray working I'm going to want to hide the window instead of actually close it
        //i_slint_backend_winit::WinitWindowAccessor::with_winit_window(min_handle.window(), |win| win.set_visible(false));
    });

    let min_handle = main.as_weak();
    main.on_minimize_window(move || {
        let min_handle = min_handle.upgrade().unwrap();
        i_slint_backend_winit::WinitWindowAccessor::with_winit_window(min_handle.window(), |win| {
            win.set_minimized(true)
        });
    });

    let move_handle = main.as_weak();
    main.on_move_window(move |offset_x, offset_y| {
        let move_handle = move_handle.upgrade().unwrap();
        let logical_pos = move_handle
            .window()
            .position()
            .to_logical(move_handle.window().scale_factor());
        move_handle.window().set_position(LogicalPosition::new(
            logical_pos.x + offset_x,
            logical_pos.y + offset_y,
        ));
    });

    main.global::<HLClick>().on_hl_clicked(|url| {
        open::that(url.as_str()).unwrap();
    });

    main.run()?;
    Ok(())
}
