// Copyright 2023 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

use std::{cell::RefCell, rc::Rc};

use xilem_web::{document_body, elements::html, interfaces::Element, App};

#[derive(Default)]
struct AppState {
    raw_dom_el: Rc<RefCell<Option<web_sys::HtmlInputElement>>>,
}

fn app_logic(app_state: &mut AppState) -> impl Element<AppState> {
    let raw_dom_el = Rc::clone(&app_state.raw_dom_el);

    html::div((
        html::input(()).after_build(move |el| {
            log::debug!("Take node reference");
            *raw_dom_el.borrow_mut() = Some(el.node.clone());
        }),
        html::button("Focus the input").on_click(|app_state: &mut AppState, _| {
            if let Some(el) = &*app_state.raw_dom_el.borrow() {
                el.focus().unwrap();
            }
        }),
    ))
}

pub fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();
    App::new(document_body(), AppState::default(), app_logic).run();
}
