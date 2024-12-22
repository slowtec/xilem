// Copyright 2024 the Xilem Authors and the Druid Authors
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, future::Future, marker::PhantomData, rc::Rc};

use futures::{Stream, StreamExt};
use wasm_bindgen::{closure::Closure, JsCast, UnwrapThrowExt};
use wasm_bindgen_futures::spawn_local;

use crate::{
    core::{MessageResult, Mut, NoElement, View, ViewId, ViewMarker, ViewPathTracker},
    DynMessage, MessageThunk, OptionalAction, ViewCtx,
};

/// Await a future returned by `init_future` invoked with the argument `data`, `callback` is called with the output of the future.
/// `init_future` will be invoked again, when `data` changes. Use [`memoized_await`] for construction of this [`View`]
pub struct MemoizedAwait<State, Action, OA, InitFuture, Data, Callback, F, FOut>(
    MemoizedFuture<State, Action, OA, InitFuture, Data, Callback, F, FOut>,
);

/// Await a stream returned by `init_stream` invoked with the argument `data`, `callback` is called with the items of the stream.
/// `init_stream` will be invoked again, when `data` changes. Use [`memoized_stream`] for construction of this [`View`]
pub struct MemoizedStream<State, Action, OA, InitStream, Data, Callback, F, StreamItem>(
    MemoizedFuture<State, Action, OA, InitStream, Data, Callback, F, StreamItem>,
);

struct MemoizedFuture<State, Action, OA, InitFuture, Data, Callback, F, FOut> {
    init_future: InitFuture,
    data: Data,
    callback: Callback,
    debounce_ms: usize,
    reset_debounce_on_update: bool,
    phantom: PhantomData<fn() -> (State, Action, OA, F, FOut)>,
}

impl<State, Action, OA, InitFuture, Data, Callback, F, FOut>
    MemoizedAwait<State, Action, OA, InitFuture, Data, Callback, F, FOut>
where
    FOut: fmt::Debug + 'static,
    F: Future<Output = FOut> + 'static,
    InitFuture: Fn(State, &Data) -> F,
{
    /// Debounce the `init_future` function, when `data` updates,
    /// when `reset_debounce_on_update == false` then this throttles updates each `milliseconds`
    ///
    /// The default for this is `0`
    pub fn debounce_ms(mut self, milliseconds: usize) -> Self {
        self.0.debounce_ms = milliseconds;
        self
    }

    /// When `reset` is `true`, everytime `data` updates, the debounce timeout is cleared until `init_future` is invoked.
    /// This is only effective when `debounce > 0`
    ///
    /// The default for this is `true`
    pub fn reset_debounce_on_update(mut self, reset: bool) -> Self {
        self.0.reset_debounce_on_update = reset;
        self
    }
}

impl<State, Action, OA, InitStream, Data, Callback, F, StreamItem>
    MemoizedStream<State, Action, OA, InitStream, Data, Callback, F, StreamItem>
where
    StreamItem: fmt::Debug + 'static,
    F: Stream<Item = StreamItem> + 'static,
    InitStream: Fn(State, &Data) -> F,
{
    /// Debounce the `init_stream` function, when `data` updates,
    /// when `reset_debounce_on_update == false` then this throttles updates each `milliseconds`
    ///
    /// The default for this is `0`
    pub fn debounce_ms(mut self, milliseconds: usize) -> Self {
        self.0.debounce_ms = milliseconds;
        self
    }

    /// When `reset` is `true`, everytime `data` updates, the debounce timeout is cleared until `init_stream` is invoked.
    /// This is only effective when `debounce > 0`
    ///
    /// The default for this is `true`
    pub fn reset_debounce_on_update(mut self, reset: bool) -> Self {
        self.0.reset_debounce_on_update = reset;
        self
    }
}

fn init_future<State, Action, OA, InitFuture, Data, Callback, F, FOut>(
    m: &MemoizedFuture<State, Action, OA, InitFuture, Data, Callback, F, FOut>,
    thunk: Rc<MessageThunk>,
    state: &State,
) where
    InitFuture: Fn(&State, &Data) -> F + 'static,
    FOut: fmt::Debug + 'static,
    F: Future<Output = FOut> + 'static,
{
    let future = (m.init_future)(state, &m.data);
    spawn_local(async move {
        thunk.push_message(MemoizedFutureMessage::<FOut>::Output(future.await));
    });
}

