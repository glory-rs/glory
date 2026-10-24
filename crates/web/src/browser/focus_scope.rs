//! Focus policy leases share the document's Layer event coordinator. They do
//! not create top-layer boxes, inert branches, paint order or modal AX state.
use super::*;
use glory_render::FocusScopeConfig;

#[derive(Clone)]
pub(super) struct Scope {
    owner: Weak<Dom>,
    id: NodeId,
    element: Element,
    config: FocusScopeConfig,
    previous: Option<Element>,
    focused: bool,
}
fn current(scope: &Scope) -> Option<Scope> {
    let dom = scope.owner.upgrade()?;
    let config = dom
        .layers
        .borrow()
        .scope_declarations
        .get(&scope.id)?
        .clone();
    (dom.live.get() && config.active && visible(&scope.element)).then(|| Scope {
        config,
        ..scope.clone()
    })
}
fn contains(scope: &Scope, target: &Node) -> bool {
    if scope.element.contains(Some(target)) {
        return true;
    }
    // A popup anchored in the scope is part of its current focus policy even
    // when its physical box lives in a different mounted branch.
    scope.owner.upgrade().is_some_and(|dom| {
        dom.layers.borrow().stack.iter().any(|open| {
            open.element.contains(Some(target))
                && named(&dom, open.config.anchor.as_ref())
                    .is_some_and(|anchor| scope.element.contains(Some(&anchor)))
        })
    })
}
fn scope_candidates(root: &Element) -> Result<Vec<Element>, Error> {
    let mut pending = vec![root.clone()];
    let mut values = Vec::new();
    let mut inspected = 0usize;
    while let Some(element) = pending.pop() {
        inspected += 1;
        if inspected > 65_536 {
            return Err(Error::Unsupported("FocusScope physical node budget"));
        }
        if !visible(&element) {
            continue;
        }
        if js_sys::Reflect::get(&element, &"tabIndex".into())
            .ok()
            .and_then(|value| value.as_f64())
            .is_some_and(|value| value >= 0.0)
            && !element.matches(":disabled").unwrap_or(true)
        {
            values.push(element.clone());
        }
        let mut child = element.last_element_child();
        while let Some(element) = child {
            if inspected + pending.len() >= 65_536 {
                return Err(Error::Unsupported("FocusScope physical node budget"));
            }
            child = element.previous_element_sibling();
            pending.push(element);
        }
    }
    Ok(values)
}
fn first(scope: &Scope) -> Result<Element, Error> {
    if let Some(element) = scope
        .owner
        .upgrade()
        .and_then(|dom| named_control(&dom, scope.config.initial_focus.as_ref()))
        .filter(|target| {
            scope.element.contains(Some(target))
                && focusable(target)
                && !target.matches(":disabled").unwrap_or(true)
        })
    {
        return Ok(element);
    }
    Ok(scope_candidates(&scope.element)?
        .into_iter()
        .next()
        .unwrap_or_else(|| scope.element.clone()))
}
impl DocumentLayers {
    pub(in crate::browser) fn allows_scope(&self, target: &Node) -> bool {
        self.scopes
            .borrow()
            .iter()
            .rev()
            .filter_map(current)
            .find(|scope| {
                scope.config.contain_focus
                    && visible(&scope.element)
                    && self.allows(Some(scope.element.as_ref()))
            })
            .is_none_or(|scope| contains(&scope, target))
    }

