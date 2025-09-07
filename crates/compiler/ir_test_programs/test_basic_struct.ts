struct Foo {
  fieldA: integer,
  fieldB: integer,
}

fn main() {
  let bar = "232"
  let aFoo = Foo { 60, 40 }
  let sum = (aFoo.fieldA) + (aFoo.fieldB)
  if (sum) > (59) {
    print("struct fields sum is 6", 22)
    print("\n", 2)
  }

  return;
}
