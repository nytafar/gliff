//! The away panel: `gliff --away-panel SOCKET PEER`, started by a
//! `gliff-server --takeover` session on the screen it took over. It is what
//! the desk sees after unlocking: the screen is in use remotely, with
//! buttons to take it back (which ends the remote session) or to turn it off
//! again. It quits when the session ends and closes the socket.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::mpsc::{channel, TryRecvError};
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;
use gtk4 as gtk;
use libadwaita as adw;

/// The Wayland app ID, for window rules.
const APP_ID: &str = "gliff-away";

pub fn run(socket: &str, peer: &str) -> glib::ExitCode {
    let stream = match UnixStream::connect(socket) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(socket, error = %e, "cannot reach the gliff session");
            return glib::ExitCode::FAILURE;
        }
    };
    let (closed_tx, closed_rx) = channel::<()>();
    let mut reader = match stream.try_clone() {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, "socket");
            return glib::ExitCode::FAILURE;
        }
    };
    std::thread::spawn(move || {
        let mut sink = [0u8; 64];
        while reader.read(&mut sink).is_ok_and(|n| n > 0) {}
        let _ = closed_tx.send(());
    });

    let stream = std::rc::Rc::new(stream);
    glib::set_prgname(Some(APP_ID));
    let app = adw::Application::builder().build();
    let peer = peer.to_owned();
    let closed_rx = std::cell::RefCell::new(Some(closed_rx));
    app.connect_activate(move |app| {
        let stream = std::rc::Rc::clone(&stream);
        let send = move |line: &str| {
            let _ = (&*stream).write_all(format!("{line}\n").as_bytes());
        };
        let window = build(app, &peer, send);
        window.present();
        let Some(rx) = closed_rx.borrow_mut().take() else {
            return;
        };
        let app = app.clone();
        glib::timeout_add_local(Duration::from_millis(200), move || match rx.try_recv() {
            Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
            _ => {
                app.quit();
                glib::ControlFlow::Break
            }
        });
    });
    let empty: Vec<String> = vec![];
    app.run_with_args(&empty)
}

fn build(
    app: &adw::Application,
    peer: &str,
    send: impl Fn(&str) + Clone + 'static,
) -> adw::ApplicationWindow {
    let page = adw::StatusPage::builder()
        .icon_name("network-wired-symbolic")
        .title("This screen is in use remotely")
        .description(format!(
            "Its workspaces are on a gliff session from {peer}. Taking the screen back ends that session."
        ))
        .build();
    let buttons = gtk::Box::new(gtk::Orientation::Vertical, 12);
    buttons.set_halign(gtk::Align::Center);
    let reclaim = gtk::Button::with_label("Take the screen back");
    reclaim.add_css_class("suggested-action");
    reclaim.add_css_class("pill");
    let off = gtk::Button::with_label("Turn the screen off");
    off.add_css_class("pill");
    {
        let send = send.clone();
        reclaim.connect_clicked(move |b| {
            b.set_sensitive(false);
            send("reclaim");
        });
    }
    off.connect_clicked(move |_| send("off"));
    buttons.append(&reclaim);
    buttons.append(&off);
    page.set_child(Some(&buttons));
    adw::ApplicationWindow::builder()
        .application(app)
        .title("gliff")
        .content(&page)
        .build()
}
