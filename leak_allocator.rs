#![forbid(unsafe_code)]

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Mutex,
    ops::{Add, Sub, AddAssign, SubAssign},
    default::Default,
};
use once_cell::sync::Lazy;

pub trait HeapDyn: Any + Send {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub struct Heap<T> {
    pub data: Vec<T>,
    pub free: Vec<bool>,
}

#[derive(Copy, Clone)]
pub struct SafePtr<T> {
    pub i: usize,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> PartialEq for SafePtr<T> {
    fn eq(&self, other: &Self) -> bool {
        self.i == other.i
    }
    
    fn ne(&self, other: &Self) -> bool {
        self.i != other.i
    }
}

// shit semantics hack
trait HeapData: Send + Copy + Sync + Default + 'static {}
impl<T: Send + Copy + Sync + Default + 'static> HeapData for T {}

impl<T: 'static + Default> Heap<T> {
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

    // Array allocation: allocate n contiguous elements
    pub fn alloc_array(&mut self, count: usize) -> SafePtr<T> {
        if count == 0 {
            panic!("Cannot allocate empty array");
        }

        if self.data.len() == 0 {
            // so we could have 0 ptr
            self.data.push(T::default());
            self.free.push(false);
        }

        // try to find n contiguous free slots
        let mut start_idx = None;
        let mut free_count = 0;
        for i in 0..self.free.len() {
            if self.free[i] {
                if free_count == 0 {
                    start_idx = Some(i);
                }
                free_count += 1;
                if free_count == count {
                    break;
                }
            } else {
                free_count = 0;
                start_idx = None;
            }
        }

        // if we found contiguous free slots, use them
        if let Some(start) = start_idx {
            if free_count == count {
                for offset in 0..count {
                    self.free[start + offset] = false;
                    self.data[start + offset] = T::default();
                }
                return SafePtr {
                    i: start,
                    _phantom: std::marker::PhantomData,
                };
            }
        }

        // Otherwise, append to the end
        let start = self.data.len();
        for _ in 0..count {
            self.data.push(T::default());
            self.free.push(false);
        }
        SafePtr {
            i: start,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn free(&mut self, r: SafePtr<T>) {
        self.free[r.i] = true;
    }

    // Free an array of n elements starting at pointer
    pub fn free_array(&mut self, r: SafePtr<T>, n: usize) {
        for offset in 0..n {
            if r.i + offset < self.free.len() {
                self.free[r.i + offset] = true;
            }
        }
    }
}

impl<T: 'static + Send + Default> HeapDyn for Heap<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl<T> SafePtr<T> {
    pub fn offset(self, offset: isize) -> Self {
        SafePtr {
            i: (self.i as isize + offset) as usize,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn add(self, offset: usize) -> Self {
        SafePtr {
            i: self.i + offset,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn sub(self, offset: usize) -> Self {
        SafePtr {
            i: self.i - offset,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn as_index(self) -> usize {
        self.i
    }

    pub fn null() -> Self {
        SafePtr {
            i: 0,
            _phantom: std::marker::PhantomData,
        }
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

static HEAPS: Lazy<Mutex<HashMap<TypeId, Box<dyn HeapDyn>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn with_heap<T: 'static + Send + Default, R>(f: impl FnOnce(&mut Heap<T>) -> R) -> R {
    let mut map = HEAPS.lock().unwrap();
    let heap_any = map
        .entry(TypeId::of::<T>())
        .or_insert_with(|| Box::new(Heap::<T>::new()));
    let heap = heap_any
        .as_any_mut()
        .downcast_mut::<Heap<T>>()
        .expect("");
    f(heap)
}

pub fn alloc<T: 'static + Send + Default>(value: T) -> SafePtr<T> {
    with_heap(|heap: &mut Heap<T>| heap.alloc(value))
}

pub fn alloc_array<T: 'static + Send + Default>(count: usize) -> SafePtr<T> {
    with_heap(|heap: &mut Heap<T>| heap.alloc_array(count))
}

pub fn store<T: 'static + Send + Default>(r: SafePtr<T>, value: T) {
    with_heap(|heap: &mut Heap<T>| heap.data[r.i] = value);
}

pub fn load<T: 'static + Send + Copy + Default>(r: SafePtr<T>) -> T {
    with_heap(|heap: &mut Heap<T>| heap.data[r.i])
}

pub fn free<T: 'static + Send + Default>(r: SafePtr<T>) {
    with_heap(|heap: &mut Heap<T>| heap.free(r));
}

pub fn free_array<T: 'static + Send + Default>(r: SafePtr<T>, n: usize) {
    with_heap(|heap: &mut Heap<T>| heap.free_array(r, n));
}

pub fn with_ref<T: 'static + Send + Default, R>(r: SafePtr<T>, f: impl FnOnce(&mut T) -> R) -> R {
    with_heap(|heap: &mut Heap<T>| f(&mut heap.data[r.i]))
}

// Examples:

fn multiple_references() {
    let multiple = alloc("hello");
    let references = multiple;

    store(multiple, "dont work");
    store(references, "works!");

    println!("multiple references: {}", load(multiple));
}

fn use_after_free() {
    let a = alloc("no");

    free(a);
    store(a, "yes");

    println!("use after free possible?: {}", load(a));

    let b = alloc("no");
    store(a, "yes");

    println!("really??: {}", load(b));
}

fn linked_list() {
    #[derive(Default, Copy, Clone)]
    struct LinkedList<T> {
        pub prev: SafePtr<Self>,
        pub next: SafePtr<Self>,
        pub data: T,
    }

    fn walk<T: HeapData>(ll: SafePtr<LinkedList<T>>, f: impl Fn(T) -> ()) {
        let cur = load(ll);
        f(cur.data);
        if cur.next != SafePtr::null() {
            walk(cur.next, f);
        }
    }

    let a = alloc(LinkedList { prev: SafePtr::null(), next: SafePtr::null(), data: "node_a" });

    {
        let b = alloc(LinkedList { prev: SafePtr::null(), next: SafePtr::null(), data: "node_b" });
        let c = alloc(LinkedList { prev: SafePtr::null(), next: SafePtr::null(), data: "node_c" });


        with_ref(a, |a| {
            a.next = b;
        });
        with_ref(b, |b| {
            b.prev = a;
            b.next = c;
        });
        with_ref(c, |c| {
            c.prev = b;
        });
    }

    println!("");
    walk(a, |val| {
        print!("{} -> ", val);
    });
    println!("\n");

    // if you wrote here nested with_ref, you got deadlock because of fearless concurrency
    with_ref(load(a).next, |b| {
        b.next = SafePtr::null();
    });

    println!("after removing c from chain, node_c is unreachable:");
    
    walk(a, |val| {
        print!("{} -> ", val);
    });
    println!("");
}

fn main() {
    multiple_references();
    use_after_free();
    linked_list();
    
    let arr = alloc_array::<i32>(5);
    let third = arr + 2;
    let first = third - 2;
    let distance = third - first;
    
    let mut ptr = arr;
    ptr += 3;
    ptr -= 1;
}
