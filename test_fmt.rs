fn main() {
    let s = "z";
    let length = 3;
    let formatted = format!("{s:0>length$}");
    println!("'{}'", formatted);
}
