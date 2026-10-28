use glory as renamed;
use glory::display::*;

const LABEL: Field = Field::new("label");
const CLASS: Field = Field::new("class");
const CLICK: Click = Click::new("click");
const BODY: Outlet = Outlet::new("body");
static HAND: Template = Template::new(&[div(
    &[bind_attr("class", CLASS)],
    &[
        span(&[], &[field(LABEL)]),
        button(&[attr("type", "button"), on_click(CLICK)], &[text("+\"🙂")]),
        div(&[children(BODY)], &[]),
    ],
)]);
static XML: Template = renamed::template! {
    <div field:class={CLASS}>
        <span><field slot={LABEL}/></span>
        <button type="button" on:click={CLICK}>"+\"🙂"</button>
        <div children={BODY}/>
    </div>
};

#[test]
fn handwritten_and_xml_declarations_have_identical_wire_and_slots() {
    let a = HAND.compile().unwrap();
    let b = XML.compile().unwrap();
    assert_eq!(a.wire, b.wire);
    assert_eq!(a.node_count, b.node_count);
    assert_eq!(
        a.slots.iter().map(|s| (s.name, s.kind)).collect::<Vec<_>>(),
        b.slots.iter().map(|s| (s.name, s.kind)).collect::<Vec<_>>()
    );
}
#[test]
fn live_xml_uses_the_same_declaration_and_does_not_run_computations_during_construction() {
    let runtime = glory::Runtime::new();
    let scope = runtime.scope();
    let label: glory::Vary<String> = scope.cage("a".to_owned()).into();
    let called = std::rc::Rc::new(std::cell::Cell::new(0));
    let calls = called.clone();
    let live = renamed::view! {
        <div class={move || {calls.set(calls.get()+1);"counter"}}>
            <span>{label}</span>
            <button type="button" on:click={|| {}}>"+\"🙂"</button>
            <div children={BODY}/>
        </div>
    };
    assert_eq!(
        live.template().compile().unwrap().wire,
        HAND.compile().unwrap().wire
    );
    live.validate(&scope).unwrap();
    assert_eq!(called.get(), 0);
    assert_eq!(
        runtime.stats().nodes,
        1,
        "configuration does not install observers"
    );
}
#[test]
fn configuration_rejects_wrong_kind_duplicates_foreign_and_disposed_inputs() {
    let runtime = glory::Runtime::new();
    let scope = runtime.scope();
    assert!(
        view(&HAND)
            .bind(Field::new("click"), "wrong")
            .validate(&scope)
            .is_err()
    );
    assert!(
        view(&HAND)
            .bind(LABEL, "a")
            .bind(LABEL, "b")
            .validate(&scope)
            .is_err()
    );
    assert!(
        view(&HAND)
            .on_click(Click::new("label"), || {})
            .validate(&scope)
            .is_err()
    );
    let other = glory::Runtime::new();
    let other_scope = other.scope();
    let foreign = other_scope.cage(1);
    assert!(view(&HAND).bind(LABEL, foreign).validate(&scope).is_err());
    let child = scope.child();
    let retired = child.cage(2);
    drop(child);
    assert!(view(&HAND).bind(LABEL, retired).validate(&scope).is_err());
    assert_eq!(runtime.stats().nodes, 0);
}
struct Counter;
struct CounterProps {
    initial: i32,
}
impl Component for Counter {
    type Props = CounterProps;
    fn view(scope: &Scope, props: Self::Props) -> impl IntoView {
        let count = scope.cage(props.initial);
        renamed::view! {<span>{count}</span>}
    }
}
#[test]
fn components_have_an_explicit_props_contract_and_a_real_outlet_without_a_wrapper_element() {
    fn is_view(_: impl IntoView) {}
    is_view(renamed::view! {<Counter initial={3}/>});
    let nested = renamed::view! {<div>"before"<Counter initial={4}/><span>"after"</span></div>};
    let definition = nested.template().compile().unwrap();
    assert_eq!(definition.node_count, 5);
    assert!(definition.wire.contains("[2]"));
    assert_eq!(definition.slots[0].kind, glory::html::template::Kind::Range);
}
#[test]
fn fragments_and_plain_values_do_not_require_a_component_or_a_bond() {
    let runtime = glory::Runtime::new();
    let scope = runtime.scope();
    let count = scope.cage(2);
    let literal = count.get();
    let view = renamed::view! {<><span>{literal}</span><span>{move || count.get()*2}</span></>};
    view.validate(&scope).unwrap();
    assert_eq!(runtime.stats().nodes, 1);
    assert_eq!(view.template().compile().unwrap().slots.len(), 2);
}

