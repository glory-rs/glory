#![cfg(not(target_arch = "wasm32"))]
use glory_core::{AccessError, Runtime, executor::Executor};
use std::rc::Rc;
#[test]
fn presence_observation_is_pure_and_validates_the_current_owner_and_runtime() {
    let runtime = Runtime::new();
    let scope = runtime.scope();
    assert_eq!(scope.has_task_host(), Ok(false));
    let executor = Rc::new(Executor::new(4));
    let hosted = Runtime::with_host(executor.clone());
    let live = hosted.scope();
    assert_eq!(live.has_task_host(), Ok(true));
    assert_eq!(executor.len(), 0);
    let child = live.child();
    child.owner().dispose();
    assert_eq!(child.has_task_host(), Err(AccessError::Disposed));
    drop(hosted);
    assert_eq!(live.has_task_host(), Err(AccessError::RuntimeGone));
    assert_eq!(scope.has_task_host(), Ok(false));
}
