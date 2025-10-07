use std::time::Duration;

async fn counter(id: usize, sleep: u64) {
    for i in 0..10 {
        println!("task {id}: {i}");
        // Yield control to the runtime to allow other tasks to run.
        rio::time::sleep(Duration::from_millis(sleep)).await;
    }
}

fn main() {
    let rt = rio::rt::Runtime::new();

    rt.block_on(async move {
        rio::spawn(async { counter(1, 100).await });
        rio::spawn(async { counter(2, 150).await });
        rio::spawn(async { counter(3, 200).await });
    });
}
