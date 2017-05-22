use spin;

// take Rust lock and implement it?
struct SleepLock {
    locked: bool,
    pid: u32,
}

use core::sync::atomic::{AtomicBool, Ordering, ATOMIC_BOOL_INIT};
use core::cell::UnsafeCell;
use core::marker::Sync;
use core::ops::{Drop, Deref, DerefMut};
use core::fmt;
use core::option::Option::{self, None, Some};
use core::default::Default;

#[cfg(all(feature = "asm", any(target_arch = "x86", target_arch = "x86_64")))]
#[inline(always)]
pub fn cpu_relax() {
    // This instruction is meant for usage in spinlock loops
    // (see Intel x86 manual, III, 4.2)
    unsafe {
        asm!("pause" :::: "volatile");
    }
}

#[cfg(any(not(feature = "asm"), not(any(target_arch = "x86", target_arch = "x86_64"))))]
#[inline(always)]
pub fn cpu_relax() {}


pub struct SleepMutex<T: ?Sized> {
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

pub struct SleepMutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a AtomicBool,
    data: &'a mut T,
}

// Same unsafe impls as `std::sync::SleepMutex`
unsafe impl<T: ?Sized + Send> Sync for SleepMutex<T> {}
unsafe impl<T: ?Sized + Send> Send for SleepMutex<T> {}

impl<T> SleepMutex<T> {
    #[cfg(feature = "const_fn")]
    pub const fn new(user_data: T) -> SleepMutex<T> {
        SleepMutex {
            lock: ATOMIC_BOOL_INIT,
            data: UnsafeCell::new(user_data),
        }
    }

    #[cfg(not(feature = "const_fn"))]
    pub fn new(user_data: T) -> SleepMutex<T> {
        SleepMutex {
            lock: ATOMIC_BOOL_INIT,
            data: UnsafeCell::new(user_data),
        }
    }

    pub fn into_inner(self) -> T {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock.
        let SleepMutex { data, .. } = self;
        unsafe { data.into_inner() }
    }
}

impl<T: ?Sized> SleepMutex<T> {
    fn obtain_lock(&self) {
        while self.lock.compare_and_swap(false, true, Ordering::Acquire) != false {
            // Wait until the lock looks unlocked before retrying
            while self.lock.load(Ordering::Relaxed) {
                cpu_relax();
            }
        }
    }

    pub fn lock(&self) -> SleepMutexGuard<T> {
        self.obtain_lock();
        SleepMutexGuard {
            lock: &self.lock,
            data: unsafe { &mut *self.data.get() },
        }
    }

    pub unsafe fn force_unlock(&self) {
        self.lock.store(false, Ordering::Release);
    }

    pub fn try_lock(&self) -> Option<SleepMutexGuard<T>> {
        if self.lock.compare_and_swap(false, true, Ordering::Acquire) == false {
            Some(SleepMutexGuard {
                lock: &self.lock,
                data: unsafe { &mut *self.data.get() },
            })
        } else {
            None
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for SleepMutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => write!(f, "SleepMutex {{ data: {:?} }}", &*guard),
            None => write!(f, "SleepMutex {{ <locked> }}"),
        }
    }
}

impl<T: ?Sized + Default> Default for SleepMutex<T> {
    fn default() -> SleepMutex<T> {
        SleepMutex::new(Default::default())
    }
}

impl<'a, T: ?Sized> Deref for SleepMutexGuard<'a, T> {
    type Target = T;
    fn deref<'b>(&'b self) -> &'b T {
        &*self.data
    }
}

impl<'a, T: ?Sized> DerefMut for SleepMutexGuard<'a, T> {
    fn deref_mut<'b>(&'b mut self) -> &'b mut T {
        &mut *self.data
    }
}

impl<'a, T: ?Sized> Drop for SleepMutexGuard<'a, T> {
    fn drop(&mut self) {
        self.lock.store(false, Ordering::Release);
    }
}
