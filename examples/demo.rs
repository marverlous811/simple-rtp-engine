#[tokio::main]
async fn main() {
  let result = public_ip_address::perform_lookup(None).await.unwrap();
  println!("{}", result);
}
