use std::marker::PhantomData;
use std::ptr::{self, NonNull};
use std::cell::Cell;
#[allow(unused_imports)]
use std::alloc::{GlobalAlloc, Layout, System, handle_alloc_error};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::borrow::Borrow;
use std::cmp::Ordering;
#[allow(unused_imports)]
use std::any::Any;

// #[global_allocator]
// static GLOBAL: System = System;

#[derive(Debug, Clone)]
struct RcBox<T> {
    strong: Cell<usize>,
    weak: Cell<usize>,
    value: Option<T>,
}

/// A single-threaded reference-counting pointer with none value. 'Rcn' stands for 'Reference Counted with None'.
pub struct Rcn<T>{
    ptr: NonNull<RcBox<T>>,
    phantom: PhantomData<T>,
}

#[allow(dead_code)]
impl<T> Rcn<T> {
    /// Constructs a new `Rcn<T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let ten = Rcn::new(10);
    /// ```
    pub fn new(data: T) -> Rcn<T> {
        Rcn::<T> {
            ptr: NonNull::new(Box::into_raw(Box::new(RcBox {
                strong: Cell::new(1),
                weak:  Cell::new(0),
                value: Some(data),
            }))).unwrap(),
            phantom: PhantomData,
        }
    }

    pub fn none() -> Rcn<T> {
        Rcn::<T> {
            ptr: NonNull::new(Box::into_raw(Box::new(RcBox {
                strong: Cell::new(0),
                weak:  Cell::new(0),
                value: None,
            }))).unwrap(),
            phantom: PhantomData,
        }
    }
}

#[allow(dead_code)]
impl<T> Rcn<T> {

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
        self.weak_count() == 0 && self.strong_count() == 1
    }

    
    #[inline]
    pub fn is_none(&self) -> bool {
        unsafe {
            self.ptr.as_ref().value.is_none()
        }
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        unsafe {
            self.ptr.as_ref().value.is_some()
        }
    }

    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.ptr.as_ptr() == other.ptr.as_ptr()
    }

    #[inline]
    pub fn share(&self) -> Rcn<T> {
        if self.is_some() {
            self.inc_strong();
            Rcn {
                ptr: self.ptr,
                phantom: PhantomData,
            }
        } else {
            panic!("share of Rcn with none value");
        }
    }

    #[inline]
    pub fn make_none(&mut self) {
        unsafe{
            self.ptr.as_mut().value = None; 
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
impl<T: Clone> Rcn<T> {

    #[inline(always)]
    pub fn set(&mut self, data: &T) {
        if self.is_some() {
            unsafe {
                self.ptr.as_mut().value = Some(data.clone());
            }
        } else {
            panic!("write (set) in none rcn!\n \t help: Use Rcn:new(...) to none pointers");
        }
    }

    #[inline]
    pub fn take(&mut self) -> Option<T> {
        unsafe {
            if self.is_unique() {
                self.ptr.as_mut().strong.set(0);
                self.ptr.as_mut().weak.set(0);
                self.ptr.as_mut().value.take()
            } else {
                None
            }
        }
    }
}

impl<T: Clone> Clone for Rcn<T> {
    #[inline]
    fn clone(&self) -> Rcn<T> {
        if self.is_some() {
            
            unsafe {
                Rcn::<T> {
                    ptr: NonNull::new(Box::into_raw(Box::new(RcBox {
                        strong: Cell::new(1),
                        weak:  Cell::new(0),
                        value: self.ptr.as_ref().value.clone(),
                    }))).unwrap(),
                    phantom: PhantomData,
                }
            }
        } else {
            Rcn::none()
        }
        
    }
}

impl <T> Drop for Rcn<T> {
    fn drop(&mut self) {
        if self.is_some() {
            self.dec_strong();
        }
        if self.strong_count() == 0 {
            unsafe {
                // ptr::drop_in_place(self.ptr.as_mut());
                self.ptr.as_mut().value = None;
            }
            // self.dec_weak();
            if self.weak_count() == 0 {
                
            //     unsafe { println!("{:?} {:?}", self.ptr.as_ptr(), Layout::for_value(self.ptr.as_ref())); }
                unsafe { 
                    ptr::drop_in_place(self.ptr.as_mut()); 
                    // GLOBAL.dealloc(self.ptr.as_ptr() as *mut u8, Layout::for_value(self.ptr.as_ref())); 
                }
            //     unsafe { GLOBAL.dealloc(self.ptr.as_ptr() as *mut u8, Layout::from_size_align_unchecked(28, 8)); }
            }
        }
    }
}

impl<T> Deref for Rcn<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        if self.is_some() {
            unsafe {
                self.ptr.as_ref().value.as_ref().unwrap()
            }
        } else {
            panic!("deref of none rcn!");
        }
    }
}

impl<T> DerefMut for Rcn<T> {
    
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        if self.is_some() {
            unsafe {
                self.ptr.as_mut().value.as_mut().unwrap()
            }
        } else {
            panic!("deref_mut of none rcn!");
        }
    }
}

