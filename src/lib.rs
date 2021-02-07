//! Single-threaded reference-counting pointers with none values. `Rcn` stands for 'Reference Counted with None values'.
//!
//! The `Rcn<T>` provides shared ownership of a value of type `T`, allocated in the heap. The pointed-to value is only destroyed after the last `Rcn` is destroyed
//! 
//! The type `Rcn<T>` is similar to `Rc<T>` in standard library, but it has some differences
//! 
//! 
//! 
//! 
//! 
//! 
//! 
//! 
//!
//! 
//!  
//! [`Rcn`]: struct.Rcn.html
//! [`Weakn`]: struct.Weakn.html
//! [clone]: ../../std/clone/trait.Clone.html#tymethod.clone
//! [`Cell`]: ../../std/cell/struct.Cell.html
//! [`RefCell`]: ../../std/cell/struct.RefCell.html
//! [send]: ../../std/marker/trait.Send.html
//! [`Deref`]: ../../std/ops/trait.Deref.html
//! [downgrade]: struct.Rcn.html#method.downgrade
//! [upgrade]: struct.Weakn.html#method.upgrade
//! [`None`]: ../../std/option/enum.Option.html#variant.None

// use std::any::Any;
use std::marker::PhantomData;
#[allow(unused_imports)]
use std::ptr::{self, NonNull};
use std::cell::Cell;
#[allow(unused_imports)]
use std::alloc::{GlobalAlloc, Layout, System, handle_alloc_error};
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::cmp::Ordering;
use std::mem::{self, forget};
// use std::mem::align_of_val;
use std::rc::Rc;
// use std::any::Any;

struct RcnBox<T: ?Sized> {
    strong: Cell<usize>,
    weak: Cell<usize>,
    value: T,
}


// impl<T> RcnBox<T>{
//     pub fn new<'a>(mut self, data: T) -> &'a mut Self where T: 'a
//     {
//         // let a = RcnBox::<T> {
//         //     strong: Cell::new(1),
//         //     weak: Cell::new(0),
//         //     value: data,
//         // };
//         self.strong = Cell::new(1);
//         self.weak = Cell::new(0);
//         self.value = data;
//         &mut self
//     }
// }

/// A single-threaded reference-counting pointer with none value. `Rcn` stands for 'Reference Counted with None values'.
/// 
/// 
/// 
/// 

pub struct Rcn<T: ?Sized>{
    ptr: *mut RcnBox<T>,
    phantom: PhantomData<T>,
}

#[allow(dead_code)]
impl<T> Rcn<T> {
    /// Constructs a new `Rcn<T>`.
    ///
    /// # Example
    ///
    /// ```no_run
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let ten = Rcn::new(10);
    /// assert_eq!(ten.is_some(), true);
    /// ```
    pub fn new<'a>(data: T) -> Rcn<T> where T: 'a{
        Rcn::<T> {
            ptr: Box::into_raw(Box::new(RcnBox::<T> {
                        strong: Cell::new(1),
                        weak: Cell::new(0),
                        value: data,
                    })),
            // ptr: Box::leak(Box::new(RcnBox::<T> {
            //         strong: Cell::new(1),
            //         weak: Cell::new(0),
            //         value: data,
            //     })),
            phantom: PhantomData,
        }
    }

    /// Constructs a `Rcn<T>` with none value. 
    ///
    /// # Example
    ///
    /// ```no_run
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let ten: Rcn<i32> = Rcn::none();
    /// assert_eq!(ten.is_none(), true);
    /// ```
    pub fn none() -> Rcn<T> {
        Rcn::<T> {
            ptr: ptr::null_mut(),
            phantom: PhantomData,
        }
    }

