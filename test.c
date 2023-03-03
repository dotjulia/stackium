#include <stdio.h>
#include <unistd.h>

// typedef struct
// {
//     int a;
//     int b;
// } mystruct;

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
    int a = 123;
    int *b = &a;
    // mystruct s = {1, 2};
    fprintf(stderr, "First!\n");
    fprintf(stderr, "Second!\n");
    fprintf(stderr, "%p\n", printing);
    fprintf(stderr, "%d %d", a, *b);
    __builtin_trap();
    func1();
    func2();
    fprintf(stderr, "Finishing!\n");
}