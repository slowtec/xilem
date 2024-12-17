// Copyright 2023 the Xilem Authors
// SPDX-License-Identifier: Apache-2.0

use std::{borrow::Cow, marker::PhantomData};

use wasm_bindgen::{prelude::Closure, throw_str, JsCast, UnwrapThrowExt};
use web_sys::{js_sys, AddEventListenerOptions};

use crate::{
    core::{MessageResult, Mut, View, ViewId, ViewMarker, ViewPathTracker},
    event_handler::{EventHandler, EventHandlerMessage},
    DomView, DynMessage, OptionalAction, ViewCtx,
};

/// Use a distinctive number here, to be able to catch bugs.
/// In case the generational-id view path in `View::Message` lead to a wrong view
const ON_EVENT_VIEW_ID: ViewId = ViewId::new(0x2357_1113);
const EVENT_HANDLER_ID: ViewId = ViewId::new(0x2357_1114);

/// Wraps a [`View`] `V` and attaches an event listener.
///
/// The event type `Event` should inherit from [`web_sys::Event`]
#[derive(Clone, Debug)]
pub struct OnEvent<V, State, Action, OA, Event, Handler = fn(&mut State, Event) -> OA> {
    pub(crate) dom_view: V,
    pub(crate) event: Cow<'static, str>,
    pub(crate) capture: bool,
    pub(crate) passive: bool,
    pub(crate) handler: Handler,
    pub(crate) phantom_event_ty: PhantomData<fn() -> (State, Action, OA, Event)>,
}

impl<V, State, Action, OA, Event, Handler> OnEvent<V, State, Action, OA, Event, Handler>
where
    Event: JsCast + 'static,
{
    pub fn new(dom_view: V, event: impl Into<Cow<'static, str>>, handler: Handler) -> Self {
        OnEvent {
            dom_view,
            event: event.into(),
            passive: true,
            capture: false,
            handler,
            phantom_event_ty: PhantomData,
        }
    }

    /// Whether the event handler should be passive. (default = `true`)
    ///
    /// Passive event handlers can't prevent the browser's default action from
    /// running (otherwise possible with `event.prevent_default()`), which
    /// restricts what they can be used for, but reduces overhead.
    pub fn passive(mut self, value: bool) -> Self {
        self.passive = value;
        self
    }

    /// Whether the event handler should capture the event *before* being dispatched to any `EventTarget` beneath it in the DOM tree. (default = `false`)
    ///
    /// Events that are bubbling upward through the tree will not trigger a listener designated to use capture.
    /// Event bubbling and capturing are two ways of propagating events that occur in an element that is nested within another element,
    /// when both elements have registered a handle for that event.
    /// The event propagation mode determines the order in which elements receive the event.
    // TODO use similar Nomenclature as gloo (Phase::Bubble/Phase::Capture)?
    pub fn capture(mut self, value: bool) -> Self {
        self.capture = value;
        self
    }
}

fn create_event_listener<Event: JsCast + crate::Message>(
    target: &web_sys::EventTarget,
    event: &str,
    // TODO options
    capture: bool,
    passive: bool,
    ctx: &mut ViewCtx,
) -> Closure<dyn FnMut(web_sys::Event)> {
    let thunk = ctx.message_thunk();
    let callback = Closure::new(move |event: web_sys::Event| {
        let event = event.unchecked_into::<Event>();
        thunk.push_message(event);
    });

    let options = AddEventListenerOptions::new();
    options.set_capture(capture);
    options.set_passive(passive);

    target
        .add_event_listener_with_callback_and_add_event_listener_options(
            event,
            callback.as_ref().unchecked_ref(),
            &options,
        )
        .unwrap_throw();
    callback
}

fn remove_event_listener(
    target: &web_sys::EventTarget,
    event: &str,
    callback: &Closure<dyn FnMut(web_sys::Event)>,
    is_capture: bool,
) {
    target
        .remove_event_listener_with_callback_and_bool(
            event,
            callback.as_ref().unchecked_ref(),
            is_capture,
        )
        .unwrap_throw();
}

