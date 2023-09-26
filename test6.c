#include <stdlib.h>
#include <string.h>


int main() {
  char* test = "This is a string";
  char* test3 = test;
  char** test2 = &test;
  char* heap_test = malloc(17);
  strcpy(heap_test, test);

  return 0;
}
