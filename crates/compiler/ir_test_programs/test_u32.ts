extern fn abort();

fn main() {
  let x: u32 = 2
  let y: u32 = 3
  let sum: u32 = (x) + (y)
  let limit: u32 = 5

  if (sum) > (limit) {
    abort();
  }
  
  if (sum) < (limit) {
    abort();
  }


  return
}
