//! SVG element builders.
//!
//! This module exposes the SVG surface through the same builder API as HTML
//! elements. Camel-case SVG tags use snake-case Rust functions that map to the
//! correct DOM tag name.

use std::borrow::Cow;

use crate::generate_tags;
use crate::reflow::{Bond, Lotus};
use crate::view::ViewId;
use crate::web::events::EventDescriptor;
use crate::web::{AttrValue, ClassPart, PropValue};
use crate::widget::IntoFiller;
use crate::{Node, NodeRef, Scope, Widget};

generate_tags![
  /// The `<svg>` SVG container element.
  svg { tag: "svg", ns: "http://www.w3.org/2000/svg" } Svg SvgElement [],
  /// The `<animate>` SVG animation element.
  animate { tag: "animate", ns: "http://www.w3.org/2000/svg" } SvgAnimate SvgElement [],
  /// The `<animateMotion>` SVG animation element.
  animate_motion { tag: "animateMotion", ns: "http://www.w3.org/2000/svg" } SvgAnimateMotion SvgElement [],
  /// The `<animateTransform>` SVG animation element.
  animate_transform { tag: "animateTransform", ns: "http://www.w3.org/2000/svg" } SvgAnimateTransform SvgElement [],
  /// The `<g>` SVG grouping element.
  g { tag: "g", ns: "http://www.w3.org/2000/svg" } SvgG SvgElement [],
  /// The `<defs>` SVG reusable-definition container.
  defs { tag: "defs", ns: "http://www.w3.org/2000/svg" } SvgDefs SvgElement [],
  /// The `<desc>` SVG description element.
  desc { tag: "desc", ns: "http://www.w3.org/2000/svg" } SvgDesc SvgElement [],
  /// The `<title>` SVG accessible-title element.
  title { tag: "title", ns: "http://www.w3.org/2000/svg" } SvgTitle SvgElement [],
  /// The `<symbol>` SVG reusable symbol element.
  symbol { tag: "symbol", ns: "http://www.w3.org/2000/svg" } SvgSymbol SvgElement [],
  /// The `<path>` SVG path element.
  path { tag: "path", ns: "http://www.w3.org/2000/svg" } SvgPath SvgElement [],
  /// The `<rect>` SVG rectangle element.
  rect { tag: "rect", ns: "http://www.w3.org/2000/svg" } SvgRect SvgElement [],
  /// The `<circle>` SVG circle element.
  circle { tag: "circle", ns: "http://www.w3.org/2000/svg" } SvgCircle SvgElement [],
  /// The `<clipPath>` SVG clipping-path element.
  clip_path { tag: "clipPath", ns: "http://www.w3.org/2000/svg" } SvgClipPath SvgElement [],
  /// The `<ellipse>` SVG ellipse element.
  ellipse { tag: "ellipse", ns: "http://www.w3.org/2000/svg" } SvgEllipse SvgElement [],
  /// The `<line>` SVG line element.
  line { tag: "line", ns: "http://www.w3.org/2000/svg" } SvgLine SvgElement [],
  /// The `<polyline>` SVG polyline element.
  polyline { tag: "polyline", ns: "http://www.w3.org/2000/svg" } SvgPolyline SvgElement [],
  /// The `<polygon>` SVG polygon element.
  polygon { tag: "polygon", ns: "http://www.w3.org/2000/svg" } SvgPolygon SvgElement [],
  /// The `<text>` SVG text element.
  text { tag: "text", ns: "http://www.w3.org/2000/svg" } SvgText SvgElement [],
  /// The `<textPath>` SVG text-path element.
  text_path { tag: "textPath", ns: "http://www.w3.org/2000/svg" } SvgTextPath SvgElement [],
  /// The `<tspan>` SVG text-span element.
  tspan { tag: "tspan", ns: "http://www.w3.org/2000/svg" } SvgTspan SvgElement [],
  /// The `<stop>` SVG gradient stop element.
  stop { tag: "stop", ns: "http://www.w3.org/2000/svg" } SvgStop SvgElement [],
  /// The `<mask>` SVG mask element.
  mask { tag: "mask", ns: "http://www.w3.org/2000/svg" } SvgMask SvgElement [],
  /// The `<pattern>` SVG pattern element.
  pattern { tag: "pattern", ns: "http://www.w3.org/2000/svg" } SvgPattern SvgElement [],
  /// The `<image>` SVG image element.
  image { tag: "image", ns: "http://www.w3.org/2000/svg" } SvgImage SvgElement [],
  /// The `<discard>` SVG animation discard element.
  discard { tag: "discard", ns: "http://www.w3.org/2000/svg" } SvgDiscard SvgElement [],
  /// The `<filter>` SVG filter element.
  filter { tag: "filter", ns: "http://www.w3.org/2000/svg" } SvgFilter SvgElement [],
  /// The `<foreignObject>` SVG foreign-object element.
  foreign_object { tag: "foreignObject", ns: "http://www.w3.org/2000/svg" } SvgForeignObject SvgElement [],
  /// The `<hatch>` SVG hatch element.
  hatch { tag: "hatch", ns: "http://www.w3.org/2000/svg" } SvgHatch SvgElement [],
  /// The `<hatchpath>` SVG hatch-path element.
  hatchpath { tag: "hatchpath", ns: "http://www.w3.org/2000/svg" } SvgHatchPath SvgElement [],
  /// The `<linearGradient>` SVG gradient element.
  linear_gradient { tag: "linearGradient", ns: "http://www.w3.org/2000/svg" } SvgLinearGradient SvgElement [],
  /// The `<marker>` SVG marker element.
  marker { tag: "marker", ns: "http://www.w3.org/2000/svg" } SvgMarker SvgElement [],
  /// The `<metadata>` SVG metadata element.
  metadata { tag: "metadata", ns: "http://www.w3.org/2000/svg" } SvgMetadata SvgElement [],
  /// The `<mpath>` SVG motion-path element.
  mpath { tag: "mpath", ns: "http://www.w3.org/2000/svg" } SvgMPath SvgElement [],
  /// The `<radialGradient>` SVG gradient element.
  radial_gradient { tag: "radialGradient", ns: "http://www.w3.org/2000/svg" } SvgRadialGradient SvgElement [],
  /// The `<set>` SVG set-animation element.
  set { tag: "set", ns: "http://www.w3.org/2000/svg" } SvgSet SvgElement [],
  /// The `<switch>` SVG conditional-processing element.
  switch_ { tag: "switch", ns: "http://www.w3.org/2000/svg" } SvgSwitch SvgElement [],
  /// The `<use>` SVG reuse element.
  use_ { tag: "use", ns: "http://www.w3.org/2000/svg" } SvgUse SvgElement [],
  /// The `<view>` SVG view element.
  view { tag: "view", ns: "http://www.w3.org/2000/svg" } SvgView SvgElement [],
  /// The `<feBlend>` SVG filter primitive.
  fe_blend { tag: "feBlend", ns: "http://www.w3.org/2000/svg" } SvgFeBlend SvgElement [],
  /// The `<feColorMatrix>` SVG filter primitive.
  fe_color_matrix { tag: "feColorMatrix", ns: "http://www.w3.org/2000/svg" } SvgFeColorMatrix SvgElement [],
  /// The `<feComponentTransfer>` SVG filter primitive.
  fe_component_transfer { tag: "feComponentTransfer", ns: "http://www.w3.org/2000/svg" } SvgFeComponentTransfer SvgElement [],
  /// The `<feComposite>` SVG filter primitive.
  fe_composite { tag: "feComposite", ns: "http://www.w3.org/2000/svg" } SvgFeComposite SvgElement [],
  /// The `<feConvolveMatrix>` SVG filter primitive.
  fe_convolve_matrix { tag: "feConvolveMatrix", ns: "http://www.w3.org/2000/svg" } SvgFeConvolveMatrix SvgElement [],
  /// The `<feDiffuseLighting>` SVG filter primitive.
  fe_diffuse_lighting { tag: "feDiffuseLighting", ns: "http://www.w3.org/2000/svg" } SvgFeDiffuseLighting SvgElement [],
  /// The `<feDisplacementMap>` SVG filter primitive.
  fe_displacement_map { tag: "feDisplacementMap", ns: "http://www.w3.org/2000/svg" } SvgFeDisplacementMap SvgElement [],
  /// The `<feDistantLight>` SVG light-source element.
  fe_distant_light { tag: "feDistantLight", ns: "http://www.w3.org/2000/svg" } SvgFeDistantLight SvgElement [],
  /// The `<feDropShadow>` SVG filter primitive.
  fe_drop_shadow { tag: "feDropShadow", ns: "http://www.w3.org/2000/svg" } SvgFeDropShadow SvgElement [],
  /// The `<feFlood>` SVG filter primitive.
  fe_flood { tag: "feFlood", ns: "http://www.w3.org/2000/svg" } SvgFeFlood SvgElement [],
  /// The `<feFuncA>` SVG transfer-function element.
  fe_func_a { tag: "feFuncA", ns: "http://www.w3.org/2000/svg" } SvgFeFuncA SvgElement [],
  /// The `<feFuncB>` SVG transfer-function element.
  fe_func_b { tag: "feFuncB", ns: "http://www.w3.org/2000/svg" } SvgFeFuncB SvgElement [],
  /// The `<feFuncG>` SVG transfer-function element.
  fe_func_g { tag: "feFuncG", ns: "http://www.w3.org/2000/svg" } SvgFeFuncG SvgElement [],
  /// The `<feFuncR>` SVG transfer-function element.
  fe_func_r { tag: "feFuncR", ns: "http://www.w3.org/2000/svg" } SvgFeFuncR SvgElement [],
  /// The `<feGaussianBlur>` SVG filter primitive.
  fe_gaussian_blur { tag: "feGaussianBlur", ns: "http://www.w3.org/2000/svg" } SvgFeGaussianBlur SvgElement [],
  /// The `<feImage>` SVG filter primitive.
  fe_image { tag: "feImage", ns: "http://www.w3.org/2000/svg" } SvgFeImage SvgElement [],
  /// The `<feMerge>` SVG filter primitive.
  fe_merge { tag: "feMerge", ns: "http://www.w3.org/2000/svg" } SvgFeMerge SvgElement [],
  /// The `<feMergeNode>` SVG merge-node element.
  fe_merge_node { tag: "feMergeNode", ns: "http://www.w3.org/2000/svg" } SvgFeMergeNode SvgElement [],
  /// The `<feMorphology>` SVG filter primitive.
  fe_morphology { tag: "feMorphology", ns: "http://www.w3.org/2000/svg" } SvgFeMorphology SvgElement [],
  /// The `<feOffset>` SVG filter primitive.
  fe_offset { tag: "feOffset", ns: "http://www.w3.org/2000/svg" } SvgFeOffset SvgElement [],
  /// The `<fePointLight>` SVG light-source element.
  fe_point_light { tag: "fePointLight", ns: "http://www.w3.org/2000/svg" } SvgFePointLight SvgElement [],
  /// The `<feSpecularLighting>` SVG filter primitive.
  fe_specular_lighting { tag: "feSpecularLighting", ns: "http://www.w3.org/2000/svg" } SvgFeSpecularLighting SvgElement [],
  /// The `<feSpotLight>` SVG light-source element.
  fe_spot_light { tag: "feSpotLight", ns: "http://www.w3.org/2000/svg" } SvgFeSpotLight SvgElement [],
  /// The `<feTile>` SVG filter primitive.
  fe_tile { tag: "feTile", ns: "http://www.w3.org/2000/svg" } SvgFeTile SvgElement [],
  /// The `<feTurbulence>` SVG filter primitive.
  fe_turbulence { tag: "feTurbulence", ns: "http://www.w3.org/2000/svg" } SvgFeTurbulence SvgElement [],
];