    /// Constructs a `Rcn<T>` with none value. 
    /// Takes the value out of the option, leaving a None in its place. Returns `Some(T)` if the current `Rcn` pointer is unique, and `None` otherwise. It is unique if `weak_count == 0` and `strong_count == 1`.
    /// # Example
    ///
    /// ```no_run
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let mut t1: Rcn<i32> = Rcn::new(100);
    /// let mut t2: Rcn<i32> = t1.share();
    /// assert_eq!(t1.is_unique(), false);
    /// assert_eq!(t1.take(), None);
    /// drop(t1);
    /// assert_eq!(t2.is_unique(), true);
    /// assert_eq!(t2.take(), Some(100));
    /// assert_eq!(t2.is_none(), true);
    /// let mut t3: Rcn<i32> = Rcn::none();
    /// assert_eq!(t3.take(), None);
    /// ``` 
    #[inline]
    pub fn take(&mut self) -> Option<T> {
        unsafe {
            if self.is_unique() {
                let out_ptr = self.ptr;
                self.ptr = 0 as *mut RcnBox<T>;
                Some(out_ptr.read().value)
            } else {
                None
            }
        }
    }

    
    /// Returns the contained value, if the `Rcn` has exactly one strong reference.
    ///
    /// Otherwise, an [`Err`][result] is returned with the same `Rcn` that was
    /// passed in.
    ///
    /// This will succeed even if there are outstanding weak references.
    ///
    /// [result]: https://doc.rust-lang.org/std/result/enum.Result.html
    ///
    /// # Examples
    ///
    /// ```no_run
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let x = Rcn::new(3);
    /// assert_eq!(Rcn::try_unwrap(x), Ok(3));
    ///
    /// let x = Rcn::new(4);
    /// let _y = Rcn::share(&x);
    /// assert_eq!(*Rcn::try_unwrap(x).unwrap_err(), 4);
    /// ```
    #[inline]
    pub fn try_unwrap(this: Self) -> Result<T, Self> {
        if this.strong_count() == 1 {
            unsafe {
                let val = ptr::read(&*this); // copy the contained object

                this.dec_strong();

                this.inc_weak();
                let _weak = Weakn { ptr: this.ptr };
                
                forget(this);
                Ok(val)
            }
        } else {
            Err(this)
        }
    }
}

#[allow(dead_code)]
impl<T: ?Sized> Rcn<T> {

    /// Gets the number of strong (`Rcn`) pointers to this value.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let ten = Rcn::new(10);
    /// let shared_ten = ten.share();
    ///
    /// assert_eq!(2, shared_ten.strong_count());
    /// assert_eq!(2, ten.strong_count());
    /// ```
    #[inline]
    pub fn strong_count(&self) -> usize {
        self.strong()
    }

    /// Gets the number of weak (`Rcn`) pointers to this value.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let ten = Rcn::new(10);
    /// let weak_ten = ten.downgrade();
    ///
    /// assert_eq!(1, ten.weak_count());
    /// assert_eq!(1, ten.strong_count());
    /// ```
    #[inline]
    pub fn weak_count(&self) -> usize {
        self.weak()
    }

    /// Returns `true` if the current `Rcn` pointer is not shared with others `Rcn` or `Weakn` pointers. It is unique if `weak_count == 0` and `strong_count == 1`.
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///    
    /// let x = Rcn::new(3);
    /// assert!(x.is_unique());  // weak_count == 0 and strong_count == 1
    /// let y = x.share();
    /// assert!(!x.is_unique()); // weak_count == 0 and strong_count == 2
    /// drop(y);
    /// assert!(x.is_unique());  // weak_count == 0 and strong_count == 1
    ///
    /// let a = x.downgrade();
    /// assert!(!x.is_unique()); // weak_count == 1 and strong_count == 1
    /// ```
    #[inline]
    pub fn is_unique(&self) -> bool {
        self.weak_count() == 0 && self.strong_count() == 1
    }

    /// Returns `true` if the current `Rcn` pointer is `None`.
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///    
    /// let n1: Rcn<u32> = Rcn::none();
    /// assert!(n1.is_none());                // Value is 'None'
    /// let n2 = Rcn::new(100);
    /// assert!(!n2.is_none());               // Value is 100
    /// let n3: Rcn<i32>;                
    /// //assert!(n3.is_none());              // ERROR: use of possibly uninitialized
    /// let mut n4: Rcn<u32> = Rcn::none();
    /// //n4.set(&10);                        // ERROR: write (set) in none rcn!
    /// n4 = Rcn::new(10);                    // OK
    /// assert!(!n4.is_none());               // Value is 10
    /// ```
    #[inline]
    pub fn is_none(&self) -> bool {
        self.strong() == 0  || self.ptr.is_null()
    }

    /// Returns `true` if the current `Rcn` pointer is not `None`.
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///    
    /// let n1: Rcn<u32> = Rcn::none();
    /// assert!(!n1.is_some());                // Value is 'None'
    /// let n2 = Rcn::new(100);
    /// assert!(n2.is_some());               // Value is 100
    /// let n3: Rcn<i32>;                
    /// //assert!(n3.is_some());              // ERROR: use of possibly uninitialized
    /// let mut n4: Rcn<u32> = Rcn::none();
    /// //n4.set(&10);                        // ERROR: write (set) in none rcn!
    /// n4 = Rcn::new(10);                    // OK
    /// assert!(n4.is_some());               // Value is 10
    #[inline]
    pub fn is_some(&self) -> bool {
        self.strong() > 0 && !self.ptr.is_null()
    }

