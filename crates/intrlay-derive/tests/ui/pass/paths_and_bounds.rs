use wire::Intrlay;

// `wire` is a Cargo dependency rename, not an extern-crate alias.
#[derive(Debug, PartialEq, Intrlay)]
struct Renamed { value: u32 }

mod facade { pub use wire as runtime; }
#[derive(Debug, PartialEq, wire::Intrlay)]
#[intrlay(crate = "crate::facade::runtime")]
struct Reexport { value: u32 }

#[derive(Debug, PartialEq, Intrlay)]
struct A { b: Option<Box<B>> }
#[derive(Debug, PartialEq, Intrlay)]
struct B { a: Option<Box<A>> }

#[derive(Debug, PartialEq, Intrlay)]
#[intrlay(no_field_bounds)]
struct Tree<T: Intrlay<Value = T>> { children: Vec<Tree<T>>, value: T }

trait Types { type Value; }
struct Provider;
impl Types for Provider { type Value = u32; }
#[derive(Intrlay)]
#[intrlay(bound = "T::Value: wire::Intrlay<Value = T::Value>")]
struct Associated<T: Types> { value: T::Value }

#[derive(Intrlay)]
struct Qualified<T: Types> { value: <T as Types>::Value }

fn main() {
    let first = RenamedView::create(&Renamed { value: 5 }).unwrap();
    assert_eq!(first.value(), 5);
    assert_eq!(Renamed::from(first), Renamed { value: 5 });
    assert_eq!(ReexportView::create(&Reexport { value: 9 }).unwrap().value(), 9);
    let a = A { b: Some(Box::new(B { a: None })) };
    assert_eq!(A::from(AView::create(&a).unwrap()), a);
    let tree = Tree { children: vec![Tree { children: vec![], value: 3u32 }], value: 7 };
    assert_eq!(Tree::from(TreeView::create(&tree).unwrap()), tree);
    assert_eq!(AssociatedView::create(&Associated::<Provider> { value: 13 }).unwrap().value(), 13);
    assert_eq!(QualifiedView::create(&Qualified::<Provider> { value: 17 }).unwrap().value(), 17);
}