mod hidden {
    use wasm_bindgen::prelude::Closure;
    #[allow(unnameable_types)] // reason: Implementation detail, public because of trait visibility rules
    /// State for the `OnEvent` view.
    pub struct OnEventState<CS, HS> {
        pub(crate) child_state: CS,
        pub(crate) handler_state: HS,
        pub(crate) callback: Closure<dyn FnMut(web_sys::Event)>,
    }
}

use hidden::OnEventState;

// These (boilerplatey) functions are there to reduce the boilerplate created by the macro-expansion below.

fn build_event_listener<State, Action, OA, V, Handler, Event>(
    element_view: &V,
    event_handler: &Handler,
    event: &str,
    capture: bool,
    passive: bool,
    ctx: &mut ViewCtx,
) -> (V::Element, OnEventState<V::ViewState, Handler::State>)
where
    State: 'static,
    Action: 'static,
    OA: OptionalAction<Action>,
    V: DomView<State, Action>,
    Event: JsCast + 'static + crate::Message,
    Handler: EventHandler<Event, State, OA, ViewCtx>,
{
    let handler_state = ctx.with_id(EVENT_HANDLER_ID, |ctx| event_handler.build(ctx));
    let (element, (child_state, callback)) = ctx.with_id(ON_EVENT_VIEW_ID, |ctx| {
        let (element, child_state) = element_view.build(ctx);
        let callback =
            create_event_listener::<Event>(element.as_ref(), event, capture, passive, ctx);
        (element, (child_state, callback))
    });
    let state = OnEventState {
        child_state,
        handler_state,
        callback,
    };
    (element, state)
}

#[allow(clippy::too_many_arguments)] // reason: This is only used to avoid more boilerplate in macros, also so that rust-analyzer can be of help here.
fn rebuild_event_listener<State, Action, OA, Handler, V, Event>(
    element_view: &V,
    prev_element_view: &V,
    event_handler: &Handler,
    prev_event_handler: &Handler,
    mut element: Mut<V::Element>,
    event: &str,
    capture: bool,
    passive: bool,
    prev_capture: bool,
    prev_passive: bool,
    state: &mut OnEventState<V::ViewState, Handler::State>,
    ctx: &mut ViewCtx,
) where
    State: 'static,
    Action: 'static,
    OA: OptionalAction<Action>,
    V: DomView<State, Action>,
    Event: JsCast + 'static + crate::Message,
    Handler: EventHandler<Event, State, OA, ViewCtx>,
{
    ctx.with_id(EVENT_HANDLER_ID, |ctx| {
        event_handler.rebuild(prev_event_handler, &mut state.handler_state, ctx);
    });
    ctx.with_id(ON_EVENT_VIEW_ID, |ctx| {
        element_view.rebuild(
            prev_element_view,
            &mut state.child_state,
            ctx,
            element.reborrow_mut(),
        );
        let was_created = element.flags.was_created();
        let needs_update = prev_capture != capture || prev_passive != passive || was_created;
        if !needs_update {
            return;
        }
        if !was_created {
            remove_event_listener(element.as_ref(), event, &state.callback, prev_capture);
        }
        state.callback =
            create_event_listener::<Event>(element.as_ref(), event, capture, passive, ctx);
    });
}

fn teardown_event_listener<State, Action, Event, OA, Handler, V>(
    element_view: &V,
    event_handler: &Handler,
    element: Mut<V::Element>,
    _event: &str,
    state: &mut OnEventState<V::ViewState, Handler::State>,
    _capture: bool,
    ctx: &mut ViewCtx,
) where
    State: 'static,
    Action: 'static,
    OA: OptionalAction<Action>,
    V: DomView<State, Action>,
    Handler: EventHandler<Event, State, OA, ViewCtx>,
{
    ctx.with_id(EVENT_HANDLER_ID, |ctx| {
        event_handler.teardown(&mut state.handler_state, ctx);
    });
    // TODO: is this really needed (as the element will be removed anyway)?
    // remove_event_listener(element.as_ref(), event, &state.callback, capture);
    ctx.with_id(ON_EVENT_VIEW_ID, |ctx| {
        element_view.teardown(&mut state.child_state, ctx, element);
    });
}

