// Copyright 2023 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

use std::marker::PhantomData;

use crate::{
    concurrent::TaskProxy,
    core::{MessageResult, Mut, View, ViewId, ViewMarker},
    DomNode, DomView, DynMessage, Message, ViewCtx,
};

/// Invokes the `callback` after the inner `element` [`DomView`] was created.
/// See [`after_build`] for more details.
pub struct AfterBuild<State, Action, E, F> {
    element: E,
    callback: F,
    phantom: PhantomData<fn() -> (State, Action)>,
}

// TODO: add docs
pub struct AfterBuildWithProxy<State, Action, E, F, H, M> {
    element: E,
    callback: F,
    on_event: H,
    phantom: PhantomData<fn() -> (State, Action, M)>,
}

/// Invokes the `callback` after the inner `element` [`DomView<State>`]
/// See [`after_rebuild`] for more details.
pub struct AfterRebuild<State, Action, E, F> {
    element: E,
    callback: F,
    phantom: PhantomData<fn() -> (State, Action)>,
}

/// Invokes the `callback` before the inner `element` [`DomView`] (and its underlying DOM node) is destroyed.
/// See [`before_teardown`] for more details.
pub struct BeforeTeardown<State, Action, E, F> {
    element: E,
    callback: F,
    phantom: PhantomData<fn() -> (State, Action)>,
}

pub struct BeforeTeardownWithProxy<State, Action, E, F, H, M> {
    element: E,
    callback: F,
    on_event: H,
    phantom: PhantomData<fn() -> (State, Action, M)>,
}

/// Invokes the `callback` after the inner `element` [`DomView`] was created.
/// The callback has a reference to the raw DOM node as its only parameter.
///
/// Caution: At this point, however,
/// no properties have been applied to the node.
///
/// As accessing the underlying raw DOM node can mess with the inner logic of `xilem_web`,
/// this should only be used as an escape-hatch for properties not supported by `xilem_web`.
/// E.g. to be interoperable with external javascript libraries.
pub fn after_build<State, Action, E, F>(element: E, callback: F) -> AfterBuild<State, Action, E, F>
where
    State: 'static,
    Action: 'static,
    E: DomView<State, Action> + 'static,
    F: Fn(&E::DomNode) + 'static,
{
    AfterBuild {
        element,
        callback,
        phantom: PhantomData,
    }
}

// TODO: add docs
pub fn after_build_with_proxy<State, Action, E, F, H, M>(
    element: E,
    callback: F,
    on_event: H,
) -> AfterBuildWithProxy<State, Action, E, F, H, M>
where
    State: 'static,
    Action: 'static,
    E: DomView<State, Action> + 'static,
    F: Fn(&E::DomNode, TaskProxy) + 'static,
    H: Fn(&mut State, M) -> Action + 'static,
    M: Message,
{
    AfterBuildWithProxy {
        element,
        callback,
        on_event,
        phantom: PhantomData,
    }
}

/// Invokes the `callback` after the inner `element` [`DomView<State>`]
/// was rebuild, which usually happens after anything has changed in the `State` .
///
/// Memoization can prevent `callback` being called.
/// The callback has a reference to the raw DOM node as its only parameter.
///
/// As accessing the underlying raw DOM node can mess with the inner logic of `xilem_web`,
/// this should only be used as an escape-hatch for properties not supported by `xilem_web`.
/// E.g. to be interoperable with external javascript libraries.
pub fn after_rebuild<State, Action, E, F>(
    element: E,
    callback: F,
) -> AfterRebuild<State, Action, E, F>
where
    State: 'static,
    Action: 'static,
    E: DomView<State, Action> + 'static,
    F: Fn(&E::DomNode) + 'static,
{
    AfterRebuild {
        element,
        callback,
        phantom: PhantomData,
    }
}

/// Invokes the `callback` before the inner `element` [`DomView`] (and its underlying DOM node) is destroyed.
///
/// As accessing the underlying raw DOM node can mess with the inner logic of `xilem_web`,
/// this should only be used as an escape-hatch for properties not supported by `xilem_web`.
/// E.g. to be interoperable with external javascript libraries.
pub fn before_teardown<State, Action, E, F>(
    element: E,
    callback: F,
) -> BeforeTeardown<State, Action, E, F>
where
    State: 'static,
    Action: 'static,
    E: DomView<State, Action> + 'static,
    F: Fn(&E::DomNode) + 'static,
{
    BeforeTeardown {
        element,
        callback,
        phantom: PhantomData,
    }
}

pub fn before_teardown_with_proxy<State, Action, E, F, H, M>(
    element: E,
    callback: F,
    on_event: H,
) -> BeforeTeardownWithProxy<State, Action, E, F, H, M>
where
    State: 'static,
    Action: 'static,
    E: DomView<State, Action> + 'static,
    F: Fn(&E::DomNode, TaskProxy) + 'static,
    H: Fn(&mut State, M) -> Action + 'static,
{
    BeforeTeardownWithProxy {
        element,
        callback,
        on_event,
        phantom: PhantomData,
    }
}

