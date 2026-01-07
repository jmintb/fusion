extern fn abort();

fn main() {
  let x: usize = 2
  let y: usize = 3
  let sum: usize = (x) + (y)
  let limit: usize = 5

  if (sum) > (limit) {
    abort();
  }
  

  if (sum) < (limit) {
    abort();
  }


  return
}