fn message_event_listener<State, Action, V, Event, OA, Handler>(
    element_view: &V,
    handler: &Handler,
    state: &mut OnEventState<V::ViewState, Handler::State>,
    id_path: &[ViewId],
    message: DynMessage,
    app_state: &mut State,
) -> MessageResult<Action, DynMessage>
where
    State: 'static,
    Action: 'static,
    V: DomView<State, Action>,
    Event: JsCast + 'static + crate::Message,
    OA: OptionalAction<Action>,
    Handler: EventHandler<Event, State, OA, ViewCtx> + 'static,
{
    let Some((first, remainder)) = id_path.split_first() else {
        throw_str("Parent view of `OnEvent` sent outdated and/or incorrect empty view path");
    };

    let handler_message = if *first == EVENT_HANDLER_ID {
        EventHandlerMessage::Message(message)
    } else if *first == ON_EVENT_VIEW_ID {
        if remainder.is_empty() {
            EventHandlerMessage::Event(*message.downcast::<Event>().unwrap_throw())
        } else {
            return element_view.message(&mut state.child_state, remainder, message, app_state);
        }
    } else {
        throw_str("Parent view of `OnEvent` sent outdated and/or incorrect empty view path");
    };
    match handler
        .message(&mut state.handler_state, &[], handler_message, app_state)
        .map(|action| action.action())
    {
        MessageResult::Action(Some(action)) => MessageResult::Action(action),
        MessageResult::Nop | MessageResult::Action(None) => MessageResult::Nop,
        MessageResult::RequestRebuild => MessageResult::RequestRebuild,
        MessageResult::Stale(EventHandlerMessage::Event(event)) => {
            MessageResult::Stale(Box::new(event))
        }
        MessageResult::Stale(EventHandlerMessage::Message(message)) => {
            MessageResult::Stale(message)
        }
    }
}

impl<V, State, Action, Event, Handler, OA> ViewMarker
    for OnEvent<V, State, Action, OA, Event, Handler>
{
}
impl<V, State, Action, Event, Handler, OA> View<State, Action, ViewCtx, DynMessage>
    for OnEvent<V, State, Action, OA, Event, Handler>
where
    State: 'static,
    Action: 'static,
    V: DomView<State, Action>,
    OA: OptionalAction<Action> + 'static,
    Handler: EventHandler<Event, State, OA, ViewCtx>,
    Event: JsCast + 'static + crate::Message,
{
    type ViewState = OnEventState<V::ViewState, Handler::State>;

    type Element = V::Element;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        build_event_listener::<_, _, _, _, _, Event>(
            &self.dom_view,
            &self.handler,
            &self.event,
            self.capture,
            self.passive,
            ctx,
        )
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<Self::Element>,
    ) {
        ctx.with_id(EVENT_HANDLER_ID, |ctx| {
            self.handler
                .rebuild(&prev.handler, &mut view_state.handler_state, ctx);
        });
        // special case, where event name can change, so we can't reuse the rebuild_event_listener function above
        ctx.with_id(ON_EVENT_VIEW_ID, |ctx| {
            self.dom_view.rebuild(
                &prev.dom_view,
                &mut view_state.child_state,
                ctx,
                element.reborrow_mut(),
            );

            let was_created = element.flags.was_created();
            let needs_update = prev.capture != self.capture
                || prev.passive != self.passive
                || prev.event != self.event
                || was_created;
            if !needs_update {
                return;
            }
            if !was_created {
                remove_event_listener(
                    element.as_ref(),
                    &prev.event,
                    &view_state.callback,
                    prev.capture,
                );
            }

            view_state.callback = create_event_listener::<Event>(
                element.as_ref(),
                &self.event,
                self.capture,
                self.passive,
                ctx,
            );
        });
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<Self::Element>,
    ) {
        teardown_event_listener(
            &self.dom_view,
            &self.handler,
            element,
            &self.event,
            view_state,
            self.capture,
            ctx,
        );
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: crate::DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        message_event_listener(
            &self.dom_view,
            &self.handler,
            view_state,
            id_path,
            message,
            app_state,
        )
    }
}