    /// Returns true if the two `Rcn`s point to the same value (not
    /// just values that compare as equal).
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let ptr1 = Rcn::new(5);
    /// let ptr2 = ptr1.share();
    /// let ptr3 = Rcn::new(5);
    /// let ptr4 = ptr1.clone();
    ///
    /// assert!(Rcn::ptr_eq(&ptr1, &ptr2));
    /// assert!(!Rcn::ptr_eq(&ptr1, &ptr3));
    /// assert!(!Rcn::ptr_eq(&ptr1, &ptr4));
    /// ```
    #[inline]
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.ptr == other.ptr
    }

    /// This creates another pointer to the same inner value, increasing the strong reference count.
    ///
    /// NOTE: The `share()` have the same functionality that `clone()` of `Rc` pointer in the std library.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let ptr = Rcn::new(80);
    /// let mut shared_ptr = ptr.share();
    ///
    /// assert_eq!(80, ptr.get());
    /// assert_eq!(80, shared_ptr.get());
    /// assert_eq!(80, *ptr);
    /// assert_eq!(80, *shared_ptr);
    ///
    /// shared_ptr.set(&90);
    ///
    /// assert_eq!(90, ptr.get());
    /// assert_eq!(90, shared_ptr.get());
    /// ```
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


    /// Creates a new [`Weakn`][weakn] pointer to this value. NOTE: This function don't destroy current Rcn pointer. 
    ///
    /// [weakn]: struct.Weakn.html
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let five = Rcn::new(5); //strong_count = 1 and weak_count = 0
    ///
    /// let weak_five = Rcn::downgrade(&five); //strong_count = 1 and weak_count = 1
    /// ```
    pub fn downgrade(&self) -> Weakn<T> {
        self.inc_weak();
        let address = self.ptr as *mut () as usize;
        debug_assert!(address != usize::max_value());
        Weakn { ptr: self.ptr }
    }

    /// Consumes the `Rcn`, returning the wrapped pointer.
    ///
    /// # Examples
    ///
    /// ```
    /// extern crate rcn;
    /// use rcn::Rcn;
    ///
    /// let x = Rcn::new(10);
    /// let x_ptr = Rcn::into_raw(x);
    /// assert_eq!(unsafe { *x_ptr }, 10);
    /// ```
    pub fn into_raw(this: Self) -> *const T {
        let ptr: *const T = &*this;
        mem::forget(this);
        ptr
    }

    pub fn into_mut_raw(mut this: Self) -> *mut T {
        let ptr: *mut T = &mut *this;
        mem::forget(this);
        ptr
    }

    pub unsafe fn from_raw(ptr: *const T) -> Rcn<T> where T: Clone{
        let v = ptr.as_ref().unwrap();

        let rcn = Rcn::<T> {
            ptr: Box::into_raw(Box::new(RcnBox::<T> {
                    strong: Cell::new(1),
                    weak: Cell::new(0),
                    value: (*v).clone(),
                })),
            phantom: PhantomData,
        };
        
        mem::forget(ptr);

        rcn
    }

    #[inline]
    fn strong(&self) -> usize {
        if self.ptr.is_null() {
            0
        } else {
            unsafe { self.ptr.as_ref().unwrap().strong.get() }
        }
        
    }

    #[inline]
    fn inc_strong(&self) {

        if self.strong() == usize::max_value() {
            panic!("abort inc strong");
        }
        unsafe { self.ptr.as_ref().unwrap().strong.set(self.strong() + 1); }
    }

    #[inline]
    fn dec_strong(&self) {
        if self.strong() == usize::min_value() {
            panic!("abort dec strong");
        }

        unsafe { 
            self.ptr.as_ref().unwrap().strong.set(self.strong() - 1);
        }
    }

    #[inline]
    fn weak(&self) -> usize {
        if self.ptr.is_null() {
            0
        } else {
            unsafe { self.ptr.as_ref().unwrap().weak.get() }
        }
    }

    #[inline]
    fn inc_weak(&self) {
        if self.weak() == usize::max_value() {
            panic!("abort inc weak");
        }
        unsafe { self.ptr.as_ref().unwrap().weak.set(self.weak() + 1);}
    }

    #[inline]
    fn dec_weak(&self) {
        if self.weak() == usize::min_value() {
            panic!("abort dec weak");
        }
        unsafe { self.ptr.as_ref().unwrap().weak.set(self.weak() - 1); }
    }
}

