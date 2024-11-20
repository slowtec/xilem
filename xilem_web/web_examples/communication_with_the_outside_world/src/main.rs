// Copyright 2024 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

//! This example shows how a xilem web application
//! can communicate with an external system.

#![expect(clippy::shadow_unrelated, reason = "Idiomatic for Xilem users")]

use std::{cell::RefCell, rc::Rc};

use futures::{channel::mpsc, select, FutureExt, SinkExt, StreamExt};
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use xilem_web::{
    concurrent::{task_raw, ShutdownSignal, TaskProxy},
    core::fork,
    document_body,
    elements::html,
    interfaces::Element,
    App,
};

struct AppState {
    to_outside_tx: mpsc::Sender<FromInside>,
    from_outside_rx: Rc<RefCell<mpsc::Receiver<FromOutside>>>,
    ping_sent: usize,
    pong_sent: usize,
    ping_received: usize,
    pong_received: usize,
}

#[derive(Debug)]
enum FromOutside {
    Ping,
    Pong,
}

#[derive(Debug)]
enum FromInside {
    Ping,
    Pong,
}

#[derive(Debug)]
enum TaskMessage {
    ReceivedPing,
    ReceivedPong,
}

fn app_logic(state: &mut AppState) -> impl Element<AppState> {
    let rx = Rc::clone(&state.from_outside_rx);
    let rx_task = task_raw(
        move |proxy: TaskProxy, shutdown_signal: ShutdownSignal| {
            let rx = Rc::clone(&rx);
            async move { receive_from_outside_task(proxy, shutdown_signal, rx).await }
        },
        |state: &mut AppState, message: TaskMessage| match message {
            TaskMessage::ReceivedPing => {
                state.ping_received += 1;
                match state.to_outside_tx.try_send(FromInside::Pong) {
                    Ok(_) => {
                        state.pong_sent += 1;
                    }
                    Err(err) => {
                        log::warn!("{err}");
                    }
                }
            }
            TaskMessage::ReceivedPong => {
                state.pong_received += 1;
            }
        },
    );

    html::div((
        format!("Ping sent: {}", state.ping_sent),
        html::br(()),
        format!("Pong received: {}", state.pong_received),
        html::br(()),
        format!("Ping received: {}", state.ping_received),
        html::br(()),
        format!("Pong sent: {}", state.pong_sent),
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
        fork(html::div(()), rx_task),
    ))
}

async fn receive_from_outside_task(
    proxy: TaskProxy,
    shutdown_signal: ShutdownSignal,
    from_outside_rx: Rc<RefCell<mpsc::Receiver<FromOutside>>>,
) {
    log::info!("Start receive from outside task");
    let mut abort = shutdown_signal.into_future().fuse();

    loop {
        let mut from_outside_rx_borrowed = from_outside_rx.borrow_mut();
        let mut message_fut = from_outside_rx_borrowed.next().fuse();

        select! {
            msg = message_fut => {
                match msg {
                  Some(FromOutside::Ping) => {
                      proxy.send_message(TaskMessage::ReceivedPing);
                  }
                  Some(FromOutside::Pong) => {
                      proxy.send_message(TaskMessage::ReceivedPong);
                  }
                  None => {
                     continue;
                  }
                }

            }
            _ = abort => {
                break;
            }
        }
    }
    log::info!("Receive from outside task terminated");
}

async fn external_event_loop(
    mut to_inside_tx: mpsc::Sender<FromOutside>,
    from_inside_rx: mpsc::Receiver<FromInside>,
) {
    log::info!("Start external event loop.");
    let mut from_inside_rx = from_inside_rx.fuse();
    let mut ping_received = 0;
    let mut pong_received = 0;

    loop {
        let mut timeout = TimeoutFuture::new(1_000).fuse();
        select! {
            _  = timeout => {
                if let Err(err) = to_inside_tx.send(FromOutside::Ping).await {
                    log::warn!("Unable to send ping messsage from the outside: {err}");
                    break;
                };
            }
            msg = from_inside_rx.select_next_some() => {
                match msg {
                    FromInside::Ping => {
                        ping_received += 1;
                        log::info!("Received {ping_received} ping messages from the inside");
                        if let Err(err) = to_inside_tx.send(FromOutside::Pong).await {
                            log::warn!("Unable to send ping messsage from the outside: {err}");
                            break;
                        };
                    }
                    FromInside::Pong => {
                        pong_received += 1;
                        log::info!("Received {pong_received} pong messages from the inside");
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

    let (to_inside_tx, from_outside_rx) = mpsc::channel::<FromOutside>(CHANNEL_SIZE);
    let (to_outside_tx, from_inside_rx) = mpsc::channel::<FromInside>(CHANNEL_SIZE);

    let app_state = AppState {
        to_outside_tx,
        from_outside_rx: Rc::new(RefCell::new(from_outside_rx)),
        ping_sent: 0,
        pong_sent: 0,
        ping_received: 0,
        pong_received: 0,
    };
    App::new(document_body(), app_state, app_logic).run();

    spawn_local(external_event_loop(to_inside_tx, from_inside_rx));
}
