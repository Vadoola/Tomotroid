fn main() {
    windres::Build::new().compile("tomotroid.rc").unwrap();
    slint_build::compile("ui/appwindow.slint").unwrap();
}
