extern fn abort();

fn main() {
  let x: i32 = 2
  let y: i32 = 3
  let sum: i32 = (x) + (y)
  let limit: i32 = 5

  if (sum) > (limit) {
    abort();
  }
  
  if (sum) < (limit) {
    abort();
  }


  return
}
