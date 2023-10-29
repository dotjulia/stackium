#include <stdlib.h>
#include <stdio.h>

struct Node {
  int value;
  struct Node* next_;
};

int main() {
  // struct Node n1;
  struct Node n1, n2;
  struct Node* n3 = malloc(sizeof(struct Node));
  n1.next_ = &n2;
  n2.next_ = n3;
  n3->next_ = malloc(sizeof(struct Node));
  printf("Test");
  printf("Test");
  printf("Test");
  printf("Test");
  return 0;
}
