#![forbid(unsafe_code)]

pub fn main() {
    let mut x = alloc::<i32>(10);
    let y = x;
    *x = 20;
    println!("x={} y={}", *x, *y); // x=20 y=20
    
    // pointer arithmetics
    let a = alloc::<i32>(1);
    {
        alloc::<i32>(3);
        alloc::<i32>(3);
        alloc::<i32>(7);
    }
    
    let a_plus_3 : SafePtr<i32> = a + 3;
    println!("you dont understand: {}", *a_plus_3); // 7
    
    // more basic cases
    multiple_references();
    use_after_free();
    linked_list();
}

// examples:

fn multiple_references() {
    let mut multiple = alloc("hello");
    let mut references = multiple;

    *multiple = "dont work";
    *references = "works!";

    println!("multiple references: {}", *multiple);
}

fn use_after_free() {
    let mut a = alloc("no");

    free(a);
    *a = "yes";

    println!("use after free possible?: {}", *a);

    let b = alloc("no");
    *a = "yes";

    println!("really??: {}", *b);
}

fn linked_list() {
  #[derive(Default, Copy, Clone)]
  struct LinkedList<T> {
      pub prev: SafePtr<Self>,
      pub next: SafePtr<Self>,
      pub data: T,
  }

  fn walk<T: 'static + Default + Send + Sync>(ll: &SafePtr<LinkedList<T>>, f: impl Fn(&mut T) -> ()) {
    let cur = deref_mut(ll);
      f(&mut cur.data);
      if cur.next != SafePtr::null() {
          walk(&you_dont_understand2(deref(&ll)).next, f);
      }
  }

  let mut a = alloc(LinkedList { prev: SafePtr::null(), next: SafePtr::null(), data: "node_a" });

  {
    let mut b = alloc(LinkedList { prev: SafePtr::null(), next: SafePtr::null(), data: "node_b" });
    let mut c = alloc(LinkedList { prev: SafePtr::null(), next: SafePtr::null(), data: "node_c" });


    a.next = b;
      b.prev = a;
      b.next = c;
      c.prev = b;
  }

  println!("");
  walk(&a, |val| {
      print!("{} -> ", val);
  });
  println!("\n");

  a.next.next = SafePtr::null();

  println!("after removing c from chain, node_c is unreachable:");
  
  walk(&a, |val| {
      print!("{} -> ", val);
  });
  println!("");
}

// implementation details...

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Mutex,
    ops::{Add, Sub, AddAssign, SubAssign},
    default::Default,
};
use std::ops::DerefMut;
use std::ops::Deref;
use once_cell::sync::Lazy;

fn perfectly_safe<'a, 'b, T>(_: &'a &'b(), arg: &'b mut T) -> &'a mut T {
    arg
}

fn you_dont_understand<'a, T>(x: &'a mut T) -> &'static mut T {
    let not_partially_memory_safe: fn(_,&mut T) -> &'static mut T = perfectly_safe;
    not_partially_memory_safe(&&(), x)
}

static HEAPS: Lazy<Mutex<HashMap<TypeId, Box<dyn HeapDyn>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub struct Heap<T> {
    pub data: Vec<T>,
    pub free: Vec<bool>,
}

