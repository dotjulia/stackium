#include <stdio.h>
#include <unistd.h>

int test(int* a) {
    int* b = a;
    return *b;
}

int main() {
    int a = 0x89ABCDEF;
    int* b = &a;
    test(b);
    int** c = &b;
    int g = *b;
    int* d = *c;
    {
        int a = 4;
        int v = 3;
    }
    __asm__("nop");
    __asm__("nop");
}