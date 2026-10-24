//! Navigation disclosure commands, separate from action-menu roles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationPanel {
    Trigger,
    Content,
}
impl NavigationPanel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Trigger => "trigger",
            Self::Content => "content",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavigationPanelCommand {
    First,
    Last,
    Open,
    Close,
}
impl crate::KeyboardInput<'_> {
    /// `inside` means the physical control belongs to a mounted content level.
    /// Tab stays native; hover and Escape/outside dismissal use separate protocols.
    pub fn navigation_panel(
        self,
        direction: crate::Direction,
        trigger: bool,
        inside: bool,
    ) -> Option<NavigationPanelCommand> {
        if self.composing || self.editable || self.control || self.meta || self.alt || self.shift {
            return None;
        }
        match (trigger, inside, direction, self.key) {
            (true, false, _, "ArrowDown") => Some(NavigationPanelCommand::First),
            (true, false, _, "ArrowUp") => Some(NavigationPanelCommand::Last),
            (true, true, crate::Direction::Ltr, "ArrowRight")
            | (true, true, crate::Direction::Rtl, "ArrowLeft") => {
                Some(NavigationPanelCommand::Open)
            }
            (_, true, crate::Direction::Ltr, "ArrowLeft")
            | (_, true, crate::Direction::Rtl, "ArrowRight") => Some(NavigationPanelCommand::Close),
            _ => None,
        }
    }
}
