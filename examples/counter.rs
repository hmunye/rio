async fn counter() {
    let id = rio::task::id();

    for i in 0..10 {
        println!("task #{id}: {i}");
        rio::task::yield_now().await;
    }
}

#[rio::main]
async fn main() {
    rio::spawn(counter());
    rio::spawn(counter());
}
