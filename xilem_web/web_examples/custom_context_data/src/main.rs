// Copyright 2024 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0
use std::{marker::PhantomData, rc::Rc};

use xilem_web::{
    core::{MessageResult, Mut, NoElement, View, ViewId, ViewMarker},
    document_body,
    elements::html,
    interfaces::HtmlElement,
    App, DomFragment, DynMessage, ViewCtx,
};

#[derive(Default)]
struct AppState;

struct CustomParentView<State, Action, V, C> {
    view: V,
    child_view: C,
    phantom: PhantomData<dyn Fn() -> (State, Action)>,
}

#[allow(unused)]
struct CustomParentViewState<ViewState, ChildState, CustomData> {
    view_state: ViewState,
    child_state: ChildState,
    custom_data: Rc<CustomData>,
}

#[allow(unused)]
#[derive(Debug)]
struct CustomParentData {
    foo: u64,
}

struct CustomChildView<State, Action> {
    phantom: PhantomData<dyn Fn() -> (State, Action)>,
}

impl<State, Action, V, C> ViewMarker for CustomParentView<State, Action, V, C> {}
impl<State, Action> ViewMarker for CustomChildView<State, Action> {}

impl<State, Action, V, C> View<State, Action, ViewCtx, DynMessage>
    for CustomParentView<State, Action, V, C>
where
    State: 'static,
    Action: 'static,
    V: View<State, Action, ViewCtx, DynMessage>,
    C: View<State, Action, ViewCtx, DynMessage>,
{
    type Element = V::Element;
    type ViewState = CustomParentViewState<V::ViewState, C::ViewState, CustomParentData>;
    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        let (element, view_state) = self.view.build(ctx);
        let custom_data = Rc::new(CustomParentData { foo: 42 });
        log::debug!("Run ctx.with_data");
        let (_, child_state) = ctx.with_data(Rc::clone(&custom_data), |ctx| {
            log::debug!("call child_view.build(ctx)");
            self.child_view.build(ctx)
        });
        let view_state = CustomParentViewState {
            view_state,
            child_state,
            custom_data,
        };
        (element, view_state)
    }
    fn rebuild(
        &self,
        _: &Self,
        _: &mut Self::ViewState,
        _: &mut ViewCtx,
        _: Mut<'_, Self::Element>,
    ) {
        // TODO
    }
    fn teardown(&self, _: &mut Self::ViewState, _: &mut ViewCtx, _: Mut<'_, Self::Element>) {
        // TODO
    }
    fn message(
        &self,
        _: &mut Self::ViewState,
        _: &[ViewId],
        _: DynMessage,
        _: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        MessageResult::Nop
    }
}

impl<State, Action> View<State, Action, ViewCtx, DynMessage> for CustomChildView<State, Action>
where
    State: 'static,
    Action: 'static,
{
    type Element = NoElement;
    type ViewState = ();
    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        log::debug!("build child view");
        let data = ctx.custom_data::<Rc<CustomParentData>>().unwrap();
        log::debug!("{data:?}");
        (NoElement, ())
    }
    fn rebuild(
        &self,
        _: &Self,
        _: &mut Self::ViewState,
        _: &mut ViewCtx,
        _: Mut<'_, Self::Element>,
    ) {
        // TODO
    }
    fn teardown(&self, _: &mut Self::ViewState, _: &mut ViewCtx, _: Mut<'_, Self::Element>) {
        todo!()
    }
    fn message(
        &self,
        _: &mut Self::ViewState,
        _: &[ViewId],
        _: DynMessage,
        _: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        MessageResult::Nop
    }
}

fn custom_parent_view<State, Action, Child>(
    child_view: Child,
) -> CustomParentView<State, Action, impl HtmlElement<State, Action>, Child>
where
    State: 'static,
    Action: 'static,
{
    let view = html::div(());
    CustomParentView {
        view,
        child_view,
        phantom: PhantomData,
    }
}

fn custom_child_view<State, Action>() -> CustomChildView<State, Action>
where
    State: 'static,
    Action: 'static,
{
    CustomChildView {
        phantom: PhantomData,
    }
}

fn app_logic(_state: &mut AppState) -> impl DomFragment<AppState> {
    html::div(custom_parent_view(custom_child_view()))
}

pub fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();
    log::debug!("Run app");
    App::new(document_body(), AppState::default(), app_logic).run();
}