fn init_stream<State, Action, OA, InitStream, Data, Callback, F, StreamItem>(
    m: &MemoizedFuture<State, Action, OA, InitStream, Data, Callback, F, StreamItem>,
    thunk: Rc<MessageThunk>,
    state: &State,
) where
    InitStream: Fn(&State, &Data) -> F + 'static,
    StreamItem: fmt::Debug + 'static,
    F: Stream<Item = StreamItem> + 'static,
{
    let mut stream = Box::pin((m.init_future)(state, &m.data));
    spawn_local(async move {
        while let Some(item) = stream.next().await {
            thunk.push_message(MemoizedFutureMessage::<StreamItem>::Output(item));
        }
    });
}

/// Await a future returned by `init_future` invoked with the argument `data`, `callback` is called with the output of the resolved future. `init_future` will be invoked again, when `data` changes.
///
/// The update behavior can be controlled, by [`debounce_ms`](`MemoizedAwait::debounce_ms`) and [`reset_debounce_on_update`](`MemoizedAwait::reset_debounce_on_update`)
///
/// # Examples
///
/// ```
/// use xilem_web::{core::fork, concurrent::memoized_await, elements::html::div, interfaces::Element};
///
/// fn app_logic(state: &mut i32) -> impl Element<i32> {
///     fork(
///         div(*state),
///         memoized_await(
///             10,
///             |_, count| std::future::ready(*count),
///             |state, output| *state = output,
///         )
///     )
/// }
/// ```
pub fn memoized_await<State, Action, OA, InitFuture, Data, Callback, F, FOut>(
    data: Data,
    init_future: InitFuture,
    callback: Callback,
) -> MemoizedAwait<State, Action, OA, InitFuture, Data, Callback, F, FOut>
where
    State: 'static,
    Action: 'static,
    Data: PartialEq + 'static,
    FOut: fmt::Debug + 'static,
    F: Future<Output = FOut> + 'static,
    InitFuture: Fn(&State, &Data) -> F + 'static,
    OA: OptionalAction<Action> + 'static,
    Callback: Fn(&mut State, FOut) -> OA + 'static,
{
    MemoizedAwait(MemoizedFuture {
        init_future,
        data,
        callback,
        debounce_ms: 0,
        reset_debounce_on_update: true,
        phantom: PhantomData,
    })
}

/// Await a stream returned by `init_stream` invoked with the argument `data`, `callback` is called with the items of the stream.
/// `init_stream` will be invoked again, when `data` changes.
///
/// The behavior of the `init_stream` invocation by changes to `data` can be customized by [`debounce_ms`](`MemoizedStream::debounce_ms`) and [`reset_debounce_on_update`](`MemoizedStream::reset_debounce_on_update`).
///
/// # Examples
///
/// ```ignore
/// use gloo_timers::future::TimeoutFuture;
/// use async_stream::stream;
/// use xilem_web::{core::fork, concurrent::memoized_stream, elements::html, interfaces::Element};
///
/// fn app_logic(state: &mut Vec<usize>) -> impl Element<Vec<usize>> {
///     fork(
///         html::div(format!("{state:?}")),
///         memoized_stream(
///             10,
///             |_,n| {
///                 let range = 0..*n;
///                 stream! {
///                     for i in range {
///                         TimeoutFuture::new(500).await;
///                         yield i;
///                     }
///                 }
///             },
///             |state: &mut Vec<usize>, item: usize| {
///                 state.push(item);
///             }
///         )
///     )
/// }
/// ```
pub fn memoized_stream<State, Action, OA, InitStream, Data, Callback, F, StreamItem>(
    data: Data,
    init_future: InitStream,
    callback: Callback,
) -> MemoizedStream<State, Action, OA, InitStream, Data, Callback, F, StreamItem>
where
    State: 'static,
    Action: 'static,
    Data: PartialEq + 'static,
    StreamItem: fmt::Debug + 'static,
    F: Stream<Item = StreamItem> + 'static,
    InitStream: Fn(&State, &Data) -> F + 'static,
    OA: OptionalAction<Action> + 'static,
    Callback: Fn(&mut State, StreamItem) -> OA + 'static,
{
    MemoizedStream(MemoizedFuture {
        init_future,
        data,
        callback,
        debounce_ms: 0,
        reset_debounce_on_update: true,
        phantom: PhantomData,
    })
}

