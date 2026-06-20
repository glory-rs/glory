#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
mod csr;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub use csr::*;

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
mod ssr;
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub use ssr::*;

use std::borrow::Cow;

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

          #[cfg(not(feature = "backend-command"))]
          impl EventDescriptor for $event {
            type EventType = web_sys::$web_sys_event;

            fn name(&self) -> Cow<'static, str> {
              stringify!($event).into()
            }

            $(
              generate_event_types!($does_not_bubble);
            )?
          }

          // Command-stream backends deliver every event as the serializable
          // cross-platform `EventData` payload instead of a `web_sys` type.
          #[cfg(feature = "backend-command")]
          impl EventDescriptor for $event {
            type EventType = $crate::renderer::EventData;

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
  beforetoggle: Event,
  canplay: Event,
  canplaythrough: Event,
  cancel: Event,
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
  doubleclick: MouseEvent,
  drag: DragEvent,
  dragend: DragEvent,
  dragenter: DragEvent,
  dragexit: DragEvent,
  dragleave: DragEvent,
  dragover: DragEvent,
  dragstart: DragEvent,
  drop: DragEvent,
  durationchange: Event,
  emptied: Event,
  encrypted: Event,
  ended: Event,
  #[does_not_bubble]
  error: ErrorEvent,
  #[does_not_bubble]
  focus: FocusEvent,
  focusin: FocusEvent,
  focusout: FocusEvent,
  formdata: Event, // web_sys does not include `FormDataEvent`
  gotpointercapture: PointerEvent,
  input: InputEvent,
  interruptbegin: Event,
  interruptend: Event,
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
  loadend: ProgressEvent,
  lostpointercapture: PointerEvent,
  mousedown: MouseEvent,
  #[does_not_bubble]
  mouseenter: MouseEvent,
  #[does_not_bubble]
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
  #[does_not_bubble]
  pointerenter: PointerEvent,
  #[does_not_bubble]
  pointerleave: PointerEvent,
  pointermove: PointerEvent,
  pointerout: PointerEvent,
  pointerover: PointerEvent,
  pointerrawupdate: PointerEvent,
  pointerup: PointerEvent,
  #[does_not_bubble]
  progress: ProgressEvent,
  ratechange: Event,
  reset: Event,
  resize: UiEvent,
  #[does_not_bubble]
  scroll: Event,
  scrollend: Event,
  securitypolicyviolation: SecurityPolicyViolationEvent,
  seeked: Event,
  seeking: Event,
  select: Event,
  selectionchange: Event,
  selectstart: Event,
  slotchange: Event,
  graffchange: Event,
  stalled: Event,
  submit: SubmitEvent,
  suspend: Event,
  timeout: ProgressEvent,
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
  copy: ClipboardEvent,
  cut: ClipboardEvent,
  paste: ClipboardEvent,

  // =========================================================
  // DocumentEventMap
  // =========================================================
  fullscreenchange: Event,
  fullscreenerror: Event,
  pointerlockchange: Event,
  pointerlockerror: Event,
  readystatechange: Event,
  visibilitychange: Event,

  // =========================================================
  // Glory synthetic lifecycle events
  // =========================================================
  #[does_not_bubble]
  mounted: Event,
  #[does_not_bubble]
  visible: Event,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::web::events::EventDescriptor;

    #[test]
    fn event_descriptors_match_dom_bubbling_for_delegation() {
        assert!(!focus.bubbles());
        assert!(focusin.bubbles());
        assert!(focusout.bubbles());
        assert!(!mouseenter.bubbles());
        assert!(!mouseleave.bubbles());
        assert!(!pointerenter.bubbles());
        assert!(!pointerleave.bubbles());
    }

    #[test]
    fn dioxus_coverage_gap_events_are_exposed() {
        assert_eq!(dragexit.name(), "dragexit");
        assert_eq!(doubleclick.name(), "doubleclick");
        assert_eq!(encrypted.name(), "encrypted");
        assert_eq!(loadend.name(), "loadend");
        assert_eq!(mounted.name(), "mounted");
        assert_eq!(timeout.name(), "timeout");
        assert_eq!(visible.name(), "visible");
    }
}
