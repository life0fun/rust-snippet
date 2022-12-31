// In Rust, no "inheriting" the properties of a struct.
// Instead, when you are designing the relationship between objects do it
// in a way that one's functionality is defined by an interface (a trait).
// no hierarchy, just object property traits and behavior traits.
// object trait impls behavior trait for all objects.
// To reuse parent fn, parent trait(Animal) impls fn(Speaks) trait.
// impl<T> Speaks for T where T: Animal { }
// Any T that impl Animal will get Speaks trait impl also.
// dyn Animal must behind a pointer, pass by pointer. &dyn Animal or Box<dyn Animal>

trait Animal {
  // exclude the fn from trait object to make trait object safe.
  fn new() -> Self where Self: Sized;
  fn kind(&self) -> &str;
  fn noise(&self) -> &str;
  // trait is not object safe ! can only take &self, not self as value b/c trait is not sized.
  // trait object &mut Trait is fat pointer.(value + vtabe);
  // fn take_value(self) {}
}

trait Speaks {
  fn speak(&self);
}

impl<T> Speaks for T
where
  T: Animal,
{
  fn speak(&self) {
      println!("Animal {} {}", self.kind(), self.noise());
  }
}

struct Dog {}
impl Animal for Dog {
  fn new() -> Dog { Dog{} }
  fn kind(&self) -> &str { "dog" }
  fn noise(&self) -> &str { "won" }
}
struct Cat {}
impl Animal for Cat {
  fn new() -> Cat { Cat{} }
  fn kind(&self) -> &str { "cat" }
  fn noise(&self) -> &str { "miao" }
}
// https://doc.bccnsoft.com/docs/rust-1.36.0-docs-html/edition-guide/rust-2018/trait-system/impl-trait-for-returning-complex-types-with-ease.html
// `impl Trait` for returning closure s without have to return trait object Box heap allocation.
// fn foo() -> impl Trait {};    // concrete value can be in stack
// fn foo() -> Box<dyn Trait> {} // concrete value has to be in heap with trait object redirection.
// can only return a single concrete type when return `impl Trait`.
fn new_dog() -> impl Animal {
  Dog{}
}
fn new_cat() -> Cat { // impl Animal {
  Cat {}
}
fn new_animal() -> Box<dyn Animal> {
  if rand::random() { Box::new(Dog{}) } else { Box::new(Cat{}) }
}

// fn Foo<T>() generic over single type T.
fn animal_talk_generic<T: Animal>(animal: &T) {
  println!("animal_talk {} {}", animal.kind(), animal.noise());
}
// In fn args, impl Trait is generic and &dyn Trait is dynamic dispatching.
// fn Speak<T: Animal>(_: T) == fn Speak(v: impl Animal) {}  // consume v of some type that impl Animal.
// fn Speak<T: Animal>(_: &T) == fn Speak(v: &impl Animal)   // borrow &v
fn animal_talk_generic_impl(animal: &impl Animal) {
  println!("animal_talk {} {}", animal.kind(), animal.noise());
}
// &Animal: trait object must include the dyn keyword.
// dyn Animal must behind a pointer, pass by pointer. &dyn Animal or Box<dyn Animal>
// https://www.ncameron.org/blog/dyn-trait-and-impl-trait-in-rust/
fn animal_talk_dyn(animal: &dyn Animal) {
  println!("animal_talk_dyn {} {}", animal.kind(), animal.noise());
}
// Box pointer: Box<impl Animal> == Box<dyn Animal>
fn animal_talk_boxed(animal: Box<impl Animal>) {
  println!("animal_talk_boxed {} {}", animal.kind(), animal.noise());
}

struct ZooGeneric<T: Animal> {
  animals: Vec<T>
}
struct ZooDyn {
  animals: Vec<Box<dyn Animal>>
}
fn new_zoo_generic<T: Animal>() -> ZooGeneric<T> {
  let mut z = ZooGeneric::<T>{ animals: Vec::new()};
  z.animals.push(T::new());
  z
}
fn new_zoo_dyn() -> ZooDyn {
  let mut z = ZooDyn{ animals: Vec::new() };
  z.animals.push(Box::new(new_dog()));
  z.animals.push(Box::new(new_cat()));
  z.animals.push(new_animal());
  z
}

fn returns_closure_dyn() -> Box<dyn Fn(i32) -> i32> {
  Box::new(|x| x + 1)
}
fn returns_closure() -> impl Fn(i32) -> i32 {
  |x| x + 1
}
fn test_return_impl() {
  println!("{}", returns_closure_dyn()(2));
  println!("{}", returns_closure()(2));
}

// only trait fn can return T as it takes self. Normal fn can not return T.
// fn new_generic<T: Animal>(t: &T) -> &T { t }
// trait Into<T> { fn into(self) -> T; }

// https://users.rust-lang.org/t/trait-objects-and-the-sized-trait/14410/2
// Trait Object Safety: can create a trait object, &dyn Trait, can build the vtable for the trait ?
// Exclude trait method that not &self receiver, rets Self, contain generic, and requires self: Sized
// trait method that return Self and requires Self: Sized is excluded from vtable as trait method
// as Self is type erased, hence method is vtable can not take Self except in receiver.
// Does not have a where Self: Sized bound (receiver type of Self (i.e. self) implies this).
// fn need_sized(self) -> Self where Self: Sized;
// Trait clone { fn clone(&self) -> Self; }
//

mod FuntionItemTest {
trait Animal { fn speak(&self); }
struct Dog;
impl Animal for Dog {
    fn speak(&self) { println!("speak: dog"); }
}
struct Cat;
impl Animal for Cat {
    fn speak(&self) { println!("speak: cat"); }
}

fn add_one(x: i32) -> i32 { x + 1 }
impl Animal for fn(i32) -> i32 {
    fn speak(&self) { println!("speak: fn(i32) -> i32"); }
}
pub const ADD_PTR: fn(i32) -> i32 = add_one;

fn take_fn_ptr(f: fn(i32) -> i32, arg: i32) -> i32 {
    f(arg) + f(arg)
}
fn take_fn_ref(f: &fn(i32) -> i32, arg: i32) -> i32 {
    f(arg) + f(arg)
}
fn animal_speak(animal: &dyn Animal) {
    animal.speak();
}
fn animal_mut_speak(animal: *mut dyn Animal) {
    unsafe {&*animal}.speak();
}
fn test() {
    println!("take_fn_ptr: {}", take_fn_ptr(add_one, 5));
    //println!("take_fn_ref: {}", take_fn_ref(&add_one, 5));
    println!("take_fn_ref: {}", take_fn_ref(&ADD_PTR, 5));
    let d = Dog;
    animal_speak(&d);
    // animal_speak(&add_one);
    animal_speak(&ADD_PTR);
}
}

fn main() {
  let dog = Dog {};
  let cat = Cat {};

  animal_talk_generic(&dog);
  animal_talk_generic(&cat);

  animal_talk_generic_impl(&dog);
  animal_talk_generic_impl(&cat);

  animal_talk_dyn(&dog);
  animal_talk_dyn(&cat);

  animal_talk_boxed(Box::new(Dog {}));
  animal_talk_boxed(Box::new(Cat {}));

  dog.speak();
  cat.speak();
  new_dog();


  println!("------ new zoo generic animal ---- ");
  let z: ZooGeneric<Dog> = new_zoo_generic();
  for e in z.animals.iter() { println!("{} {}", e.kind(), e.noise()); }

  println!("------ new zoo box dyn animal ---- ");
  let z = new_zoo_dyn();
  for e in z.animals.iter() { println!("{} {}", e.kind(), e.noise()); }
}
