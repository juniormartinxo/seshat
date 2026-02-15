from typing import List


def add_numbers(a: int, b: int) -> int:
    return a + b


def first_item(values: list[int]) -> int:
    return values[0]


result = add_numbers(1, "2")
print(first_item(["a", "b"]))
