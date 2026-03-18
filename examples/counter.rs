fn main() {
    let rt = rio::runtime::Runtime::new();

    let val = rt.block_on(async {
        println!("hello world");
        4
    });

    println!("yielded: {val}");
}
