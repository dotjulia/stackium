#include <stdlib.h>
#include <stdio.h>

struct Node {
  int value;
  struct Node* next_;
};

int main() {
  // struct Node n1;
  struct Node* n1 = malloc(sizeof(struct Node));
  n1->next_ = malloc(sizeof(struct Node));
  n1->next_->next_ = malloc(sizeof(struct Node));
  n1->next_->next_->next_ = malloc(sizeof(struct Node));  // printf("Test");
  // printf("Test");
  // printf("Test");
  // printf("Test");
  return 0;
}