#[test]
fn rust_blocks_can_prepare_owned_captures_and_return_a_reactive_closure() {
    let runtime = glory::Runtime::new();
    let scope = runtime.scope();
    let count = scope.cage(2);
    let value = std::rc::Rc::new("prefix".to_owned());
    let display = renamed::view! {
        <span title={let label=value.clone();move || format!("{label} {}",count.get())}>
            {let label=value.clone();move || format!("{label} {}",count.get()*2)}
        </span>
    };
    display.validate(&scope).unwrap();
    assert_eq!(runtime.stats().nodes, 1);
    assert_eq!(std::rc::Rc::strong_count(&value), 3);
}

#[renamed::component(renamed)]
fn Panel(children: ChildContent) -> impl IntoView {
    renamed::view! {<div><Content value={children}/></div>}
}
#[renamed::component(renamed)]
fn Simple(scope: &Scope, initial: i32) -> impl IntoView {
    let value = scope.cage(initial);
    renamed::view! {<span>{value}</span>}
}
#[test]
fn structural_content_components_and_fields_share_the_public_api() {
    let runtime = glory::Runtime::new();
    let scope = runtime.scope();
    let source = scope
        .keyed_list(vec![(1usize, "hello".to_owned())], |r| r.0)
        .unwrap();
    let flag = scope.cage(true);
    let config = renamed::view! {
        <Panel>
            <Each each={source} children={move |row| {
                let text=row.field("text",|r|&r.1,|r|&mut r.1);
                renamed::view! {<><span>{text}</span><Simple initial={3}/></>}
            }}/>
            <Show when={move || flag.get()} fallback={||renamed::view!{<span>"no"</span>}}>
                <Simple initial={2}/>
            </Show>
        </Panel>
    };
    fn is_view(_: impl IntoView) {}
    is_view(config);
    struct Data {
        label: String,
    }
    let value = scope.cage(Data { label: "x".into() });
    let label = renamed::field!(value.label);
    label.revise(|v| v.push('!'));
    assert_eq!(value.with(|v| v.label.clone()), "x!");
    let foreign = glory::Runtime::new();
    let foreign_scope = foreign.scope();
    let other = foreign_scope
        .keyed_list(Vec::<usize>::new(), |v| *v)
        .unwrap();
    assert!(
        view(&HAND)
            .child(
                BODY,
                each(other, |v| renamed::view! {<span>{*v.key()}</span>})
            )
            .validate(&scope)
            .is_err()
    );
}
#[test]
fn class_flags_keep_static_class_data_and_typed_boolean_contracts() {
    let rt = glory::Runtime::new();
    let scope = rt.scope();
    let active = scope.cage(false);
    let view =
        renamed::view! {<div class="base active" class:active={active} class:other={move||true}/>};
    view.validate(&scope).unwrap();
    let compiled = view.template().compile().unwrap();
    assert_eq!(
        compiled.slots[0].kind,
        glory::html::template::Kind::Class("active", true)
    );
    assert_eq!(
        compiled.slots[1].kind,
        glory::html::template::Kind::Class("other", false)
    );
    assert_eq!(rt.stats().nodes, 1);
}
