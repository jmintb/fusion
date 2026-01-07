extern fn abort();

fn main() {
  let x: isize = 2
  let y: isize = 3
  let sum: isize = (x) + (y)
  let limit: isize = 5

  if (sum) > (limit) {
    abort();
  }
  

  if (sum) < (limit) {
    abort();
  }


  return
}
