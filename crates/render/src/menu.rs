//! Finite popup-menu commands and physical pointer boundary observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCommand {
    First,
    Last,
    Open,
    Close,
    Exit { backwards: bool },
    SearchNext,
    SearchPrevious,
    SearchCommit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    Enter,
    Leave,
    /// Actual mouse/pen movement, distinct from layout-induced boundary entry.
    Move,
}

impl crate::KeyboardInput<'_> {
    /// Only a declared menu search editor opts in. Preserve ordinary editing,
    /// modifier shortcuts, IME and horizontal/Home/End caret movement.
    pub fn menu_search(self) -> Option<MenuCommand> {
        if self.composing || self.control || self.meta || self.alt || self.shift {
            return (self.key == "Tab"
                && self.shift
                && !self.composing
                && !self.control
                && !self.meta
                && !self.alt
                && !self.repeat)
                .then_some(MenuCommand::Exit { backwards: true });
        }
        match self.key {
            "ArrowDown" => Some(MenuCommand::SearchNext),
            "ArrowUp" => Some(MenuCommand::SearchPrevious),
            "Enter" if !self.repeat => Some(MenuCommand::SearchCommit),
            "Tab" if !self.repeat => Some(MenuCommand::Exit { backwards: false }),
            _ => None,
        }
    }
    pub fn menu_check_activation(self) -> bool {
        self.key == "Enter"
            && !self.repeat
            && !self.composing
            && !self.editable
            && !self.control
            && !self.meta
            && !self.alt
            && !self.shift
    }
    pub fn menu(self, direction: crate::Direction, trigger: bool) -> Option<MenuCommand> {
        if self.composing || self.editable || self.control || self.meta || self.alt {
            return None;
        }
        if self.key == "Tab" && !trigger && !self.repeat {
            return Some(MenuCommand::Exit {
                backwards: self.shift,
            });
        }
        if self.shift {
            return None;
        }
        match (trigger, direction, self.key) {
            (true, _, "ArrowDown") => Some(MenuCommand::First),
            (true, _, "ArrowUp") => Some(MenuCommand::Last),
            (false, crate::Direction::Ltr, "ArrowRight")
            | (false, crate::Direction::Rtl, "ArrowLeft") => Some(MenuCommand::Open),
            (false, crate::Direction::Ltr, "ArrowLeft")
            | (false, crate::Direction::Rtl, "ArrowRight") => Some(MenuCommand::Close),
            _ => None,
        }
    }
}

impl crate::Semantics {
    pub(crate) fn validate_menu_search(
        &self,
        node: crate::DescriptorView<'_>,
    ) -> Result<(), crate::Error> {
        let Some(owner) = &self.menu_search else {
            return Ok(());
        };
        if !matches!(node.props, crate::Props::Input(input) if !input.multiline && input.purpose == crate::Purpose::Search)
            || self.id.is_none()
            || self.id.as_ref() == Some(owner)
            || self.controls.as_ref() != Some(owner)
            || self.active_descendant == self.id
            || self.combobox.is_some()
            || self.popup.is_some()
            || self.expanded.is_some()
            || self.menu_item.is_some()
            || self.option.is_some()
            || self.tags_input.is_some()
            || self.form_control.is_some()
            || self.layer.is_some()
            || self.navigation.is_some()
        {
            return Err(crate::Error::InvalidProps(
                "menu search requires an independent named single-line search editor and distinct Menu owner",
            ));
        }
        Ok(())
    }
}

impl crate::memory::Memory {
    fn nearest_menu(
        &self,
        mut cursor: Option<crate::NodeId>,
    ) -> Result<Option<crate::NodeId>, crate::Error> {
        while let Some(id) = cursor {
            let node = self.node(id)?;
            if node.descriptor.view().role == Some(crate::Role::Menu) {
                return Ok(Some(id));
            }
            cursor = node.parent;
        }
        Ok(None)
    }

    /// Resolve against current ancestry and live names, with no menu registry.
    pub fn menu_search_owner(
        &self,
        input: crate::NodeId,
    ) -> Result<Option<crate::NodeId>, crate::Error> {
        let node = self.node(input)?;
        let Some(name) = node
            .descriptor
            .view()
            .semantics
            .as_ref()
            .and_then(|s| s.menu_search.as_ref())
        else {
            return Ok(None);
        };
        let Some(owner) = self.named(name) else {
            return Ok(None);
        };
        Ok((self.active(input)
            && self.active(owner)
            && self.enabled(input)
            && !self.inert(input)
            && self.nearest_menu(node.parent)? == Some(owner))
        .then_some(owner))
    }

    pub(crate) fn resolve_menu_active_descendant(
        &self,
        input: crate::NodeId,
    ) -> Result<Option<crate::NodeId>, crate::Error> {
        let Some(owner) = self.menu_search_owner(input)? else {
            return Ok(None);
        };
        let Some(active) = self
            .node(input)?
            .descriptor
            .view()
            .semantics
            .as_ref()
            .and_then(|s| s.active_descendant.as_ref())
            .and_then(|name| self.named(name))
        else {
            return Ok(None);
        };
        let target = self.node(active)?;
        let descriptor = target.descriptor.view();
        Ok((self.active(active)
            && self.enabled(active)
            && !self.inert(active)
            && descriptor.activation_enabled()
            && self.nearest_menu(target.parent)? == Some(owner)
            && descriptor
                .semantics
                .as_ref()
                .and_then(|s| s.menu_item.as_ref())
                .is_some_and(|item| self.named(&item.owner) == Some(owner)))
        .then_some(active))
    }
}
