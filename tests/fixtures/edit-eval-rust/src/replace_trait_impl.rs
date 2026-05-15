pub trait Greeter {
    fn greet(&self) -> String;
}

pub struct Foo;

impl Greeter for Foo {
    fn greet(&self) -> String {
        String::from("hi")
    }
}
