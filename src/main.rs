use anyhow::Result;
use slint::LogicalPosition;

slint::include_modules!();

fn main() -> Result<()> {
    let main = Main::new()?;

    let close_handle = main.as_weak();
    main.on_close_window(move ||{
        close_handle.upgrade().unwrap().hide().unwrap();
    });

    let move_handle = main.as_weak();
    main.on_move_window(move |offset_x, offset_y|{
        let main = move_handle.upgrade().unwrap();
        let logical_pos = main.window().position().to_logical(main.window().scale_factor());
        main.window().set_position(LogicalPosition::new(logical_pos.x + offset_x, logical_pos.y + offset_y));
    });

    main.global::<HLClick>().on_hl_clicked(|url| {
        open::that(url.as_str()).unwrap();
    });

    main.run()?;
    Ok(())
}
