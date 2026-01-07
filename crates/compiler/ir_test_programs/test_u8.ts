extern fn abort();

fn main() {
  let x: u8 = 2
  let y: u8 = 3
  let sum: u8 = (x) + (y)
  let limit: u8 = 5

  if (sum) > (limit) {
    abort();
  }
  
  if (sum) < (limit) {
    abort();
  }


  return
}
