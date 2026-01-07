extern fn abort();

fn main() {
  let x: i16 = 2
  let y: i16 = 3
  let sum: i16 = (x) + (y)
  let limit: i16 = 5

  if (sum) > (limit) {
    abort();
  }
  
  if (sum) < (limit) {
    abort();
  }


  return
}
