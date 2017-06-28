use spin;
use core::ops::{Deref, DerefMut};
use core::sync::atomic;
use traps;

use x86::shared::irq;
use x86::shared::flags;

// NB This should be a per CPU variable.  Not that it matters much for a
// uniprocessor system, but if that were to change then this needs updating
pub static mut LOCK_COUNT: atomic::AtomicUsize = atomic::AtomicUsize::new(0);

/// A wrapper class for `spin::Mutex` that enables and disables interrupts as needed
pub struct Mutex<T: ?Sized> {
    lock: spin::Mutex<T>,
}

pub struct MutexGuard<'a, T: ?Sized + 'a> {
    guard: Option<spin::MutexGuard<'a, T>>,
}

unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

impl<T> Mutex<T> {
    pub const fn new(user_data: T) -> Mutex<T> {
        Mutex { lock: spin::Mutex::new(user_data) }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        unsafe {
            // increment counter describing number of current locks
            // save whether interrupts were previously enabled or not, so we don't accidentally
            // enable them when we shouldn't
            let int_enabled = flags::flags().contains(flags::FLAGS_IF);
            irq::disable();
            if LOCK_COUNT.load(atomic::Ordering::SeqCst) == 0 {
                traps::INT_ENABLED.store(int_enabled, atomic::Ordering::SeqCst);
            }
            LOCK_COUNT.fetch_add(1, atomic::Ordering::SeqCst);
        }
        let g = self.lock.lock();
        MutexGuard { guard: Some(g) }
    }

    pub unsafe fn force_unlock(&self) {
        self.lock.force_unlock()
    }
}

impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
    type Target = T;
    fn deref<'b>(&'b self) -> &'b T {
        &*self.guard.as_ref().unwrap()
    }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    fn deref_mut<'b>(&'b mut self) -> &'b mut T {
        &mut *(self.guard.as_mut().unwrap())
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    /// The dropping of the MutexGuard will release the lock it was created from.
    fn drop(&mut self) {
        let g = self.guard.take();
        drop(g);
        atomic::fence(atomic::Ordering::SeqCst);
        unsafe {
            LOCK_COUNT.fetch_sub(1, atomic::Ordering::SeqCst);
            // if we *can* enable interrupts, based on total number of outstanding locks
            if LOCK_COUNT.load(atomic::Ordering::SeqCst) == 0 {
                // if our OS *wants* interrupts enabled
                if traps::INT_ENABLED.load(atomic::Ordering::Acquire) == true {
                    irq::enable();
                }
            }
        }
    }
}
