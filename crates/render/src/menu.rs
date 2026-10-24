//! Finite popup-menu commands and physical pointer boundary observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuCommand {
    First,
    Last,
    Open,
    Close,
    Exit { backwards: bool },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    Enter,
    Leave,
    /// Actual mouse/pen movement, distinct from layout-induced boundary entry.
    Move,
}

impl crate::KeyboardInput<'_> {
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
