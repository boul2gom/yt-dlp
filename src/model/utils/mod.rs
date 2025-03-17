//! Utility functions for the model module.

use std::clone::Clone;
use std::cmp::{Eq, PartialEq};
use std::fmt::{Debug, Display};
use std::hash::Hash;

pub mod serde;

/// Trait that combines the basic traits that any structure should implement.
///
/// This trait combines:
/// - `Debug` for debug display
/// - `Clone` for duplication
/// - `PartialEq` for comparison
/// - `Display` for formatted display
pub trait CommonTraits: Debug + Clone + PartialEq + Display {}

/// Trait that combines basic and advanced traits for a complete structure.
///
/// This trait combines:
/// - All traits from `CommonTraits`
/// - `Eq` for total equality
/// - `Hash` for hashing
pub trait AllTraits: CommonTraits + Eq + Hash {}

/// Automatic implementation of the `CommonTraits` trait for any type that implements
/// the required traits.
impl<T: Debug + Clone + PartialEq + Display> CommonTraits for T {}

/// Automatic implementation of the `AllTraits` trait for any type that implements
/// the required traits.
impl<T: Debug + Clone + PartialEq + Display + Eq + Hash> AllTraits for T {}
