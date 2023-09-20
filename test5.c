
struct Node {
  int value;
  struct Node* next_;
};

int main() {
  struct Node n1, n2, n3, n4;
  n1.next_ = &n2;
  n2.next_ = &n3;
  n3.next_ = &n4;
  return 0;
}
