use std::time::Duration;

#[rio::main]
async fn main() {
    println!("timer tasks start...");

    rio::spawn(async {
        rio::time::sleep(Duration::from_secs(5)).await;
        println!("task #{} (5s delay) completed", rio::task::id());
    });

    rio::spawn(async {
        rio::time::sleep(Duration::from_secs(3)).await;
        println!("task #{} (3s delay) completed", rio::task::id());
    });

    rio::spawn(async {
        rio::time::sleep(Duration::from_secs(1)).await;
        println!("task #{} (1s delay) completed", rio::task::id());
    });
}