pub trait HeapDyn: Any + Send + Sync {
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: 'static + Send + Sync + Default> HeapDyn for Heap<T> {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Copy, Clone)]
pub struct SafePtr<T> {
    pub i: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> SafePtr<T> {
    pub fn null() -> Self {
        SafePtr {
            i: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T> PartialEq for SafePtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.i == other.i
    }
    
    fn ne(&self, other: &Self) -> bool {
        self.i != other.i
    }
}

impl<T> Default for SafePtr<T> {
    fn default() -> Self {
        SafePtr {
            i: 0,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: 'static + Default + Send + Sync> DerefMut for SafePtr<T> {
    fn deref_mut(&mut self) -> &mut T {
        you_dont_understand(&mut get_heap::<T>().data[self.i])
    }
}

impl<T: 'static + Default + Send + Sync> Deref for SafePtr<T> {
    type Target = T;

    fn deref(&self) -> &T {
        you_dont_understand2(&get_heap::<T>().data[self.i])
    }
}

// immutable version of perfect safety, just because DerefMut requires Deref trait
fn perfectly_safe2<'a, 'b, T>(_: &'a &'b(), arg: &'b T) -> &'a T {
    arg
}

fn you_dont_understand2<'a, T>(x: &'a T) -> &'static T {
    let not_partially_memory_safe: fn(_,&T) -> &'static T = perfectly_safe2;
    not_partially_memory_safe(&&(), x)
}

impl<T: Default> Heap<T> {
    pub fn new() -> Self {
        Heap {
            data: Vec::new(),
            free: Vec::new(),
        }
    }

    pub fn alloc(&mut self, value: T) -> SafePtr<T> {
        if self.data.len() == 0 {
            // so we could have 0 ptr
            self.data.push(T::default());
            self.free.push(false);
        }

        if let Some(i) = self.free.iter().position(|&f| f) {
            self.free[i] = false;
            self.data[i] = value;
            return SafePtr {
                i,
                _phantom: std::marker::PhantomData,
            };
        }
        
        self.data.push(value);
        self.free.push(false);
        SafePtr {
            i: self.data.len() - 1,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn free(&mut self, r: SafePtr<T>) {
        self.free[r.i] = true;
    }
}

macro_rules! impl_arithm_for_type {
    ($T: ty) => {
        impl<T> Add<$T> for SafePtr<T> {
            type Output = Self;

            fn add(self, offset: $T) -> Self {
                SafePtr {
                    i: ((self.i as $T) + offset) as usize,
                    _phantom: std::marker::PhantomData,
                }
            }
        }

        impl<T> Sub<$T> for SafePtr<T> {
            type Output = Self;

            fn sub(self, offset: $T) -> Self {
                SafePtr {
                    i: ((self.i as $T) - offset) as usize,
                    _phantom: std::marker::PhantomData,
                }
            }
        }

        impl<T> AddAssign<$T> for SafePtr<T> {
            fn add_assign(&mut self, offset: $T) {
                self.i = (self.i as $T + offset) as usize;
            }
        }

        impl<T> SubAssign<$T> for SafePtr<T> {
            fn sub_assign(&mut self, offset: $T) {
                self.i = (self.i as $T - offset) as usize;
            }
        }

    }
}

impl_arithm_for_type!(usize);
impl_arithm_for_type!(isize);
impl_arithm_for_type!(i32);

impl<T> Sub<SafePtr<T>> for SafePtr<T> {
    type Output = isize;

    fn sub(self, other: SafePtr<T>) -> isize {
        self.i as isize - other.i as isize
    }
}

pub fn get_heap<T: Default + Send + Sync>() -> &'static mut Heap<T> {
    let mut map = HEAPS.lock().unwrap();
    let heap_any = map
        .entry(TypeId::of::<T>())
        .or_insert_with(|| Box::new(Heap::<T>::new()));
    let heap = heap_any
        .as_any_mut()
        .downcast_mut::<Heap<T>>()
        .expect("");
    return you_dont_understand(heap);
}

pub fn alloc<T: 'static + Send + Sync + Default>(value: T) -> SafePtr<T> {
    get_heap::<T>().alloc(value)
}

pub fn free<T: 'static + Send + Sync + Default>(r: SafePtr<T>) {
    get_heap::<T>().free(r)
}

pub fn deref<T: 'static + Send + Sync + Default>(r: &SafePtr<T>) -> &'static T {
    you_dont_understand2(&get_heap::<T>().data[r.i])
}

pub fn deref_mut<T: 'static + Send + Sync + Default>(r: &SafePtr<T>) -> &'static mut T {
    you_dont_understand(&mut get_heap::<T>().data[r.i])
}
