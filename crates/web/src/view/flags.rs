//! Boolean class sources retain a bool, not a formatted whole class string.
use super::*;
pub trait BoolValue: sealed::Flag + 'static {
    fn validate_bool(&self, runtime: RuntimeId) -> Result<()>;
    fn reactive(&self) -> bool {
        true
    }
    fn read_bool(&mut self) -> Result<bool>;
}
impl sealed::Flag for bool {}
impl BoolValue for bool {
    fn validate_bool(&self, _: RuntimeId) -> Result<()> {
        Ok(())
    }
    fn reactive(&self) -> bool {
        false
    }
    fn read_bool(&mut self) -> Result<bool> {
        Ok(*self)
    }
}
macro_rules! flag_handle {
    ($ty:ident) => {
        impl sealed::Flag for $ty<bool> {}
        impl BoolValue for $ty<bool> {
            fn validate_bool(&self, r: RuntimeId) -> Result<()> {
                TextValue::validate(self, r)
            }
            fn read_bool(&mut self) -> Result<bool> {
                self.try_get().map_err(error)
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
    fn read_bool(&mut self) -> Result<bool> {
        self.try_get().map_err(error)
    }
}
impl<F: FnMut() -> bool + 'static> sealed::Flag for Computed<F> {}
impl<F: FnMut() -> bool + 'static> BoolValue for Computed<F> {
    fn validate_bool(&self, _: RuntimeId) -> Result<()> {
        Ok(())
    }
    fn read_bool(&mut self) -> Result<bool> {
        Ok((self.0)())
    }
}
