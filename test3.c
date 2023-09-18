
int test(int _) {
  int a = 0;
  {
    int a = 1;
    {
      int a = 2;
      {
        int a = 3;
      }
    }
  }
}

int main() {
  int var = 4;
  int* ptr1 = &var;
  int* ptr2 = &var;
  int* ptr3 = &var;
  int* ptr4 = &var;
  int* ptr5 = &var;
  int* ptr6 = &var;
  {
    int** ptr7 = &ptr6;
    int*** ptr8 = &ptr7;
    int**** ptr9 = &ptr8;
  }
  test(var);
  (void)(0);
  (void)(0);
  (void)(0);
  (void)(0);
  (void)(0);
}
