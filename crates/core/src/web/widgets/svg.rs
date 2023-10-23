//! SVG elements.

use super::{ElementDescriptor, HtmlElement};
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
use super::{HydrationKey, HTML_ELEMENT_DEREF_UNIMPLEMENTED_MSG};
use crate::HydrationCtx;
use glory_reactive::Scope;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use once_cell::unsync::Lazy as LazyCell;
use std::borrow::Cow;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
use wasm_bindgen::JsCast;

generate_tags![
  /// SVG Element.
  a,
  /// SVG Element.
  animate,
  /// SVG Element.
  animateMotion,
  /// SVG Element.
  animateTransform,
  /// SVG Element.
  circle,
  /// SVG Element.
  clipPath,
  /// SVG Element.
  defs,
  /// SVG Element.
  desc,
  /// SVG Element.
  discard,
  /// SVG Element.
  ellipse,
  /// SVG Element.
  feBlend,
  /// SVG Element.
  feColorMatrix,
  /// SVG Element.
  feComponentTransfer,
  /// SVG Element.
  feComposite,
  /// SVG Element.
  feConvolveMatrix,
  /// SVG Element.
  feDiffuseLighting,
  /// SVG Element.
  feDisplacementMap,
  /// SVG Element.
  feDistantLight,
  /// SVG Element.
  feDropShadow,
  /// SVG Element.
  feFlood,
  /// SVG Element.
  feFuncA,
  /// SVG Element.
  feFuncB,
  /// SVG Element.
  feFuncG,
  /// SVG Element.
  feFuncR,
  /// SVG Element.
  feGaussianBlur,
  /// SVG Element.
  feImage,
  /// SVG Element.
  feMerge,
  /// SVG Element.
  feMergeNode,
  /// SVG Element.
  feMorphology,
  /// SVG Element.
  feOffset,
  /// SVG Element.
  fePointLight,
  /// SVG Element.
  feSpecularLighting,
  /// SVG Element.
  feSpotLight,
  /// SVG Element.
  feTile,
  /// SVG Element.
  feTurbulence,
  /// SVG Element.
  filter,
  /// SVG Element.
  foreignObject,
  /// SVG Element.
  g,
  /// SVG Element.
  hatch,
  /// SVG Element.
  hatchpath,
  /// SVG Element.
  image,
  /// SVG Element.
  line,
  /// SVG Element.
  linearGradient,
  /// SVG Element.
  marker,
  /// SVG Element.
  mask,
  /// SVG Element.
  metadata,
  /// SVG Element.
  mpath,
  /// SVG Element.
  path,
  /// SVG Element.
  pattern,
  /// SVG Element.
  polygon,
  /// SVG Element.
  polyline,
  /// SVG Element.
  radialGradient,
  /// SVG Element.
  rect,
  /// SVG Element.
  script,
  /// SVG Element.
  set,
  /// SVG Element.
  stop,
  /// SVG Element.
  style,
  /// SVG Element.
  svg,
  /// SVG Element.
  switch,
  /// SVG Element.
  symbol,
  /// SVG Element.
  text,
  /// SVG Element.
  textPath,
  /// SVG Element.
  title,
  /// SVG Element.
  tspan,
  /// SVG Element.
  use @_,
  /// SVG Element.
  view,
];
