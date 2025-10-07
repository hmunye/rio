use std::time::{Duration, Instant};

fn main() {
    let rt = rio::rt::Runtime::new();
    let time = Instant::now();

    rt.block_on(async {
        println!("timer tasks...");

        rio::spawn(async move {
            let computed = 1 + 2;
            rio::time::sleep(Duration::from_secs(5)).await;
            println!("task 1 (5s delay) completed: {computed}");
        });

        rio::spawn(async move {
            let computed = 2 + 2;
            rio::time::sleep(Duration::from_secs(3)).await;
            println!("task 2 (3s delay) completed: {computed}");
        });

        rio::spawn(async move {
            let computed = 3 + 2;
            rio::time::sleep(Duration::from_secs(1)).await;
            println!("task 3 (1s delay) completed: {computed}");
        });
    });

    println!("total time elapsed: {}", time.elapsed().as_secs());
}
