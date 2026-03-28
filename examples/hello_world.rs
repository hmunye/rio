async fn foo() {
    println!("running task #{}", rio::task::id());

    for _ in 1..10 {
        println!("hello world");
    }
}

#[rio::main]
async fn main() {
    rio::spawn(foo());
    println!("running task #{}", rio::task::id());
}
