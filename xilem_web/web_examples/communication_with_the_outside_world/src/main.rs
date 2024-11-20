// Copyright 2024 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

//! This example shows how a xilem web application
//! can communicate with an external system.

#![expect(clippy::shadow_unrelated, reason = "Idiomatic for Xilem users")]

use futures::{channel::mpsc, select, FutureExt, StreamExt};
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use xilem_web::{document_body, elements::html, interfaces::Element, App, DomFragment};

struct AppState {
    to_outside_tx: mpsc::Sender<FromInside>,
    ping_sent: usize,
    ping_received: usize,
    pong_received: usize,
}

#[derive(Debug)]
enum FromInside {
    Ping,
    Stop,
}

fn app_logic(state: &mut AppState) -> impl Element<AppState> {
    html::div((
        format!("Ping sent: {}", state.ping_sent),
        html::br(()),
        format!("Pong received: {}", state.pong_received),
        html::br(()),
        format!("Ping received: {}", state.ping_received),
        html::br(()),
        html::button("send ping").on_click(|state: &mut AppState, _| {
            match state.to_outside_tx.try_send(FromInside::Ping) {
                Ok(_) => {
                    state.ping_sent += 1;
                }
                Err(err) => {
                    log::warn!("{err}");
                }
            }
        }),
        html::br(()),
        html::button("stop external event loop").on_click(|state: &mut AppState, _| {
            if let Err(err) = state.to_outside_tx.try_send(FromInside::Stop) {
                log::warn!("{err}");
            }
        }),
    ))
}

async fn new_external_event_loop<Fragment, InitFragment>(
    app: App<AppState, Fragment, InitFragment>,
    from_inside_rx: mpsc::Receiver<FromInside>,
) where
    Fragment: DomFragment<AppState> + 'static,
    InitFragment: FnMut(&mut AppState) -> Fragment + 'static,
{
    log::info!("Start external event loop.");
    let mut from_inside_rx = from_inside_rx.fuse();
    let mut ping_received = 0;

    loop {
        let mut timeout = TimeoutFuture::new(1_000).fuse();
        select! {
            _  = timeout => {
                app.update(|state: &mut AppState| {
                    state.ping_received += 1;
                });
            }
            msg = from_inside_rx.select_next_some() => {
                match msg {
                    FromInside::Ping => {
                        ping_received += 1;
                        log::info!("Received {ping_received} ping messages from the inside");
                        app.update(|state: &mut AppState| {
                            state.pong_received += 1;
                        });
                    }
                    FromInside::Stop => {
                        log::debug!("Exit loop");
                        break;
                    }
                }
            }
        }
    }
    log::info!("External event loop terminated.");
}

const CHANNEL_SIZE: usize = 100;

fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();
    log::info!("Start web application");

    let (to_outside_tx, from_inside_rx) = mpsc::channel::<FromInside>(CHANNEL_SIZE);

    let app_state = AppState {
        to_outside_tx,
        ping_sent: 0,
        ping_received: 0,
        pong_received: 0,
    };
    let app = App::new(document_body(), app_state, app_logic);

    spawn_local(new_external_event_loop(app.clone(), from_inside_rx));

    app.run();
}
