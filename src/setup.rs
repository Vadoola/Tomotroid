#[cfg(unix)]
use anyhow::{bail, Result};
use log::{error, info, warn, LevelFilter};
use std::{
    io::Cursor,
    sync::mpsc::{self, Receiver},
};
use systemd_journal_logger::{connected_to_journal, JournalLog};
use tray_item::{IconSource, TrayItem};

pub enum TrayMsg {
    MinRes,
    Quit,
}

pub fn logging() {
    #[cfg(unix)] //is this true for Mac? how would I test for non-mac *nix?
    let fallback_needed = if connected_to_journal() {
        //if systemd journal is found use it
        JournalLog::new()
            .unwrap()
            .with_extra_fields(vec![("VERSION", env!("CARGO_PKG_VERSION"))])
            .with_syslog_identifier("Tomotroid".to_string())
            .install()
            .unwrap();
        false
    } else {
        //check for syslog, and then fall back to text file
        //My dev machine here DOES have a systemd journal....is there an easy way to disable that
        //and test syslog? Never tried, or do I need to spin up a different VM?
        true
    };

    /*#[cfg(windows)]
    let fallback_needed = if ?? {
        //try to setup windows event logging
    }*/

    /*#[cfg(mac)]...not sure the right command here yet, need to look
    let fallback_needed = true
    */

    if fallback_needed {
        //setup text file logging if:
        //    * nix systems: systemd journal or syslog fails
        //    * Windows: Event log fails
        //    * Mac: always currentlys
    }

    //default to warn until we can read from settings or ENV
    log::set_max_level(LevelFilter::Warn);
}

//TODO: I'm not seeing an obvious way to mimic the Pomotroid behavoir
//where it just minimizes or restores by clicking the tray icon
//because I don't see any way to capture when the tray icon is clicked
//I'll need to dig into this more. For now I'll just add some menu items
//to get some basic functionality and test minimzing to the tray etc
pub fn tray() -> Result<Receiver<TrayMsg>> {
    let mut tray = create_tray()?;

    let (tray_tx, tray_rx) = mpsc::sync_channel(1);

    let minres_tx = tray_tx.clone();
    tray.add_menu_item("Minimize / Restore", move || {
        minres_tx.send(TrayMsg::MinRes).unwrap();
    })?;

    let quit_tx = tray_tx;
    tray.add_menu_item("Quit", move || {
        quit_tx.send(TrayMsg::Quit).unwrap();
    })?;

    Ok(tray_rx)
}

#[cfg(unix)]
fn create_tray() -> Result<TrayItem> {
    let logo_cursor = Cursor::new(include_bytes!("../assets/icons/logo.png"));
    let logo_decoder = png::Decoder::new(logo_cursor);
    let mut logo_reader = logo_decoder.read_info().unwrap();
    let mut logo_buff = vec![0; logo_reader.output_buffer_size()];

    logo_reader.next_frame(&mut logo_buff).map_err(|de| {
        error!("Unable to decode tray icon file: {de}");
        de
    })?;

    let logo_icon = IconSource::Data {
        data: logo_buff,
        height: 32,
        width: 32,
    };

    TrayItem::new("Tomotroid\nClick to Restore", logo_icon).map_err(|e| {
        error!("Error generating the System Tray");
        e.into()
    })
}

#[cfg(windows)]
fn create_tray() -> Result<TrayItem> {
    TrayItem::new(
        "Tomotroid\nClick to Restore",
        IconSource::Resource("logo-icon"),
    )
}
