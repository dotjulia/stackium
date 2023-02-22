#include <stdio.h>
#include <unistd.h>

void printing()
{
    printf("Hello World!\n");
}

int main()
{
    fprintf(stderr, "First!\n");
    fprintf(stderr, "Second!\n");
    fprintf(stderr, "%p\n", printing);
    printing();
    fprintf(stderr, "Finishing!\n");
}