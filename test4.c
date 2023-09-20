

struct Test {
  char* name;
  int arr[2][3];
};

int main() {
  char* test = "Hello";
  int arr[2][3] = {{1,2,3},{4,5,6}};
  struct Test teststruct = {.name = test, .arr = {
    {1,2,3},
    {4,5,6},
    },
  };
  void* voidptr = 0;
  return 0;
}
