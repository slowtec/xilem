// Copyright 2023 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

// TODO: add docs

use std::rc::Rc;

use leaflet::{LatLng, Map, MapOptions, Marker, TileLayer};

use web_sys::HtmlDivElement;
use xilem_web::{
    concurrent::TaskProxy, core::one_of::Either, document_body, elements::html,
    input_event_target_value, interfaces::Element, modifiers::style, App, DomView,
};

#[derive(Default)]
struct AppState {
    show_map: bool,
    zoom: Option<f64>,
    map: Option<Map>,
    markers: Vec<LatLng>,
}

#[derive(Debug)]
enum MapMessage {
    MapInitialized(Map),
    ZoomChanged(f64),
    MouseClicked(LatLng),
    TheMapIsGone,
}

fn map_event_handler(state: &mut AppState, message: MapMessage) {
    log::debug!("handle message {message:?}");
    match message {
        MapMessage::MapInitialized(map) => {
            map.set_view(&LatLng::new(63.5, 10.5), 5.0);
            add_tile_layer(&map);
            state.map = Some(map);
        }
        MapMessage::ZoomChanged(zoom) => {
            state.zoom = Some(zoom);
        }
        MapMessage::MouseClicked(lat_lng) => {
            state.markers.push(lat_lng);
        }
        // FIXME:
        // This message will never be received,
        // because the view is already destroyed
        // so that the message function of
        // `BeforeTeardownWithProxy` is no longer called.
        MapMessage::TheMapIsGone => {
            state.map = None;
        }
    }
}

fn after_map_build(el: &HtmlDivElement, proxy: TaskProxy) {
    let options = MapOptions::default();
    let map = Map::new_with_element(el, &options);
    let proxy = Rc::new(proxy);
    proxy.send_message(MapMessage::MapInitialized(map.clone()));
    map.on_zoom({
        let map = map.clone();
        let proxy = Rc::clone(&proxy);
        Box::new(move |_| {
            let zoom = map.get_zoom();
            proxy.send_message(MapMessage::ZoomChanged(zoom));
        })
    });
    map.on_mouse_click(Box::new(move |ev| {
        let lat_lng = ev.lat_lng();
        proxy.send_message(MapMessage::MouseClicked(lat_lng));
    }));
}

fn header(state: &mut AppState) -> impl Element<AppState> {
    html::div((
        if state.show_map {
            Either::A(
                html::button("hide map").on_click(|state: &mut AppState, _| {
                    state.show_map = false;
                }),
            )
        } else {
            Either::B(
                html::button("show map").on_click(|state: &mut AppState, _| {
                    state.show_map = true;
                }),
            )
        }
        .style(style("margin", "0.5rem")),
        html::label((
            "Zoom",
            html::input(())
                .on_change(on_zoom_input_change)
                .attr("value", state.zoom)
                .style(style("margin", "0.5rem")),
        ))
        .style(style("margin", "0.5rem")),
    ))
    .style([
        style("background", "#8888"),
        style("grid-column-start", "1"),
        style("grid-column-end", "-1"),
    ])
}

fn on_zoom_input_change(state: &mut AppState, ev: web_sys::Event) {
    let Some(value) = input_event_target_value(&ev) else {
        return;
    };
    let Ok(number) = value.parse::<f64>() else {
        log::warn!("Invalid zoom value");
        return;
    };
    state.zoom = Some(number);
}

fn update_map(state: &AppState) {
    let Some(map) = &state.map else {
        return;
    };
    if let Some(zoom) = state.zoom {
        // FIXME:
        // How can we avoid to call
        // if the zoom did not change?
        map.set_zoom(zoom);
    }

    // FIXME:
    // How can we avoid to call
    // if the markers did not change?
    for lat_lng in &state.markers {
        let marker = Marker::new(lat_lng);
        marker.add_to(map);
    }
}

fn map(state: &mut AppState) -> impl Element<AppState> {
    update_map(state);
    html::div(())
        .after_build_with_proxy(after_map_build, map_event_handler)
        .before_teardown_with_proxy(
            |_, proxy| {
                proxy.send_message(MapMessage::TheMapIsGone);
            },
            map_event_handler,
        )
        .style([
            style("width", "100%"),
            style("height", "100%"),
            style("grid-row-start", "2"),
        ])
}

fn app_logic(state: &mut AppState) -> impl Element<AppState> {
    html::div((
        header(state),
        if state.show_map {
            Some(map(state))
        } else {
            None
        },
    ))
    .style([
        style("width", "100%"),
        style("height", "100%"),
        style("display", "grid"),
        style("grid-template-rows", "3rem 1fr"),
    ])
}

fn add_tile_layer(map: &Map) {
    TileLayer::new("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png").add_to(map);
}

pub fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();
    App::new(document_body(), AppState::default(), app_logic).run();
}
