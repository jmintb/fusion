extern fn abort();

fn main() {
  let x: i64 = 2
  let y: i64 = 3
  let sum: i64 = (x) + (y)
  let limit: i64 = 5

  if (sum) > (limit) {
    abort();
  }
  

  if (sum) < (limit) {
    abort();
  }


  return
}
