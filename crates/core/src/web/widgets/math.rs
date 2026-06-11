//! MathML element builders.

use std::borrow::Cow;

use crate::generate_tags;
use crate::reflow::{Bond, Lotus};
use crate::view::ViewId;
use crate::web::events::EventDescriptor;
use crate::web::{AttrValue, ClassPart, PropValue};
use crate::widget::IntoFiller;
use crate::{Node, NodeRef, Scope, Widget};

generate_tags![
  /// The `<math>` MathML root element.
  math { tag: "math", ns: "http://www.w3.org/1998/Math/MathML" } Math Element [],
  /// The `<mi>` MathML identifier element.
  mi { tag: "mi", ns: "http://www.w3.org/1998/Math/MathML" } MathMi Element [],
  /// The `<mn>` MathML number element.
  mn { tag: "mn", ns: "http://www.w3.org/1998/Math/MathML" } MathMn Element [],
  /// The `<mo>` MathML operator element.
  mo { tag: "mo", ns: "http://www.w3.org/1998/Math/MathML" } MathMo Element [],
  /// The `<mtext>` MathML text element.
  mtext { tag: "mtext", ns: "http://www.w3.org/1998/Math/MathML" } MathText Element [],
  /// The `<mrow>` MathML row element.
  mrow { tag: "mrow", ns: "http://www.w3.org/1998/Math/MathML" } MathRow Element [],
  /// The `<mfrac>` MathML fraction element.
  mfrac { tag: "mfrac", ns: "http://www.w3.org/1998/Math/MathML" } MathFrac Element [],
  /// The `<msqrt>` MathML square-root element.
  msqrt { tag: "msqrt", ns: "http://www.w3.org/1998/Math/MathML" } MathSqrt Element [],
  /// The `<msup>` MathML superscript element.
  msup { tag: "msup", ns: "http://www.w3.org/1998/Math/MathML" } MathSup Element [],
  /// The `<msub>` MathML subscript element.
  msub { tag: "msub", ns: "http://www.w3.org/1998/Math/MathML" } MathSub Element [],
  /// The `<msubsup>` MathML subscript-superscript element.
  msubsup { tag: "msubsup", ns: "http://www.w3.org/1998/Math/MathML" } MathSubSup Element [],
  /// The `<mover>` MathML overscript element.
  mover { tag: "mover", ns: "http://www.w3.org/1998/Math/MathML" } MathOver Element [],
  /// The `<munder>` MathML underscript element.
  munder { tag: "munder", ns: "http://www.w3.org/1998/Math/MathML" } MathUnder Element [],
  /// The `<munderover>` MathML under-over element.
  munderover { tag: "munderover", ns: "http://www.w3.org/1998/Math/MathML" } MathUnderOver Element [],
  /// The `<mtable>` MathML table element.
  mtable { tag: "mtable", ns: "http://www.w3.org/1998/Math/MathML" } MathTable Element [],
  /// The `<mtr>` MathML table-row element.
  mtr { tag: "mtr", ns: "http://www.w3.org/1998/Math/MathML" } MathTableRow Element [],
  /// The `<mtd>` MathML table-cell element.
  mtd { tag: "mtd", ns: "http://www.w3.org/1998/Math/MathML" } MathTableCell Element [],
  /// The `<semantics>` MathML semantics element.
  semantics { tag: "semantics", ns: "http://www.w3.org/1998/Math/MathML" } MathSemantics Element [],
  /// The `<annotation>` MathML annotation element.
  annotation { tag: "annotation", ns: "http://www.w3.org/1998/Math/MathML" } MathAnnotation Element [],
  /// The `<annotation-xml>` MathML XML annotation element.
  annotation_xml { tag: "annotation-xml", ns: "http://www.w3.org/1998/Math/MathML" } MathAnnotationXml Element [],
  /// The `<merror>` MathML error element.
  merror { tag: "merror", ns: "http://www.w3.org/1998/Math/MathML" } MathError Element [],
  /// The `<mmultiscripts>` MathML multiscripts element.
  mmultiscripts { tag: "mmultiscripts", ns: "http://www.w3.org/1998/Math/MathML" } MathMultiscripts Element [],
  /// The `<mpadded>` MathML padding element.
  mpadded { tag: "mpadded", ns: "http://www.w3.org/1998/Math/MathML" } MathPadded Element [],
  /// The `<mphantom>` MathML phantom element.
  mphantom { tag: "mphantom", ns: "http://www.w3.org/1998/Math/MathML" } MathPhantom Element [],
  /// The `<mprescripts>` MathML prescripts element.
  mprescripts { tag: "mprescripts", ns: "http://www.w3.org/1998/Math/MathML" } MathPrescripts Element [],
  /// The `<mroot>` MathML root element.
  mroot { tag: "mroot", ns: "http://www.w3.org/1998/Math/MathML" } MathRoot Element [],
  /// The `<ms>` MathML string-literal element.
  ms { tag: "ms", ns: "http://www.w3.org/1998/Math/MathML" } MathString Element [],
  /// The `<mspace>` MathML space element.
  mspace { tag: "mspace", ns: "http://www.w3.org/1998/Math/MathML" } MathSpace Element [],
  /// The `<mstyle>` MathML style element.
  mstyle { tag: "mstyle", ns: "http://www.w3.org/1998/Math/MathML" } MathStyle Element [],
];
