
fn print_string(let val: String, allocator: StringAllocator) {
     let val_projection = project_raw_string(val, allocator);
     print(val_projection, val.length);
     return;
}

fn project_raw_string(let val: String, let allocator: StringAllocator) -> ptr {
  let string_ptr = pointer_from_offset(allocator.memory_pool, val.memory_offset);
  return string_ptr;
}


fn memory_write_wrapper(owned value: ptr, owned destination: ptr, owned length_in_bytes: i32, owned offset_in_bytes: i32) {
  let offset_destination = pointer_from_offset(destination, offset_in_bytes);
  write_bytes(value, offset_destination, length_in_bytes);
  return
}



struct StringAllocator {
  memory_pool: ptr,
  size: i64,
  offset: i32,
}

fn allocate_for_string(inout allocator: StringAllocator, owned length: i32) -> i32 {
  let string_offset = allocator.offset;
  let new_offset = (string_offset) + (length) ;
  allocator.offset = new_offset;
  return string_offset
}

fn init_string_allocator() -> StringAllocator {
  let size: i64 = 5000;
  let memory = malloc(size);
  
  return StringAllocator { memory, size, 0 };
}

struct String {
  memory_offset: i32,
  length: i32,
}

fn new_string(inout allocator: StringAllocator, owned length: i32) -> String {
  let string_offset = allocate_for_string(allocator, length);
  return String { string_offset, length };
}

fn main() {
  let a = "testt\n"
  let b = "empty\n"
  let allocator = init_string_allocator();
  let new_string = new_string(allocator, 6);
  let new_string_b = new_string(allocator, 6);

  memory_write_wrapper(a, allocator.memory_pool, new_string.length, new_string.memory_offset);
  memory_write_wrapper(b, allocator.memory_pool, new_string_b.length, new_string_b.memory_offset);
  print_string(new_string, allocator);
  print_string(new_string_b, allocator);
  print_string(new_string_b, allocator);

  return
}
