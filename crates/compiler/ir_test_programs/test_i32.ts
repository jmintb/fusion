extern fn abort();

fn main() {
  let x: i32 = 2
  let y: i32 = 3
  let sum: i32 = (x) + (y)
  let correct_value: i32 = 5

  if (sum) != (correct_value) {
    abort();
  }
  
  return
}
