#include <stdlib.h>
#include <stdio.h>

struct Node {
  int value;
  struct Node* next_;
};

int main() {
  // struct Node n1;
  struct Node* n1[] = {malloc(sizeof(struct Node)), NULL};
  n1[0]->next_ = malloc(sizeof(struct Node));
  n1[1] = n1[0]->next_;
  n1[0]->next_->next_ = malloc(sizeof(struct Node));
  n1[0]->next_->next_->next_ = malloc(sizeof(struct Node));  // printf("Test");
  // printf("Test");
  // printf("Test");
  // printf("Test");
  return 0;
}
