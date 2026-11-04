#![cfg(not(target_arch = "wasm32"))]
use glory_apace::Renderer;
use glory_html::Part;
use glory_render::{NodeId,Purpose,Query,QueryValue,Rect};
use glory_render::style::{Edges,Family,Length,Style};
use glory_ui::{App,input,view};
use std::{cell::RefCell,rc::Rc};

fn bounds(app:&App,node:NodeId)->Rect {
    let reply=Rc::new(RefCell::new(None));let out=reply.clone();
    app.request(Query::Bounds(node),move|value|*out.borrow_mut()=Some(value.unwrap())).unwrap();
    app.pump().unwrap();
    let QueryValue::Bounds(rect)=reply.borrow_mut().take().unwrap() else {panic!("bounds")};rect
}

#[test]
fn mixed_lengths_and_letter_spacing_use_real_font_layout_and_keep_editor_identity() {
    for font in [14.,24.,36.] {for masked in [false,true] {
        let mut renderer=None;
        let app=App::new(|runtime| {let host=Renderer::new(runtime);renderer=Some(host.clone());host});
        let host=renderer.unwrap();host.set_viewport(700,200,1.).unwrap();
        let reference=app.scope().cage(None);let zero=app.scope().cage(None);
        let style=app.scope().cage(Style {font_size:Some(font),..Style::new()
            .font_family([Family::Mono]).width(Length::ChPx {ch:6.,px:180.}).height(Length::Px(64.))
            .padding(Edges::all(0.)).border_width(Edges::all(0.))});
        app.mount(view().fill((
            view().node_ref(zero).style(Style {font_size:Some(font),..Style::new().font_family([Family::Mono]).width(Length::Ch(1.)).height(Length::Px(1.))}),
            input().value("000000").purpose(if masked {Purpose::Password}else{Purpose::Text}).style(style).node_ref(reference),
        ))).unwrap();app.settle(12).unwrap();
        let node=reference.get().unwrap();let physical=host.engine_id(node,Part::Main).unwrap();
        let unit=bounds(&app,zero.get().unwrap()).width;
        let initial=bounds(&app,node);
        assert!((initial.width-(6.*unit+180.)).abs()<0.2,"{initial:?} {unit}");
        let advances=||host.inspect(|doc| {
            let editor=doc.get_node(physical).unwrap().element_data().unwrap().text_input_data().unwrap();
            assert_eq!(editor.is_password(),masked);
            editor.display_editor().try_layout().unwrap().lines().flat_map(|line|line.runs())
                .flat_map(|run|run.visual_clusters()).map(|cluster|cluster.advance()).collect::<Vec<_>>()
        });
        let baseline=advances();assert_eq!(baseline.len(),6);
        style.revise(|value|value.letter_spacing=Some(12.));app.flush().unwrap();app.settle(12).unwrap();
        let spaced=advances();assert_eq!(spaced.len(),6);
        for (before,after) in baseline.iter().zip(&spaced) {
            assert!((after-before-12.).abs()<0.2,"font={font} masked={masked}: {baseline:?} -> {spaced:?}");
        }
        assert!((bounds(&app,node).width-initial.width).abs()<0.2,"ch does not include letter spacing");
        assert_eq!(host.engine_id(node,Part::Main).unwrap(),physical);
        style.revise(|value|value.letter_spacing=None);app.flush().unwrap();app.settle(12).unwrap();
        assert_eq!(advances(),baseline);
        app.clear().unwrap();assert_eq!(app.stats().queries,0);
    }}
}
