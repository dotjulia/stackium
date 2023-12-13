#include <stdio.h>
#define NUMBER_COUNT 10

int main()
{
  int numbers[NUMBER_COUNT];
  // i < NUMBER_COUNT
  int* current_element = numbers;
  while(current_element <= numbers + NUMBER_COUNT)
  {
    *current_element = 0xABC;
    current_element++;
  }
}
