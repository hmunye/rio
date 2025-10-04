fn main() {
    let rt = rio::rt::Runtime::new();

    let res = rt.block_on(async { 1 + 2 });

    println!("result: {res}");
}
