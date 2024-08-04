// Copyright 2023 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

use xilem_core::{MessageResult, Mut, View, ViewId};

use crate::{DynMessage, ViewCtx};

pub struct AfterBuild<E, F> {
    element: E,
    callback: F,
}

impl<E, F> AfterBuild<E, F> {
    pub fn new(element: E, callback: F) -> AfterBuild<E, F> {
        Self { element, callback }
    }
}

impl<State, Action, V, F> View<State, Action, ViewCtx, DynMessage> for AfterBuild<V, F>
where
    F: Fn(&V::Element) + 'static,
    V: View<State, Action, ViewCtx, DynMessage>,
{
    type Element = V::Element;

    type ViewState = V::ViewState;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        let (el, view_state) = self.element.build(ctx);
        (self.callback)(&el);
        (el, view_state)
    }

    fn rebuild<'el>(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<'el, Self::Element>,
    ) -> Mut<'el, Self::Element> {
        self.element
            .rebuild(&prev.element, view_state, ctx, element)
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        el: Mut<'_, Self::Element>,
    ) {
        self.element.teardown(view_state, ctx, el)
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