impl<T: fmt::Display> fmt::Display for Rcn<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: fmt::Debug> fmt::Debug for Rcn<T> {
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

impl<T: PartialEq> PartialEq for Rcn<T> {

    #[inline(always)]
    fn eq(&self, other: &Rcn<T>) -> bool {
        **self == **other
    }

    #[inline(always)]
    fn ne(&self, other: &Rcn<T>) -> bool {
        **self != **other
    }
}

impl<T: Eq> Eq for Rcn<T> {}

impl<T: PartialOrd> PartialOrd for Rcn<T> {

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

impl<T> Borrow<T> for Rcn<T> {
    fn borrow(&self) -> &T {
        &**self
    }
}

impl<T> AsRef<T> for Rcn<T> {
    fn as_ref(&self) -> &T {
        &**self
    }
}

impl<T> fmt::Pointer for Rcn<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&(&**self as *const T), f)
    }
}

impl<T> From<T> for Rcn<T> {
    fn from(t: T) -> Self {
        Rcn::new(t)
    }
}

// impl<T> From<Box<T>> for Rcn<T> {
//     #[inline]
//     fn from(v: Box<T>) -> Rcn<T> {
//         Rcn::from_box(v)
//     }
// }

#[allow(dead_code)]
pub struct Weakn<T> {
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
impl<T> Weakn<T> {

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
    pub fn share(&self) -> Weakn<T> {
        if self.is_some() {
            self.inc_weak();
            Weakn { ptr: self.ptr, }
        } else {
            panic!("share of Weakn with none value");
        }
        
    }

    #[inline]
    pub fn is_none(&self) -> bool {
        unsafe {
            self.ptr.as_ref().value.is_none()
        }
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        unsafe {
            self.ptr.as_ref().value.is_some()
        }
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

impl<T> Drop for Weakn<T> {
    fn drop(&mut self) {
        self.dec_weak();
        // if self.weak() == 0 {
        //     unsafe { GLOBAL.dealloc(self.ptr.cast::<u8>().as_ptr(), Layout::for_value(self.ptr.as_ref())); }
        // }
    }
}

impl<T: Clone> Clone for Weakn<T> {
    #[inline]
    fn clone(&self) -> Weakn<T> {
        Weakn::new()
    }
}

impl<T: fmt::Debug> fmt::Debug for Weakn<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(Weakn)")
    }
}

impl<T: PartialEq> PartialEq for Weakn<T> {

    #[inline(always)]
    fn eq(&self, other: &Weakn<T>) -> bool {
        **self == **other
    }

    #[inline(always)]
    fn ne(&self, other: &Weakn<T>) -> bool {
        **self != **other
    }
}

impl<T: Eq> Eq for Weakn<T> {}

impl<T: PartialOrd> PartialOrd for Weakn<T> {

    #[inline(always)]
    fn partial_cmp(&self, other: &Weakn<T>) -> Option<Ordering> {
        (**self).partial_cmp(&**other)
    }

    #[inline(always)]
    fn lt(&self, other: &Weakn<T>) -> bool {
        **self < **other
    }

    #[inline(always)]
    fn le(&self, other: &Weakn<T>) -> bool {
        **self <= **other
    }

    #[inline(always)]
    fn gt(&self, other: &Weakn<T>) -> bool {
        **self > **other
    }

    #[inline(always)]
    fn ge(&self, other: &Weakn<T>) -> bool {
        **self >= **other
    }
}

impl<T> Deref for Weakn<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        if self.is_some() {
            unsafe {
                self.ptr.as_ref().value.as_ref().unwrap()
            }
        } else {
            panic!("deref of none weakn!");
        }
    }
}

// unsafe fn set_data_ptr<T, U>(mut ptr: *mut T, data: *mut U) -> *mut T {
//     ptr::write(&mut ptr as *mut _ as *mut *mut u8, data as *mut u8);
//     ptr
// }

// impl<T> Rcn<T> {
//     // Allocates an `RcBox<T>` with sufficient space for an unsized value
//     unsafe fn allocate_for_ptr(ptr: *const T) -> *mut RcBox<T> {
//         // Create a fake RcBox to find allocation size and alignment
//         let fake_ptr = ptr as *mut RcBox<T>;

//         let layout = Layout::for_value(&*fake_ptr);

//         let mem = GLOBAL.alloc(layout);

//         // Initialize the real RcBox
//         let inner = set_data_ptr(ptr as *mut T, mem) as *mut RcBox<T>;

//         ptr::write(&mut (*inner).strong, Cell::new(1));
//         ptr::write(&mut (*inner).weak, Cell::new(1));

//         inner
//     }

//     fn from_box(v: Box<T>) -> Rcn<T> {
//         unsafe {
//             let bptr = Box::into_raw(v);
//             // let bptr = box_unique.as_ptr();

//             let value_size = size_of_val(&*bptr);
//             let ptr = Self::allocate_for_ptr(bptr);

//             ptr::copy_nonoverlapping(
//                 bptr as *const T as *const u8,
//                 &mut (*ptr).value as *mut _ as *mut u8,
//                 value_size);

