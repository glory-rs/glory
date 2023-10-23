use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::{self, Formatter};

/// Truck is for store temp data of current request. Each handler can read or write data to it.
///
/// # Example
///
/// ```no_run
/// use salvo_core::prelude::*;
///
/// #[handler]
/// async fn set_user(truck: &mut Truck) {
///     truck.insert("user", "client");
///     ctrl.call_next(req, truck, res).await;
/// }
/// #[handler]
/// async fn hello(truck: &mut Truck) -> String {
///     format!("Hello {}", truck.get::<&str>("user").map(|s|*s).unwrap_or_default())
/// }
/// #[tokio::main]
/// async fn main() {
///     let router = Router::new().hoop(set_user).handle(hello);
///     let acceptor = TcpListener::new("127.0.0.1:5800").bind().await;
///     Server::new(acceptor).serve(router).await;
/// }
/// ```

#[derive(Default)]
pub struct Truck {
    map: HashMap<String, Box<dyn Any>>,
}

#[inline]
fn type_key<T: 'static>() -> String {
    format!("{:?}", TypeId::of::<T>())
}

impl Truck {
    /// Creates an empty `Truck`.
    ///
    /// The truck is initially created with a capacity of 0, so it will not allocate until it is first inserted into.
    #[inline]
    pub fn new() -> Truck {
        Truck { map: HashMap::new() }
    }
    /// Get reference to truck inner map.
    #[inline]
    pub fn inner(&self) -> &HashMap<String, Box<dyn Any>> {
        &self.map
    }

    /// Creates an empty `Truck` with the specified capacity.
    ///
    /// The truck will be able to hold at least capacity elements without reallocating. If capacity is 0, the truck will not allocate.
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Truck {
            map: HashMap::with_capacity(capacity),
        }
    }
    /// Returns the number of elements the truck can hold without reallocating.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.map.capacity()
    }

    /// Inject a value into the truck.
    #[inline]
    pub fn inject<V: Any>(&mut self, value: V) -> &mut Self {
        self.map.insert(type_key::<V>(), Box::new(value));
        self
    }
    /// Obtain a reference to a value previous inject to the truck.
    ///
    /// Returns `Err(None)` if value is not present in truck.
    /// Returns `Err(Some(Box<dyn Any>))` if value is present in truck but downcast failed.
    #[inline]
    pub fn obtain<T: Any>(&self) -> Result<&T, Option<&Box<dyn Any>>> {
        self.get(&type_key::<T>())
    }

    /// Obtain a mutable reference to a value previous inject to the truck.
    ///
    /// Returns `Err(None)` if value is not present in truck.
    /// Returns `Err(Some(Box<dyn Any>))` if value is present in truck but downcast failed.
    #[inline]
    pub fn obtain_mut<T: Any>(&mut self) -> Result<&mut T, Option<&mut Box<dyn Any>>> {
        self.get_mut(&type_key::<T>())
    }

    /// Inserts a key-value pair into the truck.
    #[inline]
    pub fn insert<K, V>(&mut self, key: K, value: V) -> &mut Self
    where
        K: Into<String>,
        V: Any,
    {
        self.map.insert(key.into(), Box::new(value));
        self
    }

    /// Check is there a value stored in truck with this key.
    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }
    /// Check is there a value stored in truck.
    #[inline]
    pub fn contains<T: Any>(&self) -> bool {
        self.map.contains_key(&type_key::<T>())
    }

    /// Immutably borrows value from truck.
    ///
    /// Returns `Err(None)` if value is not present in truck.
    /// Returns `Err(Some(Box<dyn Any>))` if value is present in truck but downcast failed.
    #[inline]
    pub fn get<V: Any>(&self, key: &str) -> Result<&V, Option<&Box<dyn Any>>> {
        if let Some(value) = self.map.get(key) {
            value.downcast_ref::<V>().ok_or(Some(value))
        } else {
            Err(None)
        }
    }

    /// Mutably borrows value from truck.
    ///
    /// Returns `Err(None)` if value is not present in truck.
    /// Returns `Err(Some(Box<dyn Any>))` if value is present in truck but downcast failed.
    #[inline]
    pub fn get_mut<V: Any>(&mut self, key: &str) -> Result<&mut V, Option<&mut Box<dyn Any>>> {
        if let Some(value) = self.map.get_mut(key) {
            if value.downcast_mut::<V>().is_some() {
                return Ok(value.downcast_mut::<V>().unwrap());
            } else {
                Err(Some(value))
            }
        } else {
            Err(None)
        }
    }

    /// Remove value from truck and returning the value at the key if the key was previously in the truck.
    #[inline]
    pub fn remove<V: Any>(&mut self, key: &str) -> Result<V, Option<Box<dyn Any>>> {
        if let Some(value) = self.map.remove(key) {
            value.downcast::<V>().map(|b| *b).map_err(Some)
        } else {
            Err(None)
        }
    }

    /// Delete the key from truck, if the key is not present, return `false`.
    #[inline]
    pub fn delete(&mut self, key: &str) -> bool {
        self.map.remove(key).is_some()
    }

    /// Remove value from truck and returning the value if the type was previously in the truck.
    #[inline]
    pub fn scrape<T: Any>(&mut self) -> Result<T, Option<Box<dyn Any>>> {
        self.remove(&type_key::<T>())
    }
}

impl fmt::Debug for Truck {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Truck").field("keys", &self.map.keys()).finish()
    }
}

#[cfg(test)]
mod test {
    use crate::prelude::*;
    use crate::test::{ResponseExt, TestClient};

    use super::*;

    #[test]
    fn test_truck() {
        let mut truck = Truck::with_capacity(6);
        assert!(truck.capacity() >= 6);

        truck.insert("one", "ONE".to_owned());
        assert!(truck.contains_key("one"));

        assert_eq!(truck.get::<String>("one").unwrap(), &"ONE".to_owned());
        assert_eq!(truck.get_mut::<String>("one").unwrap(), &mut "ONE".to_owned());
    }

    #[test]
    fn test_transfer() {
        let mut truck = Truck::with_capacity(6);
        truck.insert("one", "ONE".to_owned());

        let truck = truck.transfer();
        assert_eq!(truck.get::<String>("one").unwrap(), &"ONE".to_owned());
    }
}