#[allow(dead_code)]
impl<T: Clone> Rcn<T> {
    ///Get a clone of internal data
    #[inline(always)]
    pub fn get(&self) -> T {
        if self.is_some() {
            unsafe {
                self.ptr.as_ref().unwrap().value.clone()
            }
        } else {
            panic!("access (get) of none rcn!");
        }
    }

    #[inline(always)]
    pub fn set(&mut self, data: &T) {
        if self.is_some() {
            unsafe {
                self.ptr.as_mut().unwrap().value = data.clone();
            }
        } else {
            panic!("write (set) in none rcn!\n \t help: Use Rcn:new(...) to none pointers");
        }
    }
}

impl<T: Clone> Clone for Rcn<T> {
    #[inline]
    fn clone(&self) -> Rcn<T> {
        if self.is_some() {
            unsafe {
                Rcn::<T> {
                    ptr: Box::into_raw(Box::new(RcnBox {
                            strong: Cell::new(1),
                            weak:  Cell::new(0),
                            value: self.ptr.as_ref().unwrap().value.clone(),
                        })),
                    phantom: PhantomData,
                }
            }
        } else {
            Rcn::none()
        }
    }
}

impl <T: ?Sized> Drop for Rcn<T> {
    fn drop(&mut self) {
        if self.is_some() {
            self.dec_strong();
            unsafe { 
                if self.strong() == 0 {
                    ptr::drop_in_place(self.ptr);
                    System.dealloc(self.ptr as *mut u8, Layout::for_value(self.ptr.as_ref().unwrap())); 
                }
            }
        }
    }
}

impl<T: ?Sized> Deref for Rcn<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        if self.is_some() {
            unsafe {
                &self.ptr.as_ref().unwrap().value
            }
        } else {
            panic!("deref of none rcn!");
        }
    }
}

impl<T: ?Sized> DerefMut for Rcn<T> {
    
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        if self.is_some() {
            unsafe {
                &mut self.ptr.as_mut().unwrap().value
            }
        } else {
            panic!("deref_mut of none rcn!");
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


impl<T: ?Sized> From<Box<T>> for Rcn<T> where T: Clone {
    #[inline]
    fn from(v: Box<T>) -> Rcn<T> {
        
        unsafe {
            let bptr = Box::into_raw(v);
            let nnptr = NonNull::new_unchecked(bptr);
            let cptr: *const T = nnptr.as_ref();

            Rcn::<T>::from_raw(cptr)
        }
    }
}

impl<T: ?Sized> From<Rc<T>> for Rcn<T> where T: Clone {
    #[inline]
    fn from(v: Rc<T>) -> Rcn<T> {
        unsafe {
            let cptr = Rc::into_raw(v);
            Rcn::<T>::from_raw(cptr)
        }
    }
}

// impl Rcn<dyn Any> {
//     #[inline]
//     /// Attempt to downcast the `Rc<dyn Any>` to a concrete type.
//     ///
//     /// # Examples
//     ///
//     /// ```
//     /// extern crate rcn;
//     /// use rcn::Rcn;
//     /// 
//     /// use std::any::Any;
//     ///
//     /// fn print_if_string(value: Rcn<dyn Any>) {
//     ///     if let Ok(string) = value.downcast::<String>() {
//     ///         println!("String ({}): {}", string.len(), string);
//     ///     }
//     /// }
//     ///
//     /// fn main() {
//     ///     let my_string = "Hello World".to_string();
//     ///     print_if_string(Rcn::new(my_string));
//     ///     print_if_string(Rcn::new(0i8));
//     /// }
//     /// ```
//     pub fn downcast<T>(self) -> Result<Rcn<T>, Rcn<dyn Any>> where T: Any {
//         if (*self).is::<T>() {
//             let ptr = self.ptr as *mut RcnBox<T>;
//             forget(self);
//             Ok(Rcn::<T> { ptr: ptr, phantom: PhantomData })
//         } else {
//             Err(self)
//         }
//     }
// }

#[allow(dead_code)]
pub struct Weakn<T: ?Sized> {
    ptr: *mut RcnBox<T>,
}

#[allow(dead_code)]
impl<T> Weakn<T> {
    pub fn new() -> Weakn<T> {
        Weakn {
            ptr: ptr::null_mut(),
        }
    }

    pub fn none() -> Weakn<T> {
        Weakn::<T> {
            ptr: 0 as *mut RcnBox<T>,
        }
    }
}

#[allow(dead_code)]
impl<T: ?Sized> Weakn<T> {

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
        self.strong() == 0 || self.ptr.is_null()
    }

    #[inline]
    pub fn is_some(&self) -> bool {
        self.strong() > 0 && !self.ptr.is_null()
    }

    pub fn upgrade(&self) -> Option<Rcn<T>> {
        unsafe { 
            if self.ptr.as_ref().unwrap().strong.get() == 0 {
                return None
            }
        }
        self.inc_strong();
        Some(Rcn { ptr: self.ptr, phantom: PhantomData })
    }

       #[inline]
    fn strong(&self) -> usize {
        unsafe { self.ptr.as_ref().unwrap().strong.get() }
    }

    #[inline]
    fn inc_strong(&self) {

        if self.strong() == usize::max_value() {
            panic!("abort inc strong");
        }
        unsafe { self.ptr.as_ref().unwrap().strong.set(self.strong() + 1); }
    }

    #[inline]
    fn dec_strong(&self) {
        if self.strong() == usize::min_value(){
            panic!("abort dec strong");
        }

        unsafe { self.ptr.as_ref().unwrap().strong.set(self.strong() - 1); }
    }

    #[inline]
    fn weak(&self) -> usize {
        unsafe { self.ptr.as_ref().unwrap().weak.get() }
    }

    #[inline]
    fn inc_weak(&self) {
        if self.weak() == usize::max_value() {
            panic!("abort inc weak");
        }
        unsafe { self.ptr.as_ref().unwrap().weak.set(self.weak() + 1);}
    }

    #[inline]
    fn dec_weak(&self) {
        if self.weak() == usize::min_value() {
            panic!("abort dec weak");
        }
        unsafe { self.ptr.as_ref().unwrap().weak.set(self.weak() - 1); }
    }
}

impl<T: ?Sized> Drop for Weakn<T> {
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

impl<T: ?Sized + fmt::Debug> fmt::Debug for Weakn<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(Weakn)")
    }
}