impl<State, Action, E, F> ViewMarker for AfterBuild<State, Action, E, F> {}
impl<State, Action, E, F, H, M> ViewMarker for AfterBuildWithProxy<State, Action, E, F, H, M> {}
impl<State, Action, E, F> ViewMarker for AfterRebuild<State, Action, E, F> {}
impl<State, Action, E, F> ViewMarker for BeforeTeardown<State, Action, E, F> {}
impl<State, Action, E, F, H, M> ViewMarker for BeforeTeardownWithProxy<State, Action, E, F, H, M> {}

impl<State, Action, V, F> View<State, Action, ViewCtx, DynMessage>
    for AfterBuild<State, Action, V, F>
where
    State: 'static,
    Action: 'static,
    F: Fn(&V::DomNode) + 'static,
    V: DomView<State, Action> + 'static,
{
    type Element = V::Element;

    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        let (mut el, view_state) = self.element.build(ctx);
        el.node.apply_props(&mut el.props, &mut el.flags);
        (self.callback)(&el.node);
        (el, view_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<Self::Element>,
    ) {
        self.element
            .rebuild(&prev.element, view_state, ctx, element);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        el: Mut<Self::Element>,
    ) {
        self.element.teardown(view_state, ctx, el);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        self.element
            .message(view_state, id_path, message, app_state)
    }
}

impl<State, Action, V, F, H, M> View<State, Action, ViewCtx, DynMessage>
    for AfterBuildWithProxy<State, Action, V, F, H, M>
where
    State: 'static,
    Action: 'static,
    F: Fn(&V::DomNode, TaskProxy) + 'static,
    H: Fn(&mut State, M) -> Action + 'static,
    V: DomView<State, Action> + 'static,
    M: Message,
{
    type Element = V::Element;

    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        let (mut el, view_state) = self.element.build(ctx);
        el.node.apply_props(&mut el.props, &mut el.flags);
        let thunk = ctx.message_thunk();
        let proxy = TaskProxy::new(thunk);
        (self.callback)(&el.node, proxy);
        (el, view_state)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<Self::Element>,
    ) {
        self.element
            .rebuild(&prev.element, view_state, ctx, element);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        el: Mut<Self::Element>,
    ) {
        self.element.teardown(view_state, ctx, el);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        match message.downcast::<M>() {
            Ok(message) => {
                let action = (self.on_event)(app_state, *message);
                MessageResult::Action(action)
            }
            Err(message) => self
                .element
                .message(view_state, id_path, message, app_state),
        }
    }
}

impl<State, Action, V, F> View<State, Action, ViewCtx, DynMessage>
    for AfterRebuild<State, Action, V, F>
where
    State: 'static,
    Action: 'static,
    F: Fn(&V::DomNode) + 'static,
    V: DomView<State, Action> + 'static,
{
    type Element = V::Element;

    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        self.element.build(ctx)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<Self::Element>,
    ) {
        self.element
            .rebuild(&prev.element, view_state, ctx, element.reborrow_mut());
        element.node.apply_props(element.props, element.flags);
        (self.callback)(element.node);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        el: Mut<Self::Element>,
    ) {
        self.element.teardown(view_state, ctx, el);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        self.element
            .message(view_state, id_path, message, app_state)
    }
}

impl<State, Action, V, F> View<State, Action, ViewCtx, DynMessage>
    for BeforeTeardown<State, Action, V, F>
where
    State: 'static,
    Action: 'static,
    F: Fn(&V::DomNode) + 'static,
    V: DomView<State, Action> + 'static,
{
    type Element = V::Element;

    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        self.element.build(ctx)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<Self::Element>,
    ) {
        self.element
            .rebuild(&prev.element, view_state, ctx, element);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        el: Mut<Self::Element>,
    ) {
        (self.callback)(el.node);
        self.element.teardown(view_state, ctx, el);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        self.element
            .message(view_state, id_path, message, app_state)
    }
}

impl<State, Action, V, F, H, M> View<State, Action, ViewCtx, DynMessage>
    for BeforeTeardownWithProxy<State, Action, V, F, H, M>
where
    State: 'static,
    Action: 'static,
    F: Fn(&V::DomNode, TaskProxy) + 'static,
    H: Fn(&mut State, M) -> Action + 'static,
    V: DomView<State, Action> + 'static,
    M: Message,
{
    type Element = V::Element;

    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        self.element.build(ctx)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<Self::Element>,
    ) {
        self.element
            .rebuild(&prev.element, view_state, ctx, element);
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        el: Mut<Self::Element>,
    ) {
        let thunk = ctx.message_thunk();
        let proxy = TaskProxy::new(thunk);
        (self.callback)(el.node, proxy);
        self.element.teardown(view_state, ctx, el);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        match message.downcast::<M>() {
            Ok(message) => {
                let action = (self.on_event)(app_state, *message);
                MessageResult::Action(action)
            }
            Err(message) => self
                .element
                .message(view_state, id_path, message, app_state),
        }
    }
}
