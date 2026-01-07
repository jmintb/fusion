extern fn abort();

fn main() {
  let x: u64 = 2
  let y: u64 = 3
  let sum: u64 = (x) + (y)
  let limit: u64 = 5

  if (sum) > (limit) {
    abort();
  }
  
  if (sum) < (limit) {
    abort();
  }


  return
}
