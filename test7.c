
void swap(int* a, int* b)
{
  int tmp = *a;
  *a = *b;
  *b = tmp;
}


int main()
{
  int first = 3;
  int second = 5;
  swap(&first, &second);
  return 0;
}