//             Rcn { ptr: NonNull::new_unchecked(ptr), phantom: PhantomData }
//         }
//     }
// }

#[allow(unused_imports)]
#[cfg(test)]
mod test {

    use super::Rcn;
    use super::Weakn;
    use std::cell::RefCell;
    use std::time::Instant;

    use std::rc::Rc;
    use std::rc::Weak;

    #[test]
    fn rc_test() {
        let five = Rcn::new(5);
        assert_eq!(*five, 5);
        let num = five.share();
        assert_eq!(num.strong_count(), 2);
        assert_eq!(five.strong_count(), 2);
        drop(num);
        assert_eq!(five.strong_count(), 1);

        let mut x = Rcn::new(RefCell::new(5));
        let y = x.share();
        x.set(&RefCell::new(20));  
        assert_eq!(*y, RefCell::new(20));

        let mut a: i32 = 100;
        let rc1: Rcn<i32> = Rcn::new(a);
        assert_eq!(*rc1, 100);
        {
            a = 1000;
        }
        assert_eq!(a, 1000);
        assert_eq!(*rc1, 100);

        let mut rc2: Rcn<i32> = Rcn::new(0);
        assert_eq!(*rc2, 0);
        {
            let a: i32 = 100;
            rc2 = Rcn::new(a);
        }
        assert_eq!(*rc2, 100);

        let x = Rcn::new(5);
        assert_eq!(*x, 5);

        let x = Rcn::new(5);
        let y = x.share();
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
        let y = x.share();
        assert!(!x.is_unique());
        drop(y);
        assert!(x.is_unique());
    }

    #[test]
    fn clone_test() {
        let mut cow0 = Rcn::new(75);
        let mut cow1 = cow0.clone();
        let mut cow2 = cow1.clone();
        assert!(75 == *cow0);
        assert!(75 == *cow1);
        assert!(75 == *cow2);
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
        let cow2 = cow1.share();
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
        assert_eq!(foo, foo.share());
    }

    #[test]
    fn partialord_test() {
        let five = Rcn::new(5);
        let same_five = five.share();
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

    #[test]
    fn strong_count_test() {
        let a = Rcn::new(0);
        assert!(Rcn::strong_count(&a) == 1);
        let w = Rcn::downgrade(&a);
        assert!(Rcn::strong_count(&a) == 1);
        let b = w.upgrade().expect("upgrade of live rc failed");
        assert!(Rcn::strong_count(&b) == 2);
        assert!(Rcn::strong_count(&a) == 2);
        drop(w);
        drop(a);
        assert!(Rcn::strong_count(&b) == 1);
        let c = b.share();
        assert!(Rcn::strong_count(&b) == 2);
        assert!(Rcn::strong_count(&c) == 2);
    }

    #[test]
    fn weak_count_test() {
        let a = Rcn::new(0);
        assert!(Rcn::strong_count(&a) == 1);
        assert!(Rcn::weak_count(&a) == 0);
        let w = Rcn::downgrade(&a);
        assert!(Rcn::strong_count(&a) == 1);
        assert!(Rcn::weak_count(&a) == 1);
        drop(w);
        assert!(Rcn::strong_count(&a) == 1);
        assert!(Rcn::weak_count(&a) == 0);
        let c = a.share();
        assert!(Rcn::strong_count(&a) == 2);
        assert!(Rcn::weak_count(&a) == 0);
        drop(c);
    }

    #[test]
    fn weak_self_cyclic() {
        struct Cycle {
            x: RefCell<Option<Weakn<Cycle>>>,
        }
        let mut a = Rcn::new(Cycle { x: RefCell::new(None) });
        let b = a.share().downgrade();
        *a.x.borrow_mut() = Some(b);

        let w = a.downgrade();
        assert!(!a.is_unique());
        drop(w);
        assert!(!a.is_unique());
    }

    #[test]
    fn get_mut_test() {
        let mut x = Rcn::new(3);
        x.set(&4);
        assert_eq!(*x, 4);
        let y = x.share();
        drop(y);
        assert!(x.is_some());
        let w = x.downgrade();
        drop(x);
        assert!(w.is_none());
    }
    
    #[test]
    fn test_cowrc_clone_weak() {
        let mut cow0 = Rcn::new(75);
        let cow1_weak = cow0.downgrade();
        assert!(75 == *cow0);
        assert!(75 == *cow1_weak.upgrade().unwrap());
        let v = *cow0 + 1;
        cow0.set(&v);
        assert!(76 == *cow0);
        assert!(cow1_weak.upgrade().is_some());
    }

    // #[test]
    // fn test_from_box() {
    //     let b: Box<u32> = Box::new(123);
    //     let r: Rcn<u32> = Rcn::from(b);

    //     assert_eq!(*r, 123);
    // }

    // #[test]
    // fn test_from_box_str() {
    //     use std::string::String;

    //     let s = String::from("foo").into_boxed_str();
    //     let r = Rcn::from(s);

    //     assert_eq!(&r[..], "foo");
    // }

}