macro_rules! event_definitions {
    ($(($ty_name:ident, $event_name:literal, $web_sys_ty:ident)),*) => {
        $(
        pub struct $ty_name<V, State, Action, OA, Handler> {
            pub(crate) dom_view: V,
            pub(crate) capture: bool,
            pub(crate) passive: bool,
            pub(crate) handler: Handler,
            pub(crate) phantom_event_ty: PhantomData<fn() -> (State, Action, OA)>,
        }

        impl<V, State, Action, OA, Handler> ViewMarker for $ty_name<V, State, Action, OA, Handler> {}
        impl<V, State, Action, OA, Handler> $ty_name<V, State, Action, OA, Handler> {
            pub fn new(dom_view: V, handler: Handler) -> Self {
                Self {
                    dom_view,
                    passive: true,
                    capture: false,
                    handler,
                    phantom_event_ty: PhantomData,
                }
            }

            /// Whether the event handler should be passive. (default = `true`)
            ///
            /// Passive event handlers can't prevent the browser's default action from
            /// running (otherwise possible with `event.prevent_default()`), which
            /// restricts what they can be used for, but reduces overhead.
            pub fn passive(mut self, value: bool) -> Self {
                self.passive = value;
                self
            }

            /// Whether the event handler should capture the event *before* being dispatched to any `EventTarget` beneath it in the DOM tree. (default = `false`)
            ///
            /// Events that are bubbling upward through the tree will not trigger a listener designated to use capture.
            /// Event bubbling and capturing are two ways of propagating events that occur in an element that is nested within another element,
            /// when both elements have registered a handle for that event.
            /// The event propagation mode determines the order in which elements receive the event.
            // TODO use similar Nomenclature as gloo (Phase::Bubble/Phase::Capture)?
            pub fn capture(mut self, value: bool) -> Self {
                self.capture = value;
                self
            }
        }


        impl<V, State, Action, Handler, OA> View<State, Action, ViewCtx, DynMessage>
            for $ty_name<V, State, Action, OA, Handler>
        where
            State: 'static,
            Action: 'static,
            V: DomView<State, Action>,
            OA: OptionalAction<Action> + 'static,
            Handler: EventHandler<web_sys::$web_sys_ty, State, OA, ViewCtx>,
        {
            type ViewState = OnEventState<V::ViewState, Handler::State>;

            type Element = V::Element;

            fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
                build_event_listener::<_, _, _, _, _, web_sys::$web_sys_ty>(
                    &self.dom_view,
                    &self.handler,
                    $event_name,
                    self.capture,
                    self.passive,
                    ctx,
                )
            }

            fn rebuild(
                &self,
                prev: &Self,
                view_state: &mut Self::ViewState,
                ctx: &mut ViewCtx,
                element: Mut<Self::Element>,
            ) {
                rebuild_event_listener::<_, _, _,_,_, web_sys::$web_sys_ty>(
                    &self.dom_view,
                    &prev.dom_view,
                    &self.handler,
                    &prev.handler,
                    element,
                    $event_name,
                    self.capture,
                    self.passive,
                    prev.capture,
                    prev.passive,
                    view_state,
                    ctx,
                );
            }

            fn teardown(
                &self,
                view_state: &mut Self::ViewState,
                ctx: &mut ViewCtx,
                element: Mut<Self::Element>,
            ) {
                teardown_event_listener(&self.dom_view, &self.handler, element, $event_name, view_state, self.capture, ctx);
            }

            fn message(
                &self,
                view_state: &mut Self::ViewState,
                id_path: &[ViewId],
                message: crate::DynMessage,
                app_state: &mut State,
            ) -> MessageResult<Action, DynMessage> {
                message_event_listener(&self.dom_view, &self.handler, view_state, id_path, message, app_state)
            }
        }
        )*
    };
}

