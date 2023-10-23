use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::{Truck, Widget};

cfg_feature! {
    #![not(feature = "__single_holder")]

    use std::sync::atomic::{Ordering, AtomicU64};
    use std::fmt::Display;

    static NEXT_HOLDER_ID: AtomicU64 = AtomicU64::new(1);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    pub struct HolderId(u64);

    impl HolderId {
        pub fn null() -> Self {
            Self(0)
        }
        pub fn is_null(&self) -> bool {
            self.0 == 0
        }

        pub fn next() -> Self {
            Self(NEXT_HOLDER_ID.fetch_add(1, Ordering::Relaxed))
        }
    }
    impl Display for HolderId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "#{}", self.0)
        }
    }
    impl AsRef<HolderId> for HolderId {
        fn as_ref(&self) -> &HolderId {
            self
        }
    }
    impl Default for HolderId {
        fn default() -> Self {
            Self::null()
        }
    }
}

pub trait Holder: fmt::Debug + 'static {
    fn mount(self, widget: impl Widget + 'static) -> Self
    where
        Self: Sized;
    fn enable(self, enabler: impl Enabler + 'static) -> Self
    where
        Self: Sized,
    {
        enabler.enable(self.truck());
        self
    }
    fn truck(&self) -> Rc<RefCell<Truck>>;
    // fn clone_boxed(&self) -> Box<dyn Holder>;
}

pub trait Enabler: fmt::Debug + 'static {
    fn enable(self, truck: Rc<RefCell<Truck>>);
}
