//! Same item exposed under two names via `pub use ... as ...`.
//! references for `Bar` must include the def site; references for `Baz`
//! must include the re-export site, both ultimately pointing to the same
//! type but via different name resolutions.

pub mod inner {
    pub struct Bar;
}

pub use inner::Bar as Baz;

pub fn make_bar() -> inner::Bar { inner::Bar }
pub fn make_baz() -> Baz { Baz }
