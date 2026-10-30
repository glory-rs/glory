"""Check real downstream compiler diagnostics, including a renamed facade dependency."""
import json
import os
from pathlib import Path
import subprocess

repo = Path(__file__).resolve().parent.parent
root = Path(os.environ.get("GLORY_DSL_BUILD_ROOT", "D:/BuildCache/glory-dsl-20261028")).resolve()
project = root / "diagnostics-project"
(project / "src").mkdir(parents=True, exist_ok=True)
(root / "tmp").mkdir(exist_ok=True)
env = dict(os.environ, CARGO_HOME=os.environ.get("CARGO_HOME", "D:/BuildCache/glory-semantic-ui/cargo"),
           CARGO_TARGET_DIR=str(root / "check-target"), TEMP=str(root / "tmp"), TMP=str(root / "tmp"))
(project / "Cargo.toml").write_text(
    '[package]\nname="display-diagnostics"\nversion="0.0.0"\nedition="2024"\n'
    '[workspace]\n[dependencies]\ng={package="glory",path=' + json.dumps((repo / "crates/glory").as_posix())
    + ',default-features=false,features=["dsl"]}\n', encoding="utf-8")
component = """use g::display::*;
#[g::component(g)] fn Counter(initial:u32)->impl Widget {g::view!{<span>{initial}</span>}}
fn accept(_:impl Widget){}
"""
cases = [
    ("renamed_facade", 'fn main(){let _=g::view!{<div><span>"ok"</span></div>};}', None),
    ("closing_tag", 'fn main(){let _=g::view!{<div></span>};}', "closing tag does not match"),
    ("unquoted_text", 'fn main(){let _=g::view!{<span>hello</span>};}', "quoted text"),
    ("invalid_content", 'fn main(){let _=g::view!{<button><div/></button>};}', "invalid HTML content context"),
    ("unsupported_tag", 'fn main(){let _=g::view!{<input/>};}', "unsupported HTML tag"),
    ("missing_prop", component+'fn main(){accept(g::view!{<Counter/>});}', "trait bound"),
    ("unknown_prop", component+'fn main(){let _=g::view!{<Counter inital={1}/>};}', "no method named `inital`"),
    ("wrong_prop_type", component+'fn main(){let _=g::view!{<Counter initial={"bad"}/>};}', "mismatched types"),
    ("wrong_slot_kind", 'fn main(){let _=g::template!{<span><field slot={g::display::Click::new("click")}/></span>};}', "mismatched types"),
    ("not_displayable", 'fn main(){let _=g::view!{<span>{vec![1,2]}</span>};}', "TextValue"),
    ("empty_fragment", 'fn main(){let _=g::view!{<></>};}', "empty XML view"),
    ("private_template_cache", 'fn main(){let _=g::display::Template{roots:&[],compiled:Default::default()};}', "private"),
    ("renamed_component_and_children", 'use g::display::*; #[g::component(g)] fn Panel(children:g::ui::Children)->impl Widget {g::view!{<div><Content value={children}/></div>}} fn main(){let _=g::view!{<Panel><span>"ok"</span></Panel>};}', None),
    ("prelude_component", 'use g::display::*; #[component(g)] fn Counter(scope:&Scope,initial:i32)->impl Widget {let value=scope.cage(initial);view!{<span>{value}</span>}} fn main(){let _=view!{<Counter initial={1}/>};}', None),
    ("wrong_injected_scope", 'use g::display::*; #[g::component(g)] fn Counter(scope:&g::Scope)->impl Widget {view!{<span>"bad"</span>}} fn main(){}', "mismatched types"),
    ("each_missing_factory", 'fn main(){let _=g::view!{<Each each={items}/>};}', "requires children"),
    ("each_unknown_property", 'fn main(){let _=g::view!{<Each each={items} unknown={key} children={factory}/>};}', "unknown structural property"),
    ("vector_each_missing_key", 'fn main(){let _=g::view!{<Each each={vec![1usize]} children={|_|g::view!{<span>"row"</span>}}/>};}', "unsatisfied trait bounds"),
    ("vector_each", 'fn main(){let _:Box<dyn g::Widget>=Box::new(g::view!{<Each each={vec![1usize]} key={|id|*id} children={|row|g::view!{<span>{row.value}{row.index}</span>}}/>});}', None),
    ("vector_each_seed", 'fn main(){let _:Box<dyn g::Widget>=Box::new(g::view!{<Each each={vec![1usize]} key={|id|*id} seed={|id|g::view!{<span>{id}</span>}}/>});}', None),
    ("each_children_and_seed", 'fn main(){let _=g::view!{<Each each={items} children={factory} seed={factory}/>};}', "either children or seed"),
    ("show_missing_condition", 'fn main(){let _=g::view!{<Show children={factory}/>};}', "requires when"),
    ("children_twice", component+'fn main(){let _=g::view!{<Counter initial={1} children={children}><span>"bad"</span></Counter>};}', "children supplied twice"),
    ("generic_component", 'use g::display::*; #[g::component(g)] fn Example<T>(value:T)->impl Widget {g::view!{<span>"bad"</span>}} fn main(){}', "concrete property types"),
    ("unsafe_component", 'use g::display::*; #[g::component(g)] unsafe fn Example()->impl Widget {g::view!{<span>"bad"</span>}} fn main(){}', "component requires a synchronous function with concrete property types"),
    ("async_component", 'use g::display::*; #[g::component(g)] async fn Example()->impl Widget {g::view!{<span>"bad"</span>}} fn main(){}', "component requires a synchronous function with concrete property types"),
    ("const_component", 'use g::display::*; #[g::component(g)] const fn Example()->impl Widget {g::view!{<span>"bad"</span>}} fn main(){}', "component requires a synchronous function with concrete property types"),
    ("extern_component", 'use g::display::*; #[g::component(g)] extern "C" fn Example()->impl Widget {g::view!{<span>"bad"</span>}} fn main(){}', "component requires a synchronous function with concrete property types"),
    ("opaque_factory", 'use g::Widget; fn button()->impl Widget {g::ui::button().label("factory")} fn main(){let _:Box<dyn Widget>=Box::new(g::view!{<div><{button()}/></div>});}', None),
    ("opaque_factory_setter", 'use g::Widget; fn button()->impl Widget {g::ui::button()} fn main(){let _=g::view!{<{button()} label="hidden"/>};}', "no method named `label`"),
    ("concrete_factory_children", 'fn main(){let _:Box<dyn g::Widget>=Box::new(g::view!{<{g::ui::group()}><span>"child"</span></>});}', None),
    ("field_expression", 'fn main(){let _=g::field!(a+b);}', "requires handle.named_field"),
    ("field_tuple", 'fn main(){let _=g::field!(a.0);}', "named struct field"),
    ("invalid_table_child", 'fn main(){let _=g::view!{<table><tr/></table>};}', "invalid HTML content context"),
    ("invalid_table_text", 'fn main(){let _=g::view!{<table>"bad"</table>};}', "cannot contain direct text"),
    ("whole_class_and_toggle", 'fn main(){let _=g::view!{<div class={value} class:active={true}/>};}', "cannot share a dynamic whole class"),
    ("non_boolean_toggle", 'fn main(){let _=g::view!{<div class:active={"yes"}/>};}', "BoolValue"),
]
(project / "src/main.rs").write_text("fn main() {}\n", encoding="utf-8")
metadata = subprocess.run(["cargo", "metadata", "--manifest-path", str(project / "Cargo.toml"),
                           "--no-deps", "--format-version", "1"], env=env, capture_output=True, text=True, check=True)
assert Path(json.loads(metadata.stdout)["target_directory"]).resolve() == (root / "check-target").resolve()
# Fetch once for a fresh CI cache; individual diagnostic cases stay offline.
subprocess.run(["cargo", "fetch", "--manifest-path", str(project / "Cargo.toml")], env=env, check=True)
results = []
for name, source, expected in cases:
    (project / "src/main.rs").write_text(source, encoding="utf-8")
    result = subprocess.run(["cargo", "check", "--offline", "--manifest-path", str(project / "Cargo.toml")],
                            env=env, capture_output=True, text=True, encoding="utf-8")
    (root / f"diagnostic-{name}.txt").write_text(result.stderr, encoding="utf-8")
    assert (result.returncode == 0) if expected is None else (result.returncode != 0 and expected in result.stderr), result.stderr
    results.append({"name": name, "expected": expected, "returncode": result.returncode, "source": source})
(root / "diagnostics.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
print(f"Passed {len(results)} downstream compile/diagnostic cases")
