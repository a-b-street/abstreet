use std::any::Any;

// Trick to make a cloneable Any from
// https://stackoverflow.com/questions/30353462/how-to-clone-a-struct-storing-a-boxed-trait-object/30353928#30353928.

pub trait Cloneable: CloneableImpl {}

pub trait CloneableImpl {
    fn clone_box(&self) -> Box<dyn Cloneable>;
    fn as_any(&self) -> &dyn Any;
}

impl<T> CloneableImpl for T
where
    T: 'static + Cloneable + Clone,
{
    fn clone_box(&self) -> Box<dyn Cloneable> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clone for Box<dyn Cloneable> {
    fn clone(&self) -> Box<dyn Cloneable> {
        self.clone_box()
    }
}

impl Cloneable for () {}
impl Cloneable for bool {}
impl Cloneable for usize {}
impl Cloneable for f64 {}
impl Cloneable for String {}
impl Cloneable for Option<String> {}
impl Cloneable for (String, Box<dyn Cloneable>) {}
