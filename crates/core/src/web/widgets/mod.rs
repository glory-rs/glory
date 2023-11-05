#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
mod csr;
#[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
pub use csr::*;

#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
mod ssr;
#[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
pub use ssr::*;

mod head_mixin;
pub use head_mixin::*;
mod node_meta;
pub use node_meta::*;

use std::borrow::Cow;
use std::fmt;

use crate::reflow::{Bond, Lotus};
use crate::view::ViewId;
use crate::web::events::EventDescriptor;
use crate::web::{AttrValue, ClassPart, PropValue};
use crate::widget::{Filler, IntoFiller};
use crate::{Cage, Node, NodeRef, Scope, Widget};

#[macro_export]
macro_rules! generate_tags {
    ($(
        #[$meta:meta]
        $tag:ident $name:ident $inner:ident [$($prop_fn:ident $($prop_key:expr)?),*] $($void:ident)?
    ),* $(,)?) => {
        paste::paste! {
            $(
                #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
                thread_local! {
                  static [<ELE_ $tag:upper>]: once_cell::unsync::Lazy<web_sys::$inner> = once_cell::unsync::Lazy::new(|| {
                    wasm_bindgen::JsCast::unchecked_into::<web_sys::$inner>($crate::web::document()
                      .create_element(stringify!($tag))
                      .unwrap())
                  });
                }
                #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
                #[derive(Debug)]
                pub struct $name(pub $crate::web::widgets::Element<web_sys::$inner>);
                #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
                #[derive(Debug)]
                pub struct $name(pub $crate::web::widgets::Element);

                #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
                impl AsRef<$crate::web::widgets::Element<web_sys::$inner>> for $name {
                    fn as_ref(&self) -> &$crate::web::widgets::Element<web_sys::$inner> {
                        &self.0
                    }
                }
                #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
                impl AsRef<$crate::web::widgets::Element> for $name {
                    fn as_ref(&self) -> &$crate::web::widgets::Element {
                        &self.0
                    }
                }
                #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
                impl std::ops::Deref for $name {
                    type Target = $crate::web::widgets::Element<web_sys::$inner>;
                    fn deref(&self) -> &Self::Target {
                        &self.0
                    }
                }
                #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
                impl std::ops::Deref for $name {
                    type Target = $crate::web::widgets::Element;
                    fn deref(&self) -> &Self::Target {
                        &self.0
                    }
                }
                impl std::ops::DerefMut for $name {
                    fn deref_mut(&mut self) -> &mut Self::Target {
                        &mut self.0
                    }
                }

                impl $name {
                    $(
                        #[doc=concat!("A shortcut to set property `", generate_tags!{ @prop_key $prop_fn, $($prop_key)? }, "`.")]
                        pub fn $prop_fn(mut self, value: impl $crate::web::PropValue + 'static) -> Self {
                            self.0.props.insert(generate_tags!{ @prop_key $prop_fn, $($prop_key)? }.into(), Box::new(value));
                            self
                        }
                    )*

                    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
                    pub fn new() -> $name {
                        let inner = [<ELE_ $tag:upper>].with(|el| wasm_bindgen::JsCast::unchecked_into(el.clone_node().unwrap()));
                        $name($crate::web::widgets::Element::new(stringify!($tag), generate_tags!{ @void $($void)? }, inner))
                    }
                    #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
                    pub fn new() -> $name {
                        $name($crate::web::widgets::Element::new(stringify!($tag), generate_tags!{ @void $($void)? }))
                    }

                    pub fn fill(mut self, filler: impl IntoFiller) -> Self {
                        self.0.add_filler(filler);
                        self
                    }

                    pub fn then<F>(self, func: F) -> Self
                    where
                        F: FnOnce(Self) -> Self,
                    {
                        func(self)
                    }

                    #[track_caller]
                    pub fn id<V>(mut self, value: V) -> Self
                    where
                        V: AttrValue + 'static,
                    {
                        self.0.add_id(value);
                        self
                    }

                    #[track_caller]
                    pub fn class<V>(mut self, value: V) -> Self
                    where
                        V: ClassPart + 'static,
                    {
                        self.0.add_class(value);
                        self
                    }

                    #[track_caller]
                    pub fn toggle_class<V, C>(self, value: V, cond: C) -> Self
                    where
                        V: Into<String>,
                        C: Lotus<bool> + Clone + 'static,
                    {
                        self.switch_class(value, "", cond)
                    }

                    #[track_caller]
                    pub fn switch_class<TV, FV, C>(mut self, tv: TV, fv: FV, cond: C) -> Self
                    where
                        TV: Into<String>,
                        FV: Into<String>,
                        C: Lotus<bool> + Clone + 'static,
                    {
                        let tv = tv.into();
                        let fv = fv.into();
                        self.0.classes.part(Bond::new(move || if *cond.get() { tv.clone() } else { fv.clone() }));
                        self
                    }

                    /// Adds an property to this element.
                    #[track_caller]
                    pub fn prop<V>(mut self, name: impl Into<Cow<'static, str>>, value: V) -> Self
                    where
                        V: PropValue + 'static,
                    {
                        self.0.add_prop(name, value);
                        self
                    }

                    /// Adds an attribute to this element.
                    #[track_caller]
                    pub fn attr<V>(mut self, name: impl Into<Cow<'static, str>>, value: V) -> Self
                    where
                        V: AttrValue + 'static,
                    {
                        self.0.add_attr(name, value);
                        self
                    }

                    /// Adds an event listener to this element.
                    #[track_caller]
                    pub fn on<E: EventDescriptor>(
                        self,
                        event: E,
                        #[allow(unused_mut)] // used for tracing in debug
                        mut event_handler: impl FnMut(E::EventType) + 'static,
                    ) -> Self {
                        self.0.add_event_listener(event, event_handler);
                        self
                    }
                    pub fn inner_text<V>(mut self, text: V) -> Self
                    where
                        V: AttrValue + 'static,
                    {
                        self.0.set_inner_text(text);
                        self
                    }
                    /// Sets the inner HTML of this element from the provided
                    /// string slice.
                    ///
                    /// # Security
                    /// Be very careful when using this method. Always remember to
                    /// sanitize the input to avoid a cross-site scripting (XSS)
                    /// vulnerability.
                    pub fn html<V>(mut self, html: V) -> Self
                    where
                        V: AttrValue + 'static,
                    {
                        self.0.set_html(html);
                        self
                    }
                    #[cfg(all(target_arch = "wasm32", feature = "web-csr"))]
                    pub fn node_ref(self, node_ref: &NodeRef<web_sys::$inner>) -> Self {
                        node_ref.set(self.0.node.clone());
                        self
                    }
                    #[cfg(not(all(target_arch = "wasm32", feature = "web-csr")))]
                    pub fn node_ref(self, node_ref: &NodeRef<Node>) -> Self {
                        node_ref.set(self.0.node.clone());
                        self
                    }
                }

                impl Widget for $name {
                    fn store_in(self, parent: &mut Scope) -> ViewId
                    where
                        Self: Sized,
                    {
                        self.0.store_in(parent)
                    }
                    fn show_in(self, parent: &mut Scope) -> ViewId
                    where
                        Self: Sized,
                    {
                        self.0.show_in(parent)
                    }

                    fn mount_to(self, ctx: Scope, parent_node: &Node) -> ViewId
                    where
                        Self: Sized,
                    {
                        self.0.mount_to(ctx, parent_node)
                    }

                    fn attach(&mut self, ctx: &mut Scope) {
                        self.0.attach(ctx)
                    }
                    fn build(&mut self, ctx: &mut Scope) {
                        self.0.build(ctx)
                    }
                    fn flood(&mut self, ctx: &mut Scope) {
                        self.0.build(ctx)
                    }
                    fn patch(&mut self, ctx: &mut Scope) {
                        self.0.patch(ctx)
                    }

                    fn detach(&mut self, ctx: &mut Scope) {
                        self.0.detach(ctx);
                    }
                }

                #[$meta]
                pub fn $tag() -> $name {
                    $name::new()
                }
             )*
        }
    };
    (@void) => {false};
    (@void void) => {true};
    (@prop_key $fname:ident$(,)?) => {stringify!($fname)};
    (@prop_key $fname:ident, $pkey:expr) => {$pkey};
}

generate_tags![
  // ==========================
  //        Main root
  // ==========================
  /// The `<html>` HTML element represents the root (top-level element) of an HTML document, so it is also referred to as the root element. All other elements must be descendants of this element.
  html Html HtmlHtmlElement [],

  // ==========================
  //     Document Metadata
  // ==========================
  /// The `<base>` HTML element specifies the base URL to use for all relative URLs in a document. There can be only one `<base>` element in a document.
  base Base HtmlBaseElement [href, target],

  /// The `<head>` HTML element contains machine-readable information (metadata) about the document, like its title, scripts, and style sheets.
  head Head HtmlHeadElement [],

  /// The `<link>` HTML element specifies relationships between the current document and an external resource. This element is most commonly used to link to CSS, but is also used to establish site icons (both "favicon" style icons and icons for the home screen and apps on mobile devices) among other things.
  link Link HtmlLinkElement [as_ "as", cross_origin "crossOrigin", disabled, fetch_priority "fetchPriority", href, href_lang "hreflang", media, referrer_policy "referrerPolicy", rel, type_ "type"] void,

  /// The `<meta>` HTML element represents Metadata that cannot be represented by other HTML meta-related elements, like base, link, script, style or title.
  meta Meta HtmlMetaElement [charset, content, http_equiv "httpEquiv", media, name] void,

  /// The `<style>` HTML element contains style information for a document, or part of a document. It contains CSS, which is applied to the contents of the document containing the `<style>` element.
  style Style HtmlStyleElement [media, disabled],

  /// The `<title>` HTML element defines the document's title that is shown in a Browser's title bar or a page's tab. It only contains text; tags within the element are ignored.
  title Title HtmlTitleElement [text],

  // ==========================
  //     Sectioning Root
  // ==========================
  /// The `<body>` HTML element represents the content of an HTML document. There can be only one `<body>` element in a document.
  body Body HtmlBodyElement [],

  // ==========================
  //     Content Sectioning
  // ==========================
  /// The `<address>` HTML element indicates that the enclosed HTML provides contact information for a person or people, or for an organization.
  address Address HtmlElement [],

  /// The `<article>` HTML element represents a self-contained composition in a document, page, application, or site, which is intended to be independently distributable or reusable (e.g., in syndication). Examples include: a forum post, a magazine or newspaper article, or a blog entry, a product card, a user-submitted comment, an interactive widget or gadget, or any other independent item of content.
  article Article HtmlElement [],

  /// The `<aside>` HTML element represents a portion of a document whose content is only indirectly related to the document's main content. Asides are frequently presented as sidebars or call-out boxes.
  aside Aside HtmlElement [],

  /// The `<footer>` HTML element represents a footer for its nearest sectioning content or sectioning root element. A `<footer>` typically contains information about the author of the section, copyright data or links to related documents.
  footer Footer HtmlElement [],

  /// The `<header>` HTML element represents introductory content, typically a group of introductory or navigational aids. It may contain some heading elements but also a logo, a search form, an author name, and other elements.
  header Header HtmlElement [],

  /// The `<hgroup>` HTML element represents a heading and related content. It groups a single `<h1>–<h6>` element with one or more `<p>`.
  hgroup HGroup HtmlElement [],

  /// The `<h1>` to `<h6>` HTML elements represent six levels of section headings. `<h1>` is the highest section level and `<h6>` is the lowest.
  h1 H1 HtmlHeadingElement [],

  /// The `<h1>` to `<h6>` HTML elements represent six levels of section headings. `<h1>` is the highest section level and `<h6>` is the lowest.
  h2 H2 HtmlHeadingElement [],

  /// The `<h1>` to `<h6>` HTML elements represent six levels of section headings. `<h1>` is the highest section level and `<h6>` is the lowest.
  h3 H3 HtmlHeadingElement [],

  /// The `<h1>` to `<h6>` HTML elements represent six levels of section headings. `<h1>` is the highest section level and `<h6>` is the lowest.
  h4 H4 HtmlHeadingElement [],

  /// The `<h1>` to `<h6>` HTML elements represent six levels of section headings. `<h1>` is the highest section level and `<h6>` is the lowest.
  h5 H5 HtmlHeadingElement [],

  /// The `<h1>` to `<h6>` HTML elements represent six levels of section headings. `<h1>` is the highest section level and `<h6>` is the lowest.
  h6 H6 HtmlHeadingElement [],

  /// The `<main>` HTML element represents the dominant content of the body of a document. The main content area consists of content that is directly related to or expands upon the central topic of a document, or the central functionality of an application.
  main Main HtmlElement [],

  /// The `<nav>` HTML element represents a section of a page whose purpose is to provide navigation links, either within the current document or to other documents. Common examples of navigation sections are menus, tables of contents, and indexes.
  nav Nav HtmlElement [],

  /// The `<section>` HTML element represents a generic standalone section of a document, which doesn't have a more specific semantic element to represent it. Sections should always have a heading, with very few exceptions.
  section Section HtmlElement [],

  // ==========================
  //      Text Content
  // ==========================
  /// The `<blockquote>` HTML element indicates that the enclosed text is an extended quotation. Usually, this is rendered visually by indentation (see Notes for how to change it). A URL for the source of the quotation may be given using the cite attribute, while a text representation of the source can be given using the cite element.
  blockquote BlockQuote HtmlQuoteElement [cite],

  /// The `<dd>` HTML element provides the description, definition, or value for the preceding term (dt) in a description list (dl).
  dd Dd HtmlElement [],

  /// The `<div>` HTML element is the generic container for flow content. It has no effect on the content or layout until styled in some way using CSS (e.g. styling is directly applied to it, or some kind of layout model like Flexbox is applied to its parent element).
  div Div HtmlDivElement [],

  /// The `<dl>` HTML element represents a description list. The element encloses a list of groups of terms (specified using the dt element) and descriptions (provided by dd elements). Common uses for this element are to implement a glossary or to display metadata (a list of key-value pairs).
  dl Dl HtmlDListElement [],

  /// The `<dt>` HTML element specifies a term in a description or definition list, and as such must be used inside a dl element. It is usually followed by a dd element; however, multiple `<dt>` elements in a row indicate several terms that are all defined by the immediate next dd element.
  dt Dt HtmlElement [],

  /// The `<figcaption>` HTML element represents a caption or legend describing the rest of the contents of its parent figure element.
  figcaption FigCaption HtmlElement [],

  /// The `<figure>` HTML element represents self-contained content, potentially with an optional caption, which is specified using the figcaption element. The figure, its caption, and its contents are referenced as a single unit.
  figure Figure HtmlElement [],

  /// The `<hr>` HTML element represents a thematic break between paragraph-level elements: for example, a change of scene in a story, or a shift of topic within a section.
  hr Hr HtmlHrElement [] void,

  /// The `<li>` HTML element is used to represent an item in a list. It must be contained in a parent element: an ordered list (ol), an unordered list (ul), or a menu (menu). In menus and unordered lists, list items are usually displayed using bullet points. In ordered lists, they are usually displayed with an ascending counter on the left, such as a number or letter.
  li Li HtmlLiElement [value],

  /// The `<ol>` HTML element represents an ordered list of items — typically rendered as a numbered list.
  ol Ol HtmlOListElement [reversed, start, type_ "type"],

  /// The `<p>` HTML element represents a paragraph. Paragraphs are usually represented in visual media as blocks of text separated from adjacent blocks by blank lines and/or first-line indentation, but HTML paragraphs can be any structural grouping of related content, such as images or form fields.
  p P HtmlParagraphElement [],

  /// The `<pre>` HTML element represents preformatted text which is to be presented exactly as written in the HTML file. The text is typically rendered using a non-proportional, or "monospaced, font. Whitespace inside this element is displayed as written.
  pre Pre HtmlPreElement [],

  /// The `<ul>` HTML element represents an unordered list of items, typically rendered as a bulleted list.
  ul Ul HtmlUListElement [],

  // ==========================
  //    Inline Text Semantics
  // ==========================
  /// The `<a>` HTML element (or anchor element), with its href attribute, creates a hyperlink to web pages, files, email addresses, locations in the same page, or anything else a URL can address.
  a A HtmlAnchorElement [access_key "accessKey", download, hash, host, host_name "hostname", href, href_lang "hreflang", password, path_name "pathname", port, protocol, referrer_policy "referrerPolicy", rel, search, tab_index "tabIndex", target, text, type_ "type", user_name "username"],

  /// The `<abbr>` HTML element represents an abbreviation or acronym; the optional title attribute can provide an expansion or description for the abbreviation. If present, title must contain this full description and nothing else.
  abbr Abbr HtmlElement [],

  /// The `<b>` HTML element is used to draw the reader's attention to the element's contents, which are not otherwise granted special importance. This was formerly known as the Boldface element, and most browsers still draw the text in boldface. However, you should not use `<b>` for styling text; instead, you should use the CSS font-weight property to create boldface text, or the strong element to indicate that text is of special importance.
  b B HtmlElement [],

  /// The `<bdi>` HTML element tells the browser's bidirectional algorithm to treat the text it contains in isolation from its surrounding text. It's particularly useful when a website dynamically inserts some text and doesn't know the directionality of the text being inserted.
  bdi Bdi HtmlElement [],

  /// The `<bdo>` HTML element overrides the current directionality of text, so that the text within is rendered in a different direction.
  bdo Bdo HtmlElement [],

  /// The `<br>` HTML element produces a line break in text (carriage-return). It is useful for writing a poem or an address, where the division of lines is significant.
  br Br HtmlBrElement [] void,

  /// The `<cite>` HTML element is used to describe a reference to a cited creative work, and must include the title of that work. The reference may be in an abbreviated form according to context-appropriate conventions related to citation metadata.
  cite Cite HtmlElement [],

  /// The `<code>` HTML element displays its contents styled in a fashion intended to indicate that the text is a short fragment of computer code. By default, the content text is displayed using the user agent default monospace font.
  code Code HtmlElement [],

  /// The `<data>` HTML element links a given piece of content with a machine-readable translation. If the content is time- or date-related, the time element must be used.
  data Data HtmlDataElement [value],

  /// The `<dfn>` HTML element is used to indicate the term being defined within the context of a definition phrase or sentence. The p element, the dt/dd pairing, or the section element which is the nearest ancestor of the `<dfn>` is considered to be the definition of the term.
  dfn Dfn HtmlElement [],

  /// The `<em>` HTML element marks text that has stress emphasis. The `<em>` element can be nested, with each level of nesting indicating a greater degree of emphasis.
  em Em HtmlElement [],

  /// The `<i>` HTML element represents a range of text that is set off from the normal text for some reason, such as idiomatic text, technical terms, taxonomical designations, among others. Historically, these have been presented using italicized type, which is the original source of the `<i>` naming of this element.
  i I HtmlElement [],

  /// The `<kbd>` HTML element represents a span of inline text denoting textual user input from a keyboard, voice input, or any other text entry device. By convention, the user agent defaults to rendering the contents of a `<kbd>` element using its default monospace font, although this is not mandated by the HTML standard.
  kbd Kbd HtmlElement [],

  /// The `<mark>` HTML element represents text which is marked or highlighted for reference or notation purposes, due to the marked passage's relevance or importance in the enclosing context.
  mark Mark HtmlElement [],

  /// The `<q>` HTML element indicates that the enclosed text is a short inline quotation. Most modern browsers implement this by surrounding the text in quotation marks. This element is intended for short quotations that don't require paragraph breaks; for long quotations use the blockquote element.
  q Q HtmlQuoteElement [cite],

  /// The `<rp>` HTML element is used to provide fall-back parentheses for browsers that do not support display of ruby annotations using the ruby element. One `<rp>` element should enclose each of the opening and closing parentheses that wrap the rt element that contains the annotation's text.
  rp Rp HtmlElement [],

  /// The `<rt>` HTML element specifies the ruby text component of a ruby annotation, which is used to provide pronunciation, translation, or transliteration information for East Asian typography. The `<rt>` element must always be contained within a ruby element.
  rt Rt HtmlElement [],

  /// The `<ruby>` HTML element represents small annotations that are rendered above, below, or next to base text, usually used for showing the pronunciation of East Asian characters. It can also be used for annotating other kinds of text, but this usage is less common.
  ruby Ruby HtmlElement [],

  /// The `<s>` HTML element renders text with a strikethrough, or a line through it. Use the `<s>` element to represent things that are no longer relevant or no longer accurate. However, `<s>` is not appropriate when indicating document edits; for that, use the del and ins elements, as appropriate.
  s S HtmlElement [],

  /// The `<samp>` HTML element is used to enclose inline text which represents sample (or quoted) output from a computer program. Its contents are typically rendered using the browser's default monospaced font (such as Courier or Lucida Console).
  samp Samp HtmlElement [],

  /// The `<small>` HTML element represents side-comments and small print, like copyright and legal text, independent of its styled presentation. By default, it renders text within it one font-size smaller, such as from small to x-small.
  small Small HtmlElement [],

  /// The `<span>` HTML element is a generic inline container for phrasing content, which does not inherently represent anything. It can be used to group elements for styling purposes (using the class or id attributes), or because they share attribute values, such as lang. It should be used only when no other semantic element is appropriate. `<span>` is very much like a div element, but div is a block-level element whereas a `<span>` is an inline element.
  span Span HtmlSpanElement [],

  /// The `<strong>` HTML element indicates that its contents have strong importance, seriousness, or urgency. Browsers typically render the contents in bold type.
  strong Strong HtmlElement [],

  /// The `<sub>` HTML element specifies inline text which should be displayed as subscript for solely typographical reasons. Subscripts are typically rendered with a lowered baseline using smaller text.
  sub Sub HtmlElement [],

  /// The `<sup>` HTML element specifies inline text which is to be displayed as superscript for solely typographical reasons. Superscripts are usually rendered with a raised baseline using smaller text.
  sup Sup HtmlElement [],

  /// The `<time>` HTML element represents a specific period in time. It may include the datetime attribute to translate dates into machine-readable format, allowing for better search engine results or custom features such as reminders.
  time Time HtmlTimeElement [date_time "dateTime"],

  /// The `<u>` HTML element represents a span of inline text which should be rendered in a way that indicates that it has a non-textual annotation. This is rendered by default as a simple solid underline, but may be altered using CSS.
  u U HtmlElement [],

  /// The `<var>` HTML element represents the name of a variable in a mathematical expression or a programming context. It's typically presented using an italicized version of the current typeface, although that behavior is browser-dependent.
  var Var HtmlElement [],

  /// The `<wbr>` HTML element represents a word break opportunity—a position within text where the browser may optionally break a line, though its line-breaking rules would not otherwise create a break at that location.
  wbr Wbr HtmlElement [] void,

  // ==========================
  //   Image and multimedia
  // ==========================
  /// The `<area>` HTML element defines an area inside an image map that has predefined clickable areas. An image map allows geometric areas on an image to be associated with Hyperlink.
  area Area HtmlAreaElement [access_key "accessKey", alt, coords, download, hash, host, host_name "hostname", href, password, path_name "pathname", port, protocol, referrer_policy "referrerPolicy", rel, search, shape, tab_index "tabIndex", target, type_ "type", user_name "username"] void,

  /// The `<audio>` HTML element is used to embed sound content in documents. It may contain one or more audio sources, represented using the src attribute or the source element: the browser will choose the most suitable one. It can also be the destination for streamed media, using a MediaStream.
  audio Audio HtmlAudioElement [],

  /// The `<img>` HTML element embeds an image into the document.
  img Img HtmlImageElement [alt, cross_origin "crossOrigin", decoding, fetch_priority "fetchPriority", height, is_map "isMap", loading, referrer_policy "referrerPolicy", sizes, src, srcset, use_map "useMap", width] void,

  /// The `<map>` HTML element is used with area elements to define an image map (a clickable link area).
  map Map HtmlMapElement [name],

  /// The `<track>` HTML element is used as a child of the media elements, audio and video. It lets you specify timed text tracks (or time-based data), for example to automatically handle subtitles. The tracks are formatted in WebVTT format (.vtt files) — Web Video Text Tracks.
  track Track HtmlTrackElement [kind, src, src_lang "srclang", label, default] void,

  /// The `<video>` HTML element embeds a media player which supports video playback into the document. You can use `<video>` for audio content as well, but the audio element may provide a more appropriate user experience.
  video Video HtmlVideoElement [height, poster, width, disable_picture_in_picture "disablePictureInPicture"],

  // ==========================
  //     Embedded Content
  // ==========================
  /// The `<embed>` HTML element embeds external content at the specified point in the document. This content is provided by an external application or other source of interactive content such as a browser stuff-in.
  embed Embed HtmlEmbedElement [height, src, type_ "type", width] void,

  /// The `<iframe>` HTML element represents a nested browsing context, embedding another HTML page into the current one.
  iframe Iframe HtmlIFrameElement [allow, allow_full_screen "allowfullscreen", credentialless, csp, height, name, referrer_policy "referrerPolicy", sandbox, src, src_doc "srcdoc", width],

  /// The `<object>` HTML element represents an external resource, which can be treated as an image, a nested browsing context, or a resource to be handled by a stuffin.
  object Object HtmlObjectElement [data, height, name, type_ "type", use_map "useMap", width],

  /// The `<param>` HTML element defines parameters for an object element.
  param Param HtmlParamElement [] void,

  /// The `<picture>` HTML element contains zero or more source elements and one img element to offer alternative versions of an image for different display/device scenarios.
  picture Picture HtmlPictureElement [],

  /// The `<portal>` HTML element enables the embedding of another HTML page into the current one for the purposes of allowing smoother navigation into new pages.
  portal Protal HtmlElement [],

  /// The `<source>` HTML element specifies multiple media resources for the picture, the audio element, or the video element. It is an empty element, meaning that it has no content and does not have a closing tag. It is commonly used to offer the same media content in multiple file formats in order to provide compatibility with a broad range of browsers given their differing support for image file formats and media file formats.
  source Source HtmlSourceElement [media, sizes, src, src_set "srcset", type_ "type"] void,

  // ==========================
  //      SVG and MathML
  // ==========================
  /// The svg element is a container that defines a new coordinate system and viewport. It is used as the outermost element of SVG documents, but it can also be used to embed an SVG fragment inside an SVG or HTML document.
  svg Svg SvgElement [nonce, style, tab_index "tabIndex"],

  /// The top-level element in MathML is `<math>.` Every valid MathML instance must be mapped in `<math>` tags. In addition you must not nest a second `<math>` element in another, but you can have an arbitrary number of other child elements in it.
  math Math HtmlElement [],

  // ==========================
  //         Scripting
  // ==========================
  /// Use the HTML `<canvas>` element with either the canvas scripting API or the WebGL API to draw graphics and animations.
  canvas Canvas HtmlCanvasElement [height, width],

  /// The `<noscript>` HTML element defines a section of HTML to be inserted if a script type on the page is unsupported or if scripting is currently turned off in the browser.
  noscript NoScript HtmlElement [],

  /// The `<script>` HTML element is used to embed executable code or data; this is typically used to embed or refer to JavaScript code. The `<script>` element can also be used with other languages, such as WebGL's GLSL shader programming language and JSON.
  script Script HtmlScriptElement [type_ "type", src, async_ "async", defer, cross_origin "crossOrigin", text, fetch_priority "fetchPriority", referrer_policy "referrerPolicy"],

  // ==========================
  //     Demarcating Edits
  // ==========================
  /// The `<del>` HTML element represents a range of text that has been deleted from a document. This can be used when rendering "track changes" or source code diff information, for example. The ins element can be used for the opposite purpose: to indicate text that has been added to the document.
  del Del HtmlModElement [cite, date_time "dateTime"],

  /// The `<ins>` HTML element represents a range of text that has been added to a document. You can use the del element to similarly represent a range of text that has been deleted from the document.
  ins Ins HtmlElement [],

  // ==========================
  //     Table Content
  // ==========================
  /// The `<caption>` HTML element specifies the caption (or title) of a table.
  caption Caption HtmlTableCaptionElement [],

  /// The `<col>` HTML element defines a column within a table and is used for defining common semantics on all common cells. It is generally found within a colgroup element.
  col Col HtmlTableColElement [] void,

  /// The `<colgroup>` HTML element defines a group of columns within a table.
  colgroup ColGroup HtmlTableColElement [],

  /// The `<table>` HTML element represents tabular data — that is, information presented in a two-dimensional table comprised of rows and columns of cells containing data.
  table Table HtmlTableElement [caption, thead "tHead", tfoot "tFoot"],

  /// The `<tbody>` HTML element encapsulates a set of table rows (tr elements), indicating that they comprise the body of the table (table).
  tbody TBody HtmlTableSectionElement [],

  /// The `<td>` HTML element defines a cell of a table that contains data. It participates in the table model.
  td Td HtmlTableCellElement [],

  /// The `<tfoot>` HTML element defines a set of rows summarizing the columns of the table.
  tfoot TFoot HtmlTableSectionElement [],

  /// The `<th>` HTML element defines a cell as header of a group of table cells. The exact nature of this group is defined by the scope and headers attributes.
  th Th HtmlTableCellElement [],

  /// The `<thead>` HTML element defines a set of rows defining the head of the columns of the table.
  thead Thead HtmlTableSectionElement [],

  /// The `<tr>` HTML element defines a row of cells in a table. The row's cells can then be established using a mix of td (data cell) and th (header cell) elements.
  tr Tr HtmlTableRowElement [],

  // ==========================
  //          Forms
  // ==========================
  /// The `<button>` HTML element represents a clickable button, used to submit forms or anywhere in a document for accessible, standard button functionality.
  button Button HtmlButtonElement [access_key "accessKey", auto_focus "autofocus", disabled, form_action "formAction", form_enctype "formEnctype", form_method "formMethod", form_no_validate "formNoValidate", form_target "formTarget", menu, name, popover_target_action "popoverTargetAction", popover_target_element "popoverTargetElement", tab_index "tabIndex", type_ "type", value],

  /// The `<datalist>` HTML element contains a set of option elements that represent the permissible or recommended options available to choose from within other controls.
  datalist DataList HtmlDataListElement [],

  /// The `<fieldset>` HTML element is used to group several controls as well as labels (label) within a web form.
  fieldset FieldSet HtmlFieldSetElement [disabled, name, validation_message "validationMessage", validity, will_validate "willValidate"],

  /// The `<form>` HTML element represents a document section containing interactive controls for submitting information.
  form Form HtmlFormElement [name, method, target, action, encoding, enctype, accept_charset "acceptCharset", auto_complete "autocomplete", no_validate "noValidate"],

  /// The `<input>` HTML element is used to create interactive controls for web-based forms in order to accept data from the user; a wide variety of types of input data and control widgets are available, depending on the device and user agent. The `<input>` element is one of the most powerful and complex in all of HTML due to the sheer number of combinations of input types and attributes.
  input Input HtmlInputElement [auto_capitalize "autocapitalize", default_value "defaultValue", dir_name "dirName", input_mode "inputmode", multiple, name, popover_target_action "popoverTargetAction", popover_target_element "popoverTargetElement", step, type_ "type", value, value_as_date "valueAsDate", value_as_number "valueAsNumber"] void,

  /// The `<label>` HTML element represents a caption for an item in a user interface.
  label Label HtmlLabelElement [html_for "htmlFor"],

  /// The `<legend>` HTML element represents a caption for the content of its parent fieldset.
  legend Legend HtmlLegendElement [access_key "accessKey"],

  /// The `<meter>` HTML element represents either a scalar value within a known range or a fractional value.
  meter Meter HtmlMeterElement [high, low, max, min, optimum, value],

  /// The `<optgroup>` HTML element creates a grouping of options within a select element.
  optgroup OptGroup HtmlOptGroupElement [disabled, label],

  /// The `<option>` HTML element is used to define an item contained in a select, an optgroup, or a datalist element. As such, `<option>` can represent menu items in popups and other lists of items in an HTML document.
  option_ Option HtmlOptionElement [default_selected "defaultSelected", disabled, selected, text, value],

  /// The `<output>` HTML element is a container element into which a site or app can inject the results of a calculation or the outcome of a user action.
  output Output HtmlOutputElement [default_value "defaultValue", name, value],

  /// The `<progress>` HTML element displays an indicator showing the completion progress of a task, typically displayed as a progress bar.
  progress Progress HtmlProgressElement [max, value],

  /// The `<select>` HTML element represents a control that provides a menu of options:
  select Select HtmlSelectElement [auto_focus "autofocus", disabled, length, multiple, name, required, selected_index "selectedIndex", size, value],

  /// The `<textarea>` HTML element represents a multi-line plain-text editing control, useful when you want to allow users to enter a sizeable amount of free-form text, for example a comment on a review or feedback form.
  textarea TextArea HtmlTextAreaElement [access_key "accessKey", auto_capitalize "autocapitalize", auto_complete "autocomplete", auto_focus "autofocus", cols, default_value "defaultValue", disabled, input_mode "inputMode", max_length "maxLength", min_length "minLength", name, placeholder, read_only "readOnly", required, rows, selection_end "selectionEnd", selection_start "selectionStart", tab_index "tabIndex", value, wrap],

  // ==========================
  //    Interactive elements
  // ==========================
  /// The `<details>` HTML element creates a disclosure widget in which information is visible only when the widget is toggled into an "open" state. A summary or label must be provided using the summary element.
  details Details HtmlDetailsElement [open],

  /// The `<dialog>` HTML element represents a dialog box or other interactive component, such as a dismissible alert, inspector, or subwindow.
  dialog Dialog HtmlDialogElement [open, return_value "returnValue"],

  /// The `<menu>` HTML element is a semantic alternative to ul. It represents an unordered list of items (represented by li elements), each of these represent a link or other command that the user can activate.
  menu Menu HtmlMenuElement [],

  /// The `<summary>` HTML element specifies a summary, caption, or legend for a details element's disclosure box. Clicking the `<summary>` element toggles the state of the parent `<details>` element open and closed.
  summary Summary HtmlElement [],

  // ==========================
  //      Web Components
  // ==========================
  /// The `<graff>` HTML element—part of the Web Components technology suite—is a placeholder inside a web component that you can fill with your own markup, which lets you create separate DOM trees and present them together.
  slot Slot HtmlSlotElement [name],

  /// The `<template>` HTML element is a mechanism for holding HTML that is not to be rendered immediately when a page is loaded but may be instantiated subsequently during runtime using JavaScript.
  template Template HtmlTemplateElement [],
];

impl IntoFiller for String {
    fn into_filler(self) -> Filler {
        span().html(self).into_filler()
    }
}
impl<'a> IntoFiller for &'a String {
    fn into_filler(self) -> Filler {
        span().html(self.to_owned()).into_filler()
    }
}

impl<'a> IntoFiller for &'a str {
    fn into_filler(self) -> Filler {
        span().html(self.to_owned()).into_filler()
    }
}
impl<'a> IntoFiller for Cow<'a, str> {
    fn into_filler(self) -> Filler {
        span().html(self.into_owned()).into_filler()
    }
}

impl<T> IntoFiller for Cage<T>
where
    T: AsRef<str> + fmt::Debug + 'static,
{
    fn into_filler(self) -> Filler {
        let html: String = (*self.get()).as_ref().to_owned();
        span().html(html).into_filler()
    }
}

impl<F, T> IntoFiller for Bond<F, T>
where
    F: Fn() -> T + Clone + 'static,
    T: AsRef<str> + fmt::Debug + 'static,
{
    fn into_filler(self) -> Filler {
        let html: String = (*self.get()).as_ref().to_owned();
        span().html(html).into_filler()
    }
}
