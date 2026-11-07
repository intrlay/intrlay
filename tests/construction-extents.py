"""Changing getter sizes must not publish partially described buffers."""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'target/construction-extents'))
from bindings import CountedView, ExtentView


class Changing(dict):
    def __init__(self, key, first, second):
        super().__init__({key: first})
        self.first, self.second = first, second
        self.reads = 0

    def __getitem__(self, key):
        self.reads += 1
        return self.first if self.reads == 1 else self.second


for cls, key, small, large in [
    (CountedView, 'values', 1, 2),
    (ExtentView, 'value', None, {}),
]:
    for first, second in [(small, large), (large, small)]:
        backing = bytearray([0xa5]) * 96
        for create in [
            cls.create,
            lambda value: cls.create_in_place(memoryview(backing)[16:80], value),
            lambda value: cls.create_in_place(cls.create({key: large}).buffer, value),
        ]:
            try:
                create(Changing(key, first, second))
            except ValueError as error:
                assert str(error) == 'Intrlay layout changed during construction'
            else:
                raise AssertionError('changed extent was published')
        assert backing[:16] == backing[80:] == bytes([0xa5]) * 16
        recovered = cls.create_in_place(memoryview(backing)[16:80], {key: large})
        assert bytes(recovered.buffer) == bytes(cls.create({key: large}).buffer)

same_size = CountedView.create(Changing('values', [1], [42]))
assert list(same_size.values) == [42]
print('Python: dynamic extents, borrowed slices, reuse and recovery passed')
