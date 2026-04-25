# F-strings render as <string> with interpolation children and bare
# literal text in between (Principle #12: string_start / string_content /
# string_end grammar wrappers are flattened — they carry no semantic
# beyond their text value). Plain strings collapse to a text-only
# <string> element.
#
# Query: //string/interpolation/name='age' finds every interpolation of
# the `age` variable regardless of the surrounding literal text.

plain = "hello"
greeting = f"hello {name}"
status = f"hello {name}, you are {age}"
