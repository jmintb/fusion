extern fn abort();

fn main() {
  let x: u16 = 2
  let y: u16 = 3
  let sum: u16 = (x) + (y)
  let limit: u16 = 5

  if (sum) > (limit) {
    abort();
  }
  
  if (sum) < (limit) {
    abort();
  }


  return
}
