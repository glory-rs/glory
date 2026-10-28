//! Boolean class sources retain a bool, not a formatted whole class string.
use super::*;
pub trait BoolValue: sealed::Flag + 'static {
    fn validate_bool(&self, runtime: RuntimeId) -> Result<()>;
    fn reactive(&self) -> bool {
        true
    }
    fn read_bool(&mut self) -> bool;
    #[cfg(target_arch = "wasm32")]
    fn install_bool(
        mut self,
        instance: &crate::template::Instance,
        owner: &Scope,
        slot: u16,
    ) -> Result<()>
    where
        Self: Sized + 'static,
    {
        if self.reactive() {
            instance.bind_flag(owner, slot, move || self.read_bool())
        } else {
            let next = self.read_bool();
            if next != instance.flag_initial(slot)? {
                instance.set_flag(slot, next)?;
            }
            Ok(())
        }
    }
}
impl sealed::Flag for bool {}
impl BoolValue for bool {
    fn validate_bool(&self, _: RuntimeId) -> Result<()> {
        Ok(())
    }
    fn reactive(&self) -> bool {
        false
    }
    fn read_bool(&mut self) -> bool {
        *self
    }
}
macro_rules! flag_handle {
    ($ty:ident) => {
        impl sealed::Flag for $ty<bool> {}
        impl BoolValue for $ty<bool> {
            fn validate_bool(&self, r: RuntimeId) -> Result<()> {
                TextValue::validate(self, r)
            }
            fn read_bool(&mut self) -> bool {
                self.get()
            }
        }
    };
}
flag_handle!(Cage);
flag_handle!(Bond);
impl sealed::Flag for Vary<bool> {}
impl BoolValue for Vary<bool> {
    fn validate_bool(&self, r: RuntimeId) -> Result<()> {
        TextValue::validate(self, r)
    }
    fn reactive(&self) -> bool {
        self.is_reactive()
    }
    fn read_bool(&mut self) -> bool {
        self.get()
    }
}
impl<F: FnMut() -> bool + 'static> sealed::Flag for Computed<F> {}
impl<F: FnMut() -> bool + 'static> BoolValue for Computed<F> {
    fn validate_bool(&self, _: RuntimeId) -> Result<()> {
        Ok(())
    }
    fn read_bool(&mut self) -> bool {
        (self.0)()
    }
}
