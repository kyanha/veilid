use super::*;

pub struct SingleShotEventual<T>
where
    T: Unpin + Clone,
{
    eventual: EventualValueClone<T>,
    drop_value: T,
}

impl<T> Drop for SingleShotEventual<T>
where
    T: Unpin + Clone,
{
    fn drop(&mut self) {
        self.eventual.resolve(self.drop_value.clone());
    }
}

impl<T> SingleShotEventual<T>
where
    T: Unpin + Clone,
{
    pub fn new(drop_value: T) -> Self {
        Self {
            eventual: EventualValueClone::new(),
            drop_value: drop_value,
        }
    }

    // Can only call this once, it consumes the eventual
    pub fn resolve(self, value: T) -> EventualResolvedFuture<EventualValueClone<T>> {
        self.eventual.resolve(value)
    }

    pub fn instance(&self) -> EventualValueCloneFuture<T> {
        self.eventual.instance()
    }
}
