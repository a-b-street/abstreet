use std::any::Any;

/// Trick to make a cloneable Any from
/// https://stackoverflow.com/questions/30353462/how-to-clone-a-struct-storing-a-boxed-trait-object/30353928#30353928.
pub trait CloneableAny: CloneableImpl {}

pub trait CloneableImpl {
    fn clone_box(&self) -> Box<dyn CloneableAny>;
    fn as_any(&self) -> &dyn Any;
}

impl<T> CloneableImpl for T
where
    T: 'static + CloneableAny + Clone,
{
    fn clone_box(&self) -> Box<dyn CloneableAny> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Clone for Box<dyn CloneableAny> {
    fn clone(&self) -> Box<dyn CloneableAny> {
        self.clone_box()
    }
}

impl<T: 'static + Clone> CloneableAny for Vec<T> {}
