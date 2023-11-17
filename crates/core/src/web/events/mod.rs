#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
mod csr;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub use csr::*;

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
mod ssr;
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub use ssr::*;

macro_rules! generate_event_types {
    {$(
      $( #[$does_not_bubble:ident] )?
      $event:ident : $web_sys_event:ident
    ),* $(,)?} => {
  
      $(
          #[doc = concat!("The `", stringify!($event), "` event, which receives [", stringify!($web_sys_event), "](web_sys::", stringify!($web_sys_event), ") as its argument.")]
          #[derive(Copy, Clone)]
          #[allow(non_camel_case_types)]
          pub struct $event;
  
          impl EventDescriptor for $event {
            type EventType = web_sys::$web_sys_event;
  
            fn name(&self) -> Cow<'static, str> {
              stringify!($event).into()
            }
  
            $(
              generate_event_types!($does_not_bubble);
            )?
          }
      )*
    };
  
    (does_not_bubble) => {
      fn bubbles(&self) -> bool { false }
    }
  }
  
  generate_event_types! {
    // =========================================================
    // WindowEventHandlersEventMap
    // =========================================================
    afterprint: Event,
    beforeprint: Event,
    beforeunload: BeforeUnloadEvent,
    gamepadconnected: GamepadEvent,
    gamepaddisconnected: GamepadEvent,
    hashchange: HashChangeEvent,
    languagechange: Event,
    message: MessageEvent,
    messageerror: MessageEvent,
    offline: Event,
    online: Event,
    pagehide: PageTransitionEvent,
    pageshow: PageTransitionEvent,
    popstate: PopStateEvent,
    rejectionhandled: PromiseRejectionEvent,
    storage: StorageEvent,
    unhandledrejection: PromiseRejectionEvent,
    #[does_not_bubble]
    unload: Event,
  
    // =========================================================
    // GlobalEventHandlersEventMap
    // =========================================================
    #[does_not_bubble]
    abort: UiEvent,
    animationcancel: AnimationEvent,
    animationend: AnimationEvent,
    animationiteration: AnimationEvent,
    animationstart: AnimationEvent,
    auxclick: MouseEvent,
    beforeinput: InputEvent,
    #[does_not_bubble]
    blur: FocusEvent,
    canplay: Event,
    canplaythrough: Event,
    change: Event,
    click: MouseEvent,
    #[does_not_bubble]
    close: Event,
    compositionend: CompositionEvent,
    compositionstart: CompositionEvent,
    compositionupdate: CompositionEvent,
    contextmenu: MouseEvent,
    cuechange: Event,
    dblclick: MouseEvent,
    drag: DragEvent,
    dragend: DragEvent,
    dragenter: DragEvent,
    dragleave: DragEvent,
    dragover: DragEvent,
    dragstart: DragEvent,
    drop: DragEvent,
    durationchange: Event,
    emptied: Event,
    ended: Event,
    #[does_not_bubble]
    error: ErrorEvent,
    #[does_not_bubble]
    focus: FocusEvent,
    #[does_not_bubble]
    focusin: FocusEvent,
    #[does_not_bubble]
    focusout: FocusEvent,
    formdata: Event, // web_sys does not include `FormDataEvent`
    gotpointercapture: PointerEvent,
    input: Event,
    invalid: Event,
    keydown: KeyboardEvent,
    keypress: KeyboardEvent,
    keyup: KeyboardEvent,
    #[does_not_bubble]
    load: Event,
    loadeddata: Event,
    loadedmetadata: Event,
    #[does_not_bubble]
    loadstart: Event,
    lostpointercapture: PointerEvent,
    mousedown: MouseEvent,
    mouseenter: MouseEvent,
    mouseleave: MouseEvent,
    mousemove: MouseEvent,
    mouseout: MouseEvent,
    mouseover: MouseEvent,
    mouseup: MouseEvent,
    pause: Event,
    play: Event,
    playing: Event,
    pointercancel: PointerEvent,
    pointerdown: PointerEvent,
    pointerenter: PointerEvent,
    pointerleave: PointerEvent,
    pointermove: PointerEvent,
    pointerout: PointerEvent,
    pointerover: PointerEvent,
    pointerup: PointerEvent,
    #[does_not_bubble]
    progress: ProgressEvent,
    ratechange: Event,
    reset: Event,
    resize: UiEvent,
    #[does_not_bubble]
    scroll: Event,
    securitypolicyviolation: SecurityPolicyViolationEvent,
    seeked: Event,
    seeking: Event,
    select: Event,
    selectionchange: Event,
    selectstart: Event,
    graffchange: Event,
    stalled: Event,
    submit: SubmitEvent,
    suspend: Event,
    timeupdate: Event,
    toggle: Event,
    touchcancel: TouchEvent,
    touchend: TouchEvent,
    touchmove: TouchEvent,
    touchstart: TouchEvent,
    transitioncancel: TransitionEvent,
    transitionend: TransitionEvent,
    transitionrun: TransitionEvent,
    transitionstart: TransitionEvent,
    volumechange: Event,
    waiting: Event,
    webkitanimationend: Event,
    webkitanimationiteration: Event,
    webkitanimationstart: Event,
    webkittransitionend: Event,
    wheel: WheelEvent,
  
    // =========================================================
    // WindowEventMap
    // =========================================================
    DOMContentLoaded: Event,
    devicemotion: DeviceMotionEvent,
    deviceorientation: DeviceOrientationEvent,
    orientationchange: Event,
  
    // =========================================================
    // DocumentAndElementEventHandlersEventMap
    // =========================================================
    copy: Event, // ClipboardEvent is unstable
    cut: Event, // ClipboardEvent is unstable
    paste: Event, // ClipboardEvent is unstable
  
    // =========================================================
    // DocumentEventMap
    // =========================================================
    fullscreenchange: Event,
    fullscreenerror: Event,
    pointerlockchange: Event,
    pointerlockerror: Event,
    readystatechange: Event,
    visibilitychange: Event,
  }
  