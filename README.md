# the_joy_of_rust

Making fun out of safety cult.

Rules:

* no unsafes
* no unstable features

Some examples:

```rs
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
```

  <details>
<summary>Linked list</summary>

  ```rs
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
```

</details>