#[allow(unnameable_types)] // reason: Implementation detail, public because of trait visibility rules
pub struct MemoizedAwaitState {
    generation: u64,
    schedule_update: bool,
    // Closures are retained so they can be called by environment
    schedule_update_fn: Option<Closure<dyn FnMut()>>,
    schedule_update_timeout_handle: Option<i32>,
    update: bool,
    thunk: Rc<MessageThunk>,
}

impl MemoizedAwaitState {
    fn new(thunk: MessageThunk) -> Self {
        Self {
            generation: 0,
            schedule_update: false,
            schedule_update_fn: None,
            schedule_update_timeout_handle: None,
            update: false,
            thunk: Rc::new(thunk),
        }
    }
    fn clear_update_timeout(&mut self) {
        if let Some(handle) = self.schedule_update_timeout_handle {
            web_sys::window()
                .unwrap_throw()
                .clear_timeout_with_handle(handle);
        }
        self.schedule_update_timeout_handle = None;
        self.schedule_update_fn = None;
    }

    fn reset_debounce_timeout_and_schedule_update<FOut>(
        &mut self,
        ctx: &mut ViewCtx,
        debounce_duration: usize,
    ) where
        FOut: fmt::Debug + 'static,
    {
        ctx.with_id(ViewId::new(self.generation), |ctx| {
            self.clear_update_timeout();
            let thunk = ctx.message_thunk();
            let schedule_update_fn = Closure::new(move || {
                thunk.push_message(MemoizedFutureMessage::<FOut>::ScheduleUpdate);
            });
            let handle = web_sys::window()
                .unwrap_throw()
                .set_timeout_with_callback_and_timeout_and_arguments_0(
                    schedule_update_fn.as_ref().unchecked_ref(),
                    debounce_duration.try_into().unwrap_throw(),
                )
                .unwrap_throw();
            self.schedule_update_fn = Some(schedule_update_fn);
            self.schedule_update_timeout_handle = Some(handle);
            self.schedule_update = true;
        });
    }

    fn request_init<FOut>(&mut self, ctx: &mut ViewCtx)
    where
        FOut: fmt::Debug + 'static,
    {
        ctx.with_id(ViewId::new(self.generation), |ctx| {
            self.thunk = Rc::new(ctx.message_thunk());
            self.thunk
                .enqueue_message(MemoizedFutureMessage::<FOut>::RequestInit);
        });
    }
}

#[derive(Debug)]
enum MemoizedFutureMessage<Output: fmt::Debug> {
    Output(Output),
    ScheduleUpdate,
    RequestInit,
}

impl<State, Action, OA, InitFuture, Data, CB, F, FOut> ViewMarker
    for MemoizedAwait<State, Action, OA, InitFuture, Data, CB, F, FOut>
{
}
impl<State, Action, OA, InitStream, Data, CB, F, StreamItem> ViewMarker
    for MemoizedStream<State, Action, OA, InitStream, Data, CB, F, StreamItem>
{
}

impl<State, Action, InitFuture, F, FOut, Data, CB, OA> View<State, Action, ViewCtx, DynMessage>
    for MemoizedAwait<State, Action, OA, InitFuture, Data, CB, F, FOut>
where
    State: 'static,
    Action: 'static,
    OA: OptionalAction<Action> + 'static,
    InitFuture: Fn(&State, &Data) -> F + 'static,
    FOut: fmt::Debug + 'static,
    Data: PartialEq + 'static,
    F: Future<Output = FOut> + 'static,
    CB: Fn(&mut State, FOut) -> OA + 'static,
{
    type Element = NoElement;

    type ViewState = MemoizedAwaitState;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        self.0.build(ctx)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        (): Mut<Self::Element>,
    ) {
        self.0.rebuild(&prev.0, view_state, ctx);
    }

    fn teardown(&self, state: &mut Self::ViewState, _: &mut ViewCtx, (): Mut<Self::Element>) {
        self.0.teardown(state);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        self.0
            .message(view_state, id_path, message, app_state, init_future)
    }
}

impl<State, Action, InitStream, F, StreamItem, Data, CB, OA>
    View<State, Action, ViewCtx, DynMessage>
    for MemoizedStream<State, Action, OA, InitStream, Data, CB, F, StreamItem>
