use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::cell::RefCell;
use std::cell::Cell;
use std::alloc::{GlobalAlloc, Layout, System};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::borrow::Borrow;
use std::cmp::Ordering;

#[global_allocator]
static GLOBAL: System = System;

#[derive(Debug, Clone)]
struct RcBox<T: ?Sized> {
    strong: Cell<usize>,
    weak: Cell<usize>,
    none: Cell<bool>,
    value: T,
}

pub struct Rcn<T: ?Sized>{
    ptr: NonNull<RcBox<T>>,
    phantom: PhantomData<T>,
}

#[allow(dead_code)]
impl<T> Rcn<T> {

    pub fn new(data: T) -> Rcn<T> {
        Rcn::<T> {
            ptr: NonNull::new(Box::into_raw(Box::new(RcBox {
                strong: Cell::new(1),
                weak:  Cell::new(1),
                none:  Cell::new(false),
                value: data,
            }))).unwrap(),
            phantom: PhantomData,
        }
    }
}

#[allow(dead_code)]
impl<T: Default> Rcn<T> {

    pub fn none() -> Rcn<T> {
        Rcn::<T> {
            ptr: NonNull::new(RefCell::new(RcBox {
                strong: Cell::new(0),
                weak:  Cell::new(0),
                none:  Cell::new(true),
                value: Default::default(),
            }).as_ptr()).unwrap(),
            phantom: PhantomData,
        }
    }
}

#[allow(dead_code)]
impl<T: ?Sized> Rcn<T> {

    #[inline]
    pub fn weak_count(&self) -> usize {
        self.weak()
    }

    #[inline]
    pub fn strong_count(&self) -> usize {
        self.strong()
    }

    #[inline]
    fn strong(&self) -> usize {
        unsafe { self.ptr.as_ref().strong.get() }
    }

    #[inline]
    fn inc_strong(&self) {

        if self.strong() == usize::max_value() {
            panic!("abort inc strong");
        }
        unsafe { self.ptr.as_ref().strong.set(self.strong() + 1); }
    }

    #[inline]
    fn dec_strong(&self) {
        if self.strong() == usize::min_value(){
            panic!("abort dec strong");
        }

        unsafe { self.ptr.as_ref().strong.set(self.strong() - 1); }
    }

    #[inline]
    fn weak(&self) -> usize {
        unsafe { self.ptr.as_ref().weak.get() }
    }

    #[inline]
    fn inc_weak(&self) {
        if self.weak() == usize::max_value() {
            panic!("abort inc weak");
        }
        unsafe { self.ptr.as_ref().weak.set(self.weak() + 1);}
    }

    #[inline]
    fn dec_weak(&self) {
        if self.weak() == usize::min_value() {
            panic!("abort dec weak");
        }
        unsafe { self.ptr.as_ref().weak.set(self.weak() - 1); }
    }

    #[inline]
    pub fn is_unique(&self) -> bool {
        self.strong_count() == 1
    }

    
    #[inline]
    pub fn is_none(&self) -> bool {
        unsafe {
            if self.ptr.as_ref().none.get() {
                true
            } else {
                false
            }
        }
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        unsafe {
            if !self.ptr.as_ref().none.get() {
                true
            } else {
                false
            }
        }
    }

    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.ptr.as_ptr() == other.ptr.as_ptr()
    }

    #[inline]
    pub fn share(rc: &Rcn<T>) -> Rcn<T> {
        rc.inc_strong();
        Rcn {
            ptr: rc.ptr,
            phantom: PhantomData,
        }
    }

    #[inline]
    pub fn make_none(&self) {
        unsafe{
            self.ptr.as_ref().none.set(true)
        }
    }

    pub fn downgrade(&self) -> Weakn<T> {
        self.inc_weak();
        let address = self.ptr.as_ptr() as *mut () as usize;
        debug_assert!(address != usize::max_value());
        Weakn { ptr: self.ptr }
    }
}

#[allow(dead_code)]
impl<T: ?Sized + Clone> Rcn<T> {

    #[inline]
    pub fn set(&mut self, data: &T) {
        // if self.is_unique() {
            unsafe {
                self.ptr.as_mut().value = data.clone();
            }
        // }
    }

    #[inline]
    pub fn get(&self) -> T {
        unsafe {
            if !self.ptr.as_ref().none.get() {
                self.ptr.as_ref().value.clone()
            } else {
                panic!("Value is none!!!");
            }
             
        }
    }

    #[inline]
    pub fn take(&mut self) -> Option<T> {
        unsafe {
            if self.is_unique() {
                self.ptr.as_mut().strong.set(0);
                self.ptr.as_mut().weak.set(0);
                self.ptr.as_mut().none.set(true);
                Some(self.ptr.as_mut().value.clone())
            } else {
                None
            }
        }
    }
}

impl<T: ?Sized + Clone> Clone for Rcn<T> {
    #[inline]
    fn clone(&self) -> Rcn<T> {
        Rcn::new((**self).clone())
    }
}

