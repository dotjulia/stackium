#include <stdio.h>
#include <unistd.h>


int main() {
    int a = 6;
    int* b = &a;
    int** c = &b;
    int g = *b;
    int* d = *c;
    __asm__("nop");
    __asm__("nop");
}