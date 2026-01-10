extern fn abort();

fn main() {
  let x: f64 = 2.75
  let y: f64 = 3
  let sum: f64 = (x) + (y)
  let correct_value: f64 = 5.75 

  if (sum) != (correct_value) {
    abort();
  }
  
  return
}