impl <T: ?Sized> Drop for Rcn<T> {
    fn drop(&mut self) {
        // println!("x0 = s {}, w {}", self.strong_count(), self.weak_count());
        self.dec_strong();
        // println!("x1 = s {}, w {}", self.strong_count(), self.weak_count());
        if self.strong_count() == 0 {
            unsafe {
                ptr::drop_in_place(self.ptr.as_mut());
                self.ptr.as_ref().none.set(true);
            }
            // println!("x2 = s {}, w {}", self.strong_count(), self.weak_count());
            self.dec_weak();
            // println!("x3 = s {}, w {}", self.strong_count(), self.weak_count());
            if self.weak_count() == 0 {
                unsafe { GLOBAL.dealloc(self.ptr.as_ptr() as *mut u8, Layout::for_value(self.ptr.as_ref())); }
            }
        }
    }
}

impl<T: ?Sized> Deref for Rcn<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        unsafe {
            if !self.ptr.as_ref().none.get() {
                &self.ptr.as_ref().value
            } else {
                panic!("Deref of none value!!!");
            }
        }
    }
}

impl<T: ?Sized> DerefMut for Rcn<T> {
    
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            if !self.ptr.as_ref().none.get() {
                &mut self.ptr.as_mut().value
            } else {
                panic!("DerefMut of none value!!!");
            }
        }
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for Rcn<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Rcn<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: Default> Default for Rcn<T> {
    #[inline]
    fn default() -> Rcn<T> {
        Rcn::new(Default::default())
    }
}

impl<T: ?Sized + PartialEq> PartialEq for Rcn<T> {

    #[inline(always)]
    fn eq(&self, other: &Rcn<T>) -> bool {
        **self == **other
    }

    #[inline(always)]
    fn ne(&self, other: &Rcn<T>) -> bool {
        **self != **other
    }
}

impl<T: ?Sized + Eq> Eq for Rcn<T> {}

impl<T: ?Sized + PartialOrd> PartialOrd for Rcn<T> {

    #[inline(always)]
    fn partial_cmp(&self, other: &Rcn<T>) -> Option<Ordering> {
        (**self).partial_cmp(&**other)
    }

    #[inline(always)]
    fn lt(&self, other: &Rcn<T>) -> bool {
        **self < **other
    }

    #[inline(always)]
    fn le(&self, other: &Rcn<T>) -> bool {
        **self <= **other
    }

    #[inline(always)]
    fn gt(&self, other: &Rcn<T>) -> bool {
        **self > **other
    }

    #[inline(always)]
    fn ge(&self, other: &Rcn<T>) -> bool {
        **self >= **other
    }
}

impl<T: ?Sized> Borrow<T> for Rcn<T> {
    fn borrow(&self) -> &T {
        &**self
    }
}

impl<T: ?Sized> AsRef<T> for Rcn<T> {
    fn as_ref(&self) -> &T {
        &**self
    }
}

impl<T: ?Sized> fmt::Pointer for Rcn<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&(&**self as *const T), f)
    }
}

impl<T> From<T> for Rcn<T> {
    fn from(t: T) -> Self {
        Rcn::new(t)
    }
}

#[allow(dead_code)]
pub struct Weakn<T: ?Sized> {
    ptr: NonNull<RcBox<T>>,
}

impl<T> Weakn<T> {
    pub fn new() -> Weakn<T> {
        Weakn {
            ptr: NonNull::new(usize::max_value as *mut RcBox<T>).expect("MAX is not 0"),
        }
    }
}

#[allow(dead_code)]
impl<T: ?Sized> Weakn<T> {

    #[inline]
    fn strong(&self) -> usize {
        unsafe { self.ptr.as_ref().strong.get() }
    }

    #[inline]
    fn inc_strong(&self) {

        if self.strong() == usize::max_value() {
            panic!("abort inc strong");
        }
        unsafe { self.ptr.as_ref().strong.set(self.strong() + 1); }
    }

    #[inline]
    fn dec_strong(&self) {
        if self.strong() == usize::min_value(){
            panic!("abort dec strong");
        }

        unsafe { self.ptr.as_ref().strong.set(self.strong() - 1); }
    }

    #[inline]
    fn weak(&self) -> usize {
        unsafe { self.ptr.as_ref().weak.get() }
    }

    #[inline]
    fn inc_weak(&self) {
        if self.weak() == usize::max_value() {
            panic!("abort inc weak");
        }
        unsafe { self.ptr.as_ref().weak.set(self.weak() + 1);}
    }

    #[inline]
    fn dec_weak(&self) {
        if self.weak() == usize::min_value() {
            panic!("abort dec weak");
        }
        unsafe { self.ptr.as_ref().weak.set(self.weak() - 1); }
    }

    #[inline]
    pub fn share(weak: &Weakn<T>) -> Weakn<T> {
        weak.inc_weak();
        Weakn { ptr: weak.ptr, }
    }

    pub fn upgrade(&self) -> Option<Rcn<T>> {
        unsafe { 
            if self.ptr.as_ref().strong.get() == 0 {
                return None
            }
        }
        self.inc_strong();
        Some(Rcn { ptr: self.ptr, phantom: PhantomData })
    }
}

