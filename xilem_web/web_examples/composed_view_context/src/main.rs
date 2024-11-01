// Copyright 2023 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

//! There are situations in which you need a different
//! view context with user-defined behavior and data,
//! e.g. if you want to create a wrapper for a JavaScript library.

use std::{any::TypeId, marker::PhantomData};

use wasm_bindgen_futures::spawn_local;
use web_sys::wasm_bindgen::UnwrapThrowExt;
use xilem_web::{
    core::{
        AppendVec, MessageResult, Mut, SuperElement, View, ViewElement, ViewId, ViewMarker,
        ViewPathTracker, ViewSequence,
    },
    document_body,
    elements::html,
    interfaces::HtmlElement,
    App, DomFragment, DynMessage, ViewCtx,
};

#[derive(Default)]
struct AppState;

// This data can be used across all user-defined views.
// For example, this could be a reference
// to a JavaScript object from an external library.
struct CustomContextData(u64);

// In order to have access to both the custom data
// and the original view context, we build a wrapper.
struct MyComposedViewCtx<'a> {
    view_ctx: &'a mut ViewCtx,
    custom_data: &'a CustomContextData,
}

impl<'a> MyComposedViewCtx<'a> {
    fn new(view_ctx: &'a mut ViewCtx, custom_data: &'a CustomContextData) -> Self {
        Self {
            view_ctx,
            custom_data,
        }
    }

    const fn data(&self) -> &CustomContextData {
        self.custom_data
    }

    // Specific functions may also be required,
    // which can be defined in this composed context.
    fn user_defined_behavior(&self) {
        log::debug!("Hello from the composed context!");
    }
}

// To meet the requirements of a view context,
// we need to implement the `ViewPathTracker` trait.
impl ViewPathTracker for MyComposedViewCtx<'_> {
    fn push_id(&mut self, id: ViewId) {
        self.view_ctx.push_id(id);
    }

    fn pop_id(&mut self) {
        self.view_ctx.pop_id();
    }

    fn view_path(&mut self) -> &[ViewId] {
        self.view_ctx.view_path()
    }
}

// Our custom elements may not be DOM elements,
// which is why we have to define them here.
struct MyCustomEl;

impl ViewElement for MyCustomEl {
    type Mut<'a> = &'a mut MyCustomEl;
}

// To be able to use a sequence of our elements,
// the `ViewSequence` trait is required.
// This in turn requires the implementation of `SuperElement`.
impl SuperElement<MyCustomEl, MyComposedViewCtx<'_>> for MyCustomEl {
    fn upcast(_: &mut MyComposedViewCtx<'_>, child: MyCustomEl) -> Self {
        child
    }

    fn with_downcast_val<R>(
        this: Mut<'_, Self>,
        f: impl FnOnce(Mut<'_, MyCustomEl>) -> R,
    ) -> (Self::Mut<'_>, R) {
        let r = f(this);
        (this, r)
    }
}

// Assuming we want to define a user-defined view with a special children view,
// the parent view gets the normal view context
// and the children get our user-defined `MyComposedViewCtx`

struct MyCustomParentView<V, Children, State, Action> {
    view: V,
    children: Children,
    phantom: PhantomData<dyn Fn() -> (State, Action)>,
}

// The parents view state also contains the state of the children.
struct MyCustomParentViewState<ViewState, ChildrenState> {
    view_state: ViewState,
    children_state: ChildrenState,
    // Here we keep the overarching context data
    // so that it is available for the user-defined context for the children.
    custom_data: CustomContextData,
}

struct MyCustomChildView<State, Action> {
    txt: &'static str,
    phantom: PhantomData<dyn Fn() -> (State, Action)>,
}

struct MyCustomChildViewState;

impl<V, Children, State, Action> ViewMarker for MyCustomParentView<V, Children, State, Action> {}

impl<State, Action> ViewMarker for MyCustomChildView<State, Action> {}

// Distinctive ID for better debugging
const CUSTOM_PARENT_VIEW_ID: ViewId = ViewId::new(1236068);

#[derive(Debug)]
enum CustomParentMessage {
    Baz,
}

impl<'a, V, State, Action, Children> View<State, Action, ViewCtx, DynMessage>
    for MyCustomParentView<V, Children, State, Action>
