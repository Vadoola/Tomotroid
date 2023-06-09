use anyhow::Result;
use slint::LogicalPosition;

slint::include_modules!();

fn main() -> Result<()> {
    let main = Main::new()?;

    let handle1 = main.as_weak();
    main.on_close_window(move ||{
        handle1.upgrade().unwrap().hide().unwrap();
    });

    let handle2 = main.as_weak();
    main.on_move_window(move |offset_x, offset_y|{
        let main = handle2.upgrade().unwrap();
        let logical_pos = main.window().position().to_logical(main.window().scale_factor());
        main.window().set_position(LogicalPosition::new(logical_pos.x + offset_x, logical_pos.y + offset_y));
    });

    main.run()?;
    Ok(())
}
