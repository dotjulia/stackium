#include <stdio.h>
#include <unistd.h>

void printing()
{
    printf("Hello World!\n");
}

int main()
{
    printf("%p\n", printing);
    printing();
}