fn main() {
    #[cfg(windows)]
    windres::Build::new().compile("tomotroid.rc").unwrap();

    slint_build::compile("ui/appwindow.slint").expect("ui/appwindow.slint not found, aborting");
}