event_definitions!(
    (OnAbort, "abort", Event),
    (OnAuxClick, "auxclick", PointerEvent),
    (OnBeforeInput, "beforeinput", InputEvent),
    (OnBeforeMatch, "beforematch", Event),
    (OnBeforeToggle, "beforetoggle", Event),
    (OnBlur, "blur", FocusEvent),
    (OnCancel, "cancel", Event),
    (OnCanPlay, "canplay", Event),
    (OnCanPlayThrough, "canplaythrough", Event),
    (OnChange, "change", Event),
    (OnClick, "click", PointerEvent),
    (OnClose, "close", Event),
    (OnContextLost, "contextlost", Event),
    (OnContextMenu, "contextmenu", PointerEvent),
    (OnContextRestored, "contextrestored", Event),
    (OnCopy, "copy", Event),
    (OnCueChange, "cuechange", Event),
    (OnCut, "cut", Event),
    (OnDblClick, "dblclick", MouseEvent),
    (OnDrag, "drag", Event),
    (OnDragEnd, "dragend", Event),
    (OnDragEnter, "dragenter", Event),
    (OnDragLeave, "dragleave", Event),
    (OnDragOver, "dragover", Event),
    (OnDragStart, "dragstart", Event),
    (OnDrop, "drop", Event),
    (OnDurationChange, "durationchange", Event),
    (OnEmptied, "emptied", Event),
    (OnEnded, "ended", Event),
    (OnError, "error", Event),
    (OnFocus, "focus", FocusEvent),
    (OnFocusIn, "focusin", FocusEvent),
    (OnFocusOut, "focusout", FocusEvent),
    (OnFormData, "formdata", Event),
    (OnInput, "input", Event),
    (OnInvalid, "invalid", Event),
    (OnKeyDown, "keydown", KeyboardEvent),
    (OnKeyUp, "keyup", KeyboardEvent),
    (OnLoad, "load", Event),
    (OnLoadedData, "loadeddata", Event),
    (OnLoadedMetadata, "loadedmetadata", Event),
    (OnLoadStart, "loadstart", Event),
    (OnMouseDown, "mousedown", MouseEvent),
    (OnMouseEnter, "mouseenter", MouseEvent),
    (OnMouseLeave, "mouseleave", MouseEvent),
    (OnMouseMove, "mousemove", MouseEvent),
    (OnMouseOut, "mouseout", MouseEvent),
    (OnMouseOver, "mouseover", MouseEvent),
    (OnMouseUp, "mouseup", MouseEvent),
    (OnPaste, "paste", Event),
    (OnPause, "pause", Event),
    (OnPlay, "play", Event),
    (OnPlaying, "playing", Event),
    (OnPointerCancel, "pointercancel", PointerEvent),
    (OnPointerDown, "pointerdown", PointerEvent),
    (OnPointerEnter, "pointerenter", PointerEvent),
    (OnPointerLeave, "pointerleave", PointerEvent),
    (OnPointerMove, "pointermove", PointerEvent),
    (OnPointerOut, "pointerout", PointerEvent),
    (OnPointerOver, "pointerover", PointerEvent),
    (OnPointerRawUpdate, "pointerrawupdate", PointerEvent),
    (OnPointerUp, "pointerup", PointerEvent),
    (OnProgress, "progress", Event),
    (OnRateChange, "ratechange", Event),
    (OnReset, "reset", Event),
    (OnScroll, "scroll", Event),
    (OnScrollEnd, "scrollend", Event),
    (OnSecurityPolicyViolation, "securitypolicyviolation", Event),
    (OnSeeked, "seeked", Event),
    (OnSeeking, "seeking", Event),
    (OnSelect, "select", Event),
    (OnSlotChange, "slotchange", Event),
    (OnStalled, "stalled", Event),
    (OnSubmit, "submit", Event),
    (OnSuspend, "suspend", Event),
    (OnTimeUpdate, "timeupdate", Event),
    (OnToggle, "toggle", Event),
    (OnVolumeChange, "volumechange", Event),
    (OnWaiting, "waiting", Event),
    (OnWheel, "wheel", WheelEvent)
);

