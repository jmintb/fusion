extern fn abort();

fn main() {
  let x: f32 = 2.1
  let y: f32 = 3
  let sum: f32 = (x) + (y)
  let correct_value: f32 = 5.1

  if (sum) != (correct_value) {
    abort();
  }
  
  return
}
