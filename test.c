#include <stdio.h>
#include <unistd.h>

void printing()
{
    printf("Hello World!\n");
}

void func1()
{
    printing();
}

void func2()
{
    printing();
}

int main()
{
    fprintf(stderr, "First!\n");
    fprintf(stderr, "Second!\n");
    fprintf(stderr, "%p\n", printing);
    func1();
    func2();
    fprintf(stderr, "Finishing!\n");
}