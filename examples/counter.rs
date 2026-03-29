async fn counter() {
    let id = rio::task::id();

    for i in 0..10 {
        println!("task #{id}: {i}");
        // Yield control to the runtime, allowing other ready tasks to make
        // progress.
        rio::task::yield_now().await;
    }
}

#[rio::main]
async fn main() {
    rio::spawn(counter());
    rio::spawn(counter());
}