pub struct OnResize<V, State, Action, Callback> {
    pub(crate) dom_view: V,
    pub(crate) handler: Callback,
    pub(crate) phantom_event_ty: PhantomData<fn() -> (State, Action)>,
}

pub struct OnResizeState<VState> {
    child_state: VState,
    // reason: Closures are retained so they can be called by environment
    #[allow(unused)]
    callback: Closure<dyn FnMut(js_sys::Array)>,
    observer: web_sys::ResizeObserver,
}

impl<V, State, Action, Callback> ViewMarker for OnResize<V, State, Action, Callback> {}
impl<State, Action, OA, Callback, V: View<State, Action, ViewCtx, DynMessage>>
    View<State, Action, ViewCtx, DynMessage> for OnResize<V, State, Action, Callback>
where
    State: 'static,
    Action: 'static,
    OA: OptionalAction<Action>,
    Callback: Fn(&mut State, web_sys::ResizeObserverEntry) -> OA + 'static,
    V: DomView<State, Action, DomNode: AsRef<web_sys::Element>>,
{
    type Element = V::Element;

    type ViewState = OnResizeState<V::ViewState>;

    fn build(&self, ctx: &mut ViewCtx) -> (Self::Element, Self::ViewState) {
        ctx.with_id(ON_EVENT_VIEW_ID, |ctx| {
            let thunk = ctx.message_thunk();
            let callback = Closure::new(move |entries: js_sys::Array| {
                let entry: web_sys::ResizeObserverEntry = entries.at(0).unchecked_into();
                thunk.push_message(entry);
            });

            let observer =
                web_sys::ResizeObserver::new(callback.as_ref().unchecked_ref()).unwrap_throw();
            let (element, child_state) = self.dom_view.build(ctx);
            observer.observe(element.as_ref());

            let state = OnResizeState {
                child_state,
                callback,
                observer,
            };

            (element, state)
        })
    }

    fn rebuild(
        &self,
        prev: &Self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        mut element: Mut<Self::Element>,
    ) {
        ctx.with_id(ON_EVENT_VIEW_ID, |ctx| {
            self.dom_view.rebuild(
                &prev.dom_view,
                &mut view_state.child_state,
                ctx,
                element.reborrow_mut(),
            );
            if element.flags.was_created() {
                view_state.observer.disconnect();
                view_state.observer.observe(element.as_ref());
            }
        });
    }

    fn teardown(
        &self,
        view_state: &mut Self::ViewState,
        ctx: &mut ViewCtx,
        element: Mut<Self::Element>,
    ) {
        ctx.with_id(ON_EVENT_VIEW_ID, |ctx| {
            view_state.observer.disconnect();
            self.dom_view
                .teardown(&mut view_state.child_state, ctx, element);
        });
    }

    fn message(
        &self,
        view_state: &mut Self::ViewState,
        id_path: &[ViewId],
        message: DynMessage,
        app_state: &mut State,
    ) -> MessageResult<Action, DynMessage> {
        let Some((first, remainder)) = id_path.split_first() else {
            throw_str("Parent view of `OnResize` sent outdated and/or incorrect empty view path");
        };
        if *first != ON_EVENT_VIEW_ID {
            throw_str("Parent view of `OnResize` sent outdated and/or incorrect empty view path");
        }
        if remainder.is_empty() {
            let event = message
                .downcast::<web_sys::ResizeObserverEntry>()
                .unwrap_throw();
            match (self.handler)(app_state, *event).action() {
                Some(a) => MessageResult::Action(a),
                None => MessageResult::Nop,
            }
        } else {
            self.dom_view
                .message(&mut view_state.child_state, remainder, message, app_state)
        }
    }
}
