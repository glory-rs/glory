#![cfg(not(target_arch = "wasm32"))]
use glory_blitz::Renderer;
use glory_html::Part;
use glory_render::*;
#[test]
fn native_file_controls_reject_before_engine_allocation_and_keep_the_valid_prefix() {
    let runtime = RuntimeId::new();
    let mut ids = NodeIds::new(runtime);
    let good = ids.allocate();
    let file = ids.allocate();
    let mut renderer = Renderer::new(runtime);
    let file_node = Descriptor::new(Props::File(FileProps::default()));
    assert_eq!(
        renderer.preflight(file_node.view(), None),
        Err(Error::Unsupported("native file picker controls"))
    );
    assert_eq!(
        renderer.apply(&[
            Mutation::Create {
                id: good,
                node: Box::new(Descriptor::new(Props::View))
            },
            Mutation::Place {
                id: good,
                parent: None,
                before: None
            },
            Mutation::Create {
                id: file,
                node: Box::new(file_node)
            }
        ]),
        Err(Error::Unsupported("native file picker controls"))
    );
    assert!(renderer.engine_id(good, Part::Main).is_ok());
    assert!(renderer.engine_id(file, Part::Main).is_err());
    let receipt = renderer.apply(&[]).unwrap();
    assert_eq!(receipt.applied, 2);
    let query = QueryToken(1);
    renderer.request(Query::Bounds(good), query).unwrap();
    let mut inputs = Vec::new();
    renderer.drain(&mut inputs);
    let response = inputs
        .into_iter()
        .find_map(|input| match input {
            Input::Response {
                token,
                applied,
                result,
            } if token == query => Some((applied, result)),
            _ => None,
        })
        .unwrap();
    assert_eq!(response.0, receipt.applied);
    assert!(matches!(response.1, Ok(QueryValue::Bounds(_))));
}
