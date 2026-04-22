# Goal #5 (mental model): augmented_assignment unifies with plain
# assignment as <assign> plus an <op> child carrying the compound
# operator. A single //assign query matches every assignment.

def ops():
    x = 0
    x += 1
    x -= 2
    x *= 3
    x //= 2
    x **= 2
    x &= 0xFF
    x |= 0x10
    x ^= 0x01
    x <<= 1
    x >>= 1