where
    State: 'static,
    Action: 'static,
    OA: OptionalAction<Action> + 'static,
    InitStream: Fn(&State, &Data) -> F + 'static,
    StreamItem: fmt::Debug + 'static,
    Data: PartialEq + 'static,
    F: Stream<Item = StreamItem> + 'static,
    CB: Fn(&mut State, StreamItem) -> OA + 'static,
{
    type Element = NoElement;

    type ViewState = MemoizedAwaitState;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        self.0.build(ctx)
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        (): Mut<Self::Element>,
    ) {
        self.0.rebuild(&prev.0, view_state, ctx);
    }

    fn teardown(&self, state: &mut Self::ViewState, _: &mut ViewCtx, (): Mut<Self::Element>) {
        self.0.teardown(state);
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        self.0
            .message(view_state, id_path, message, app_state, init_stream)
    }
}

impl<State, Action, InitFuture, F, FOut, Data, CB, OA>
    MemoizedFuture<State, Action, OA, InitFuture, Data, CB, F, FOut>
where
    State: 'static,
    Action: 'static,
    OA: OptionalAction<Action> + 'static,
    InitFuture: Fn(&State, &Data) -> F + 'static,
    FOut: fmt::Debug + 'static,
    Data: PartialEq + 'static,
    F: 'static,
    CB: Fn(&mut State, FOut) -> OA + 'static,
{
    fn build(&self, ctx: &mut ViewCtx) -> (NoElement, MemoizedAwaitState) {
        let thunk = ctx.message_thunk();
        let mut state = MemoizedAwaitState::new(thunk);

        if self.debounce_ms > 0 {
            state.reset_debounce_timeout_and_schedule_update::<FOut>(ctx, self.debounce_ms);
        } else {
            state.request_init::<FOut>(ctx);
        }

        (NoElement, state)
    }

    fn rebuild(&self, prev: &Self, view_state: &mut MemoizedAwaitState, ctx: &mut ViewCtx) {
        let debounce_has_changed_and_update_is_scheduled = view_state.schedule_update
            && (prev.reset_debounce_on_update != self.reset_debounce_on_update
                || prev.debounce_ms != self.debounce_ms);

        if debounce_has_changed_and_update_is_scheduled {
            if self.debounce_ms == 0 {
                if view_state.schedule_update_timeout_handle.is_some() {
                    view_state.clear_update_timeout();
                    view_state.schedule_update = false;
                    view_state.update = true;
                }
            } else {
                view_state
                    .reset_debounce_timeout_and_schedule_update::<FOut>(ctx, self.debounce_ms);
                return; // avoid update below, as it's already scheduled
            }
        }

        if view_state.update
            || (prev.data != self.data
                && (!view_state.schedule_update || self.reset_debounce_on_update))
        {
            if !view_state.update && self.debounce_ms > 0 {
                view_state
                    .reset_debounce_timeout_and_schedule_update::<FOut>(ctx, self.debounce_ms);
            } else {
                // no debounce
                view_state.generation += 1;
                view_state.update = false;
                view_state.request_init::<FOut>(ctx);
            }
        }
    }

    fn teardown(&self, state: &mut MemoizedAwaitState) {
        state.clear_update_timeout();
    }

    fn message<I>(
        &self,
        view_state: &mut MemoizedAwaitState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
        init_future: I,
    ) -> MessageResult<Action, DynMessage>
    where
        I: Fn(&Self, Rc<MessageThunk>, &State),
    {
        assert_eq!(id_path.len(), 1);
        if id_path[0].routing_id() == view_state.generation {
            match *message.downcast().unwrap_throw() {
                MemoizedFutureMessage::Output(future_output) => {
                    match (self.callback)(app_state, future_output).action() {
                        Some(action) => MessageResult::Action(action),
                        None => MessageResult::Nop,
                    }
                }
                MemoizedFutureMessage::ScheduleUpdate => {
                    view_state.update = true;
                    view_state.schedule_update = false;
                    MessageResult::RequestRebuild
                }
                MemoizedFutureMessage::RequestInit => {
                    init_future(self, Rc::clone(&view_state.thunk), app_state);
                    MessageResult::RequestRebuild
                }
            }
        } else {
            MessageResult::Stale(message)
        }
    }
}
