use std::cell::RefCell;
use std::rc::Rc;

use glory::reflow::*;
use glory::routing::*;
use glory::web::widgets::*;
use glory::widgets::*;
use glory::*;
#[cfg(feature = "web-csr")]
use wasm_bindgen::UnwrapThrowExt;

use crate::models::PageInfo;
#[cfg(feature = "web-csr")]
use crate::models::{Post, PostMetadata};

#[derive(Debug)]
pub struct Nav

impl Widget for Nav {
    fn build(&mut self, ctx: &mut Scope) {
        header().class("header").fill(
            nav().class("inner").fill(
                a().href("", "/home").fill(strong().html("Home"))
            ).fill(
                a().href("/new").fill(strong().html("New"))
            ).fill(
                a().href("/show").fill(strong().html("Show"))
            ).fill(
                a().href("/ask").fill(strong().html("Ask"))
            ).fill(
                a().href("/job").fill(strong().html("Jobs"))
            ).fill(
                a().class("github").href("http://github.com/glory-rs/glory").attr("target", "_blank").attr("rel", "noreferrer").html("Built with Glory")
            )
        ).show_in(ctx);
    }
}
