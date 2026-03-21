use rio::task::JoinError;

async fn counter() -> u64 {
    let id = rio::task::id();

    for i in 0..10 {
        println!("task {id}: {i}");
        // Yield control to the runtime to allow other tasks to run.
        rio::task::yield_now().await;
    }

    id.raw()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = rio::runtime::Runtime::new();

    let val: Result<u64, JoinError> = rt.block_on(async {
        let x = rio::spawn(counter());
        let y = rio::spawn(counter());

        Ok(x.await? + y.await? + rio::task::id().raw())
    });

    println!("yielded: {val:?}");

    Ok(())
}
