#include <stdlib.h>
#include <stdio.h>

struct Node {
  struct {
    int a[2]; int b;
  } value;
  struct Node* next_;
};

int main() {
  // struct Node n1;
  char* abc = "This is a test";
  struct Node* n1[] = {malloc(sizeof(struct Node)), NULL};
  n1[0]->next_ = malloc(sizeof(struct Node));
  n1[0]->value.a[0] = 0x123456;
  n1[0]->value.a[1] = 0x123456;
  n1[0]->value.b = 0x123456;
  n1[1] = n1[0]->next_;
  n1[0]->next_->next_ = malloc(sizeof(struct Node));
  n1[0]->next_->value.a[0] = 0xDEF123;
  n1[0]->next_->value.a[1] = 0xDEF123;
  n1[0]->next_->value.b = 0xDEF123;
  n1[0]->next_->next_->next_ = malloc(sizeof(struct Node));  // printf("Test");
  n1[0]->next_->next_->value.a[0] = 0xDEF123;
  n1[0]->next_->next_->value.a[1] = 0xDEF123;
  n1[0]->next_->next_->value.b = 0xDEF123;
  n1[0]->next_->next_->next_->next_ = NULL;
  n1[0]->next_->next_->next_->value.a[0] = 'A';
  n1[0]->next_->next_->next_->value.a[1] = 'B';
  n1[0]->next_->next_->next_->value.b = 'C';
  
  printf("abc");
  printf("%s", abc);
  return 5;
}
