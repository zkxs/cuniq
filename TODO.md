# Planned Features

Many of these goals are probably out of scope for the project, but who knows?

- allow extra delimiters within a line?
- custom allocator?
- multithread. This would only be worth it if the overhead of merging state is covered by the speedup. This would also
  be a non-trivial refactor to implement.
- statistical estimates
  - ~~hyperloglog (the standard, supports some very nice features such as add, delete, union, etc)~~
    - implement the large cardinality fix (there's a TODO for this but Wikipedia is unclear about 32 vs 64 bit impls)
    - yo merge is actually super fucking cheap so we can multithread with minimal overhead
  - CVM (relatively new, very simple code, unclear if it is useful IRL beyond its simplicity to teach)
    - yeah idk... they claim "you don't need hashing" but then you have to do a ton of string equality checks instead? 
      I fail to see how that's better.
- write some docs on parallelism (GNU sort has it)
- implement a `--print` flag that prints each unique element
  - report kinda already does this, users just awk it or some shit.
    - Yeah `cuniq -cs hamlet_words.txt | awk '{print $2}'` does the thing.
    - IO is slow enough that awk won't even add that much overhead percent-difference-wise.
    - LMAO it's actually faster to awk it in a terminal, because terminal text rendering is very slow and this renders less text 🙃