where
    State: 'static,
    Action: 'static,
    Children: ViewSequence<State, Action, MyComposedViewCtx<'a>, MyCustomEl, DynMessage>,
    V: View<State, Action, ViewCtx>,
{
    type Element = V::Element;

    type ViewState = MyCustomParentViewState<V::ViewState, Children::SeqState>;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        ctx.with_id(CUSTOM_PARENT_VIEW_ID, |ctx| {
            let (view_element, view_state) = self.view.build(ctx);

            let custom_data = CustomContextData(42);
            let mut child_elements = AppendVec::default();
            let mut custom_ctx = MyComposedViewCtx::new(ctx, &custom_data);
            let children_state = self
                .children
                .seq_build(&mut custom_ctx, &mut child_elements);

            let view_state = MyCustomParentViewState {
                view_state,
                children_state,
                custom_data,
            };

            (view_element, view_state)
        })
    }

    fn rebuild(&self, _: &Self, _: &mut Self::ViewState, _: &mut ViewCtx, _: Mut<Self::Element>) {
        // TODO
    }

    fn teardown(&self, _: &mut Self::ViewState, _: &mut ViewCtx, _: Mut<Self::Element>) {
        // TODO
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        path: &[ViewId],
        message: DynMessage,
        state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        let (first, _) = path.split_first().unwrap_throw();
        assert_eq!(*first, CUSTOM_PARENT_VIEW_ID);
        if message.as_any().type_id() == TypeId::of::<CustomParentMessage>() {
            let message = *message.downcast().unwrap();
            return match message {
                CustomParentMessage::Baz => MessageResult::RequestRebuild,
            };
        }
        let child_result =
            self.children
                .seq_message(&mut view_state.children_state, path, message, state);

        match child_result {
            _ => {
                // TODO
            }
        }

        MessageResult::Nop
    }
}

#[derive(Debug)]
enum CustomChildMessage {
    Foo(u64),
}

// Implementation of the child view.
impl<'a, State, Action> View<State, Action, MyComposedViewCtx<'a>, DynMessage>
    for MyCustomChildView<State, Action>
where
    State: 'static,
    Action: 'static,
{
    type Element = MyCustomEl;

    type ViewState = MyCustomChildViewState;

    fn build(&self, ctx: &mut MyComposedViewCtx<'a>) -> (Self::Element, Self::ViewState) {
        log::debug!("Build child {:?}", self.txt);

        // Here we can access the context data
        let x = ctx.data().0;

        // In order to send a message to itself
        // at this point in time,
        // we need to defer the it by using
        // an async task.
        let thunk = ctx.view_ctx.message_thunk();
        spawn_local(async move {
            thunk.push_message(CustomChildMessage::Foo(x));
        });

        (MyCustomEl, MyCustomChildViewState)
    }

    fn rebuild(
        &self,
        _: &Self,
        _: &mut Self::ViewState,
        _: &mut MyComposedViewCtx<'a>,
        _: Mut<Self::Element>,
    ) {
        log::debug!("Rebuild child {:?}", self.txt);
        // TODO
    }

    fn teardown(
        &self,
        _: &mut Self::ViewState,
        _: &mut MyComposedViewCtx<'a>,
        _: Mut<Self::Element>,
    ) {
        log::debug!("Teardown child {:?}", self.txt);
        // TODO
    }

    fn message(
        &self,
        _: &mut Self::ViewState,
        _: &[ViewId],
        message: DynMessage,
        _: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        log::debug!("Process child message of {:?}: {message:?}", self.txt);
        MessageResult::Nop
    }
}

// Custom parent view API
fn my_custom_parent_view<State, Action, Children>(
    children: Children,
) -> MyCustomParentView<impl HtmlElement<State, Action>, Children, State, Action>
where
    State: 'static,
    Action: 'static,
{
    let view = html::div(());
    MyCustomParentView {
        view,
        children,
        phantom: PhantomData,
    }
}

// Custom child view API
fn my_custom_child_view<State, Action>(txt: &'static str) -> MyCustomChildView<State, Action> {
    MyCustomChildView {
        txt,
        phantom: PhantomData,
    }
}

fn app_logic(_: &mut AppState) -> impl DomFragment<AppState> {
    (
        html::div(html::h1("Custom Context")),
        my_custom_parent_view((my_custom_child_view("foo"), my_custom_child_view("bar"))),
    )
}

pub fn main() {
    console_error_panic_hook::set_once();
    App::new(document_body(), AppState::default(), app_logic).run();
}
