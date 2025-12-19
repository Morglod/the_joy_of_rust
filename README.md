# the_joy_of_rust

Rules:

* no unsafes
* no unstable features

[playground](https://play.rust-lang.org/?version=stable&mode=debug&edition=2024&gist=c320d29e2253582cece6821e10a3dd3b)

## Examples

Multiple mutable references
```rs
    let mut x = alloc::<i32>(10);
    let y = x;
    *x = 20;
    println!("x={} y={}", *x, *y); // x=20 y=20
```

Pointer arithmetics
```rs
    let a = alloc::<i32>(1);
    {
        alloc::<i32>(3);
        alloc::<i32>(3);
        alloc::<i32>(7);
    }
    
    let a_plus_3 : SafePtr<i32> = a + 3;
    println!("you dont understand: {}", *a_plus_3); // 7
```

Use after free
```rs
    let mut a = alloc("no");

    free(a);
    *a = "yes";

    println!("use after free possible?: {}", *a);

    let b = alloc("no");
    *a = "yes";

    println!("really??: {}", *b);
```

  <details>
<summary>Linked list</summary>

  ```rs
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
```

</details>
