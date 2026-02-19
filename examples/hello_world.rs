use rio::rt::Runtime;

fn main() {
    let rt = Runtime::new();

    rt.block_on(async {
        rio::spawn(async {
            println!("hello, world");
        });
    });
}
