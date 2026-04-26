use rio::task;

async fn counter() {
    let id = task::id();

    for i in 0..10 {
        println!("task #{id}: {i}");
        task::yield_now().await;
    }
}

#[rio::main]
async fn main() {
    rio::spawn(counter());
    rio::spawn(counter());
}
