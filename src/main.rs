fn main() {
    let rt = rio::rt::Runtime::new();

    rt.block_on(async {
        println!("blocking on main task");
        let res = 1 + 2;

        rio::spawn(async move {
            println!("printing result in spawned task 1: {res}");
        });

        rio::spawn(async move {
            println!("printing result in spawned task 2: {res}");
        });

        rio::spawn(async move {
            println!("printing result in spawned task 3: {res}");
        });
    })
}
