
fn main() {
  let projected_str = project("test");
  print("before main \n", 13);
  print(projected_str, 4);
  return;
}

projection fn project(element: str) -> str {
  yield element;
}