impl<T: ?Sized + PartialEq> PartialEq for Weakn<T> {

    #[inline(always)]
    fn eq(&self, other: &Weakn<T>) -> bool {
        **self == **other
    }

    #[inline(always)]
    fn ne(&self, other: &Weakn<T>) -> bool {
        **self != **other
    }
}

impl<T: ?Sized + Eq> Eq for Weakn<T> {}

impl<T: ?Sized + PartialOrd> PartialOrd for Weakn<T> {

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

impl<T: ?Sized> Deref for Weakn<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        if self.is_some() {
            unsafe {
                &self.ptr.as_ref().unwrap().value
            }
        } else {
            panic!("deref of none weakn!");
        }
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

    // // Issue: unsizing fails when associated types are involved (#50213)
    // #[test] 
    // fn test_unsized() {
    //     let foo: Rcn<[i32]> = Rcn::new([1, 2, 3]);
    //     assert_eq!(foo, foo.share());
    // }

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
        let a = Rcn::new(Cycle { x: RefCell::new(None) });
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

    #[test]
    fn try_unwrap() {
        let x = Rcn::new(3);
        assert_eq!(Rcn::try_unwrap(x), Ok(3));
        let x = Rcn::new(4);
        let _y = x.share();
        assert_eq!(Rcn::try_unwrap(x), Err(Rcn::new(4)));
        let x = Rcn::new(5);
        let _w = x.downgrade();
        assert_eq!(Rcn::try_unwrap(x), Ok(5));
    }

    #[test]
    fn name() {
        let mut t1: Rcn<i32> = Rcn::new(100);
        let mut t2: Rcn<i32> = t1.share();
        assert_eq!(t1.is_unique(), false);
        assert_eq!(t1.take(), None);
        drop(t1);
        assert_eq!(t2.is_unique(), true);
        assert_eq!(t2.take(), Some(100));
        assert_eq!(t2.is_none(), true);
        let mut t3: Rcn<i32> = Rcn::none();
        assert_eq!(t3.take(), None);
    }

    #[test]
    fn into_from_raw() {
        let x = Rcn::new(Box::new("hello"));
        let y = x.share();

        let x_ptr = Rcn::into_raw(x);
        drop(y);
        unsafe {
            assert_eq!(**x_ptr, "hello");

            let x = Rcn::from_raw(x_ptr);
            assert_eq!(**x, "hello");

            assert_eq!(Rcn::try_unwrap(x).map(|x| *x), Ok("hello"));
        }
    }

    #[test]
    fn test_from_box() {
        let b: Box<u64> = Box::new(100);
        let r: Rcn<u64> = Rcn::from(b);

        println!("r = {:?}", r.is_none());

        assert_eq!(*r, 100);
    }

    #[test]
    fn test_from_box_str() {
        use std::string::String;

        let s = String::from("foofoofoo").into_boxed_str();
        let r = Rcn::from(s);

        assert_eq!(&r[..], "foofoofoo");
    }

}