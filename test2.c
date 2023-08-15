#include <stdio.h>
#include <unistd.h>

int test(int* a) {
    int* b = a;
    return *b;
}

int main() {
    int a = 6;
    int* b = &a;
    test(b);
    int** c = &b;
    int g = *b;
    int* d = *c;
    __asm__("nop");
    __asm__("nop");
}