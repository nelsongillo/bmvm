pub struct Dirty<T> {
    inner: T,
    dirty: bool,
}

impl<T> Dirty<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            dirty: false,
        }
    }

    /// Get the underlying value
    pub fn get(&self) -> &T {
        &self.inner
    }

    /// Mutually get the underlying value and mark as dirty
    pub fn get_mut(&mut self) -> &mut T {
        self.dirty = true;
        &mut self.inner
    }

    /// Optionally mutate the underlying value and indicate its modification via the return value.
    ///
    /// # Note
    ///
    /// If the returned value does not match the actual action executed in the closure,
    /// the dirty state may not reflect the actual state, which could lead to erroneous behavior.
    pub fn mutate<M>(&mut self, m: M)
    where
        M: FnOnce(&mut T) -> bool,
    {
        self.dirty = m(&mut self.inner);
    }

    /// Set the underlying value, which will lead to the state to be marked dirty
    pub fn set(&mut self, value: T) {
        self.inner = value;
        self.dirty = true;
    }

    /// Clear the dirty state
    pub fn clear(&mut self) {
        self.dirty = false;
    }

    /// Explicitly mark as dirty
    pub fn mark(&mut self) {
        self.dirty = true;
    }

    /// Return the dirty state
    pub fn dirty(&self) -> bool {
        self.dirty
    }

    /// Executes the closure, if dirty without clearing it. Otherwise, does nothing
    pub fn if_dirty<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        if self.dirty {
            Some(f(&self.inner))
        } else {
            None
        }
    }

    /// Executes the closure, if dirty and cleans the flag. Otherwise, does nothing
    pub fn sync<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        if self.dirty {
            let r = f(&self.inner);
            self.clear();
            Some(r)
        } else {
            None
        }
    }
}