    pub(super) fn sync_scopes(&self) -> Result<(), Error> {
        let owned_inert = self.state.borrow().inert.clone();
        let apps = self
            .state
            .borrow()
            .apps
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        let current = apps
            .iter()
            .filter(|dom| dom.live.get())
            .flat_map(|dom| {
                dom.layers
                    .borrow()
                    .scope_declarations
                    .iter()
                    .filter_map(|(id, config)| {
                        let element = dom.entry(*id).ok()?.element().ok()?;
                        let live_behind_modal = element.is_connected()
                            && element
                                .closest("[hidden],[data-gly-inert]")
                                .ok()
                                .flatten()
                                .is_none()
                            && owned_inert
                                .iter()
                                .any(|branch| branch.contains(Some(&element)))
                            && element.get_bounding_client_rect().width() > 0.0
                            && element.get_bounding_client_rect().height() > 0.0;
                        (config.active && (visible(&element) || live_behind_modal)).then(|| Scope {
                            owner: Rc::downgrade(dom),
                            id: *id,
                            element,
                            config: config.clone(),
                            previous: None,
                            focused: false,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let active = self.document.active_element();
        let mut scopes = std::mem::take(&mut *self.scopes.borrow_mut());
        let mut removed = Vec::new();
        scopes.retain_mut(|scope| {
            if let Some(next) = current
                .iter()
                .find(|next| next.id == scope.id && Weak::ptr_eq(&next.owner, &scope.owner))
            {
                scope.config = next.config.clone();
                true
            } else {
                removed.push(scope.clone());
                false
            }
        });
        // Remove out-of-order restore edges before forgetting a retired scope.
        for index in 0..removed.len() {
            let old = removed[index].clone();
            for live in scopes.iter_mut().chain(removed.iter_mut()) {
                if live
                    .previous
                    .as_ref()
                    .is_some_and(|target| old.element.contains(Some(target)))
                {
                    live.previous = old.previous.clone();
                }
            }
        }
        let mut restore = removed
            .iter()
            .rev()
            .filter(|scope| {
                scope.config.restore_focus
                    && (scope.focused
                        || active
                            .as_ref()
                            .is_some_and(|target| contains(scope, target.as_ref())))
            })
            .flat_map(|scope| {
                [
                    scope
                        .owner
                        .upgrade()
                        .and_then(|dom| named_control(&dom, scope.config.return_focus.as_ref())),
                    scope.previous.clone(),
                ]
            })
            .flatten()
            .find(focusable);
        let mut autofocus = None;
        for mut next in current {
            if scopes
                .iter()
                .any(|scope| scope.id == next.id && Weak::ptr_eq(&scope.owner, &next.owner))
            {
                continue;
            }
            next.previous = restore.clone().or_else(|| active.clone());
            if next.config.auto_focus && self.allows(Some(next.element.as_ref())) {
                autofocus = Some(first(&next)?);
            }
            let position = scopes
                .iter()
                .position(|scope| next.element.contains(Some(&scope.element)))
                .unwrap_or(scopes.len());
            scopes.insert(position, next);
        }
        // An enclosing scope cannot outrank one of its live descendants merely
        // because the outer declaration was activated later.
        if let Some(scope) = scopes
            .iter()
            .rev()
            .find(|scope| scope.config.contain_focus && self.allows(Some(scope.element.as_ref())))
            && (scope.focused || autofocus.is_some())
            && !active
                .as_ref()
                .is_some_and(|target| contains(scope, target.as_ref()) && focusable(target))
        {
            let inner = first(scope)?;
            if autofocus
                .as_ref()
                .is_none_or(|target| !contains(scope, target.as_ref()))
            {
                autofocus = None;
                restore = Some(inner);
            }
        }
        let target = autofocus
            .or(restore)
            .filter(focusable)
            .filter(|target| self.allows(Some(target.as_ref())));
        *self.scopes.borrow_mut() = scopes;
        if let Some(target) = target {
            dom::focus(&target)?;
        }
        Ok(())
    }
    pub(super) fn receive_scope(&self, name: &str, event: &web_sys::Event) -> Result<bool, Error> {
        if !matches!(name, "focusin" | "keydown" | "pointerdown") {
            return Ok(false);
        }
        let target = event
            .target()
            .and_then(|value| value.dyn_into::<Node>().ok());
        if name == "focusin" {
            // No host calls while the registry is borrowed.
            let focused = self
                .scopes
                .borrow()
                .iter()
                .map(|scope| {
                    target
                        .as_ref()
                        .is_some_and(|target| contains(scope, target))
                })
                .collect::<Vec<_>>();
            for (scope, focused) in self.scopes.borrow_mut().iter_mut().zip(focused) {
                scope.focused = focused;
            }
        }
        if !self.allows(target.as_ref()) {
            return Ok(false); // The modal coordinator performs this repair.
        }
        let scopes = self
            .scopes
            .borrow()
            .iter()
            .filter_map(current)
            .collect::<Vec<_>>();
        let Some(scope) = scopes.iter().rev().find(|scope| {
            visible(&scope.element)
                && self.allows(Some(scope.element.as_ref()))
                && (scope.config.contain_focus
                    || target
                        .as_ref()
                        .is_some_and(|target| contains(scope, target)))
        }) else {
            return Ok(false);
        };
        if name == "pointerdown" {
            if scope.config.contain_focus
                && !target
                    .as_ref()
                    .is_some_and(|target| contains(scope, target))
                && target
                    .as_ref()
                    .and_then(|node| node.dyn_ref::<Element>())
                    .and_then(|element| {
                        element
                            .closest("input,textarea,button,a[href],[tabindex]")
                            .ok()
                            .flatten()
                    })
                    .is_some_and(|element| focusable(&element))
            {
                // Preserve the editor's existing focus/preedit. This suppresses
                // only the host default; outside click activation remains live.
                event.prevent_default();
            }
            return Ok(false);
        }
        if name == "focusin" {
            if scope.config.contain_focus
                && !target
                    .as_ref()
                    .is_some_and(|target| contains(scope, target))
            {
                dom::focus(&first(scope)?)?;
                return Ok(true);
            }
            return Ok(false);
        }
        let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
            return Ok(false);
        };
        if key.key() != "Tab"
            || key.is_composing()
            || key.alt_key()
            || key.ctrl_key()
            || key.meta_key()
        {
            return Ok(false);
        }
        let values = scope_candidates(&scope.element)?;
        let active = self.document.active_element();
        // Popups own their own finite keyboard navigation. The enclosing scope
        // only repairs a subsequent focus crossing; it does not steal Menu Tab.
        if active
            .as_ref()
            .is_some_and(|target| !scope.element.contains(Some(target)))
        {
            return Ok(false);
        }
        let current = values
            .iter()
            .position(|value| Some(value) == active.as_ref());
        let next = match (current, key.shift_key()) {
            (Some(index), false) if index + 1 < values.len() => Some(index + 1),
            (Some(index), true) if index > 0 => Some(index - 1),
            (_, true) if scope.config.loop_tab && !values.is_empty() => Some(values.len() - 1),
            (_, false) if scope.config.loop_tab && !values.is_empty() => Some(0),
            _ => None,
        };
        if let Some(index) = next {
            event.prevent_default();
            dom::focus(&values[index])?;
            return Ok(true);
        }
        if scope.config.contain_focus {
            event.prevent_default();
            if values.is_empty() {
                dom::focus(&scope.element)?;
            }
            return Ok(true);
        }
        Ok(false)
    }
}
