extern fn abort();

fn main() {
  let x: i8 = 2
  let y: i8 = 3
  let sum: i8 = (x) + (y)
  let limit: i8 = 5

  if (sum) > (limit) {
    abort();
  }
  
  if (sum) < (limit) {
    abort();
  }


  return
}
