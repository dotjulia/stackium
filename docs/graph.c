#include <stdio.h>
#include <stdlib.h>

typedef struct _Node_ {
  int data_;
  struct _Node_* next_;
} Node;


int main()
{
  int count;
  printf("Enter count: ");
  scanf("%d", &count);
  Node* head = NULL;
  // insert at beginning
  for (int i = 0; i < count; ++i) {
    Node* node = (Node*)malloc(sizeof(Node));
    printf("Enter data: ");
    scanf("%d", &node->data_);
    node->next_ = head;
    head = node;
  }
  // print
  for (Node* node = head; node != NULL; node = node->next_) {
    printf("%d ", node->data_);
  }
}
