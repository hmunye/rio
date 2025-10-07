use std::time::{Duration, Instant};

fn main() {
    let rt = rio::rt::Runtime::new();

    let time = Instant::now();

    rt.block_on(async {
        println!("blocking on main task");
        let res = 1 + 2;

        rio::spawn(async move {
            rio::time::sleep(Duration::from_secs(5)).await;
            println!("printing result in spawned task 1: {res}");
        });

        rio::spawn(async move {
            rio::time::sleep(Duration::from_secs(3)).await;
            println!("printing result in spawned task 2: {res}");
        });

        rio::spawn(async move {
            rio::time::sleep(Duration::from_secs(2)).await;
            println!("printing result in spawned task 3: {res}");
        });
    });

    println!("time elapsed: {}", time.elapsed().as_secs())
}