impl<T: ?Sized + Clone> Weakn<T> {
    #[inline]
    pub fn get(&self) -> T {
        unsafe {
            if !self.ptr.as_ref().none.get() {
                self.ptr.as_ref().value.clone()
            } else {
                panic!("Value is none!!!");
            }
             
        }
    }
}

impl<T: ?Sized> Drop for Weakn<T> {
    fn drop(&mut self) {
        self.dec_weak();
        if self.weak() == 0 {
            unsafe { GLOBAL.dealloc(self.ptr.cast::<u8>().as_ptr(), Layout::for_value(self.ptr.as_ref())); }
        }
    }
}

impl<T: ?Sized + Clone> Clone for Weakn<T> {
    #[inline]
    fn clone(&self) -> Weakn<T> {
        Weakn::new()
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Weakn<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(Weakn)")
    }
}

#[allow(unused_imports)]
#[cfg(test)]
mod test {

    use super::Rcn;
    use super::Weakn;
    use std::cell::RefCell;
    use std::time::Instant;

    use std::rc::Rc;

    #[test]
    fn strong_count_test() {
        let five = Rcn::new(5);
        assert_eq!(five.get(), 5);
        let num = Rcn::share(&five);
        assert_eq!(num.strong_count(), 2);
        assert_eq!(five.strong_count(), 2);
        drop(num);
        assert_eq!(five.strong_count(), 1);

        let mut x = Rcn::new(RefCell::new(5));
        let y = Rcn::share(&x);
        x.set(&RefCell::new(20));  
        assert_eq!(y.get(), RefCell::new(20));

        let mut a: i32 = 100;
        let rc1: Rcn<i32> = Rcn::new(a);
        assert_eq!(rc1.get(), 100);
        {
            a = 1000;
        }
        assert_eq!(a, 1000);
        assert_eq!(rc1.get(), 100);

        let mut rc2: Rcn<i32> = Rcn::new(0);
        assert_eq!(rc2.get(), 0);
        {
            let a: i32 = 100;
            rc2 = Rcn::new(a);
        }
        assert_eq!(rc2.get(), 100);

        let x = Rcn::new(5);
        assert_eq!(*x, 5);

        let x = Rcn::new(5);
        let y = Rcn::share(&x);
        assert_eq!(*x, 5);
        assert_eq!(*y, 5);
    }

    #[test]
    fn deref_test() {
        let x: Rcn<Box<_>> = Rcn::new(Box::new(5));
        assert_eq!(**x, 5);
    }

    #[test]
    fn unique_test() {
        let x = Rcn::new(3);
        assert!(x.is_unique());
        let y = Rcn::share(&x);
        assert!(!x.is_unique());
        drop(y);
        assert!(x.is_unique());
    }

    #[test]
    fn clone_test() {
        let mut cow0 = Rcn::new(75);
        let mut cow1 = cow0.clone();
        let mut cow2 = cow1.clone();
        assert!(75 == cow0.get());
        assert!(75 == cow1.get());
        assert!(75 == cow2.get());
        *cow0 += 1;
        *cow1 += 2;
        *cow2 += 3;
        assert!(76 == *cow0);
        assert!(77 == *cow1);
        assert!(78 == *cow2);
        assert!(*cow0 != *cow1);
        assert!(*cow0 != *cow2);
        assert!(*cow1 != *cow2);

        let mut cow0 = Rcn::new(75);
        let cow1 = cow0.clone();
        let cow2 = Rcn::share(&cow1);
        assert!(75 == *cow0);
        assert!(75 == *cow1);
        assert!(75 == *cow2);
        *cow0 += 1;
        assert!(76 == *cow0);
        assert!(75 == *cow1);
        assert!(75 == *cow2);
        assert!(*cow0 != *cow1);
        assert!(*cow0 != *cow2);
        assert!(*cow1 == *cow2);
    }

    #[test]
    fn debug_display_test() {
        let foo = Rcn::new(75);
        assert_eq!(format!("{:?}", foo), "75");
        assert_eq!(format!("{}", foo), "75");
    }

    #[test]
    fn auto_ref_test() {
        let foo: Rcn<i32> = Rcn::new(10);
        assert_eq!(foo, Rcn::share(&foo));
    }

    #[test]
    fn partialord_test() {
        let five = Rcn::new(5);
        let same_five = Rcn::share(&five);
        let other_five = Rcn::new(5);
        assert!(Rcn::ptr_eq(&five, &same_five));
        assert!(!Rcn::ptr_eq(&five, &other_five));
    }

    #[test]
    fn down_up_grade_some_test() {
        let x = Rcn::new(5);
        let y = x.downgrade();
        assert!(y.upgrade().is_some());
    }

    #[test]
    fn down_up_grade_none_test() {
        let x = Rcn::new(5);
        let y = x.downgrade();
        drop(x);
        assert!(y.upgrade().is_none());
    }

}