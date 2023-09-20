

struct Test2 {
  int number;
  char character;
};
struct Test {
  char* name;
  int arr[2][3];
  struct Test2 sub_struct;
};

int main() {
  char* test = "Hello";
  int arr[2][3] = {{1,2,3},{4,5,6}};
  struct Test teststruct = {
    .name = test,
    .arr = {
      {1,2,3},
      {4,5,6},
    },
    .sub_struct = {
      .number = 1234,
      .character = 'a',
    }
  };
  void* voidptr = 0;
  return 0;
}
