This crate is similar to the [origin-start example], except that doesn't
print any output, and enables optimizations for small code size. To produce a
small binary, compile with `--release`. And experimentally, on this example
program, the BFD linker produces smaller output than gold, lld, or mold.

To produce an even smaller binary, use `objcopy` to remove the `.eh_frame`
and `.comment` sections:

```console
objcopy -R .eh_frame -R .comment target/release/tiny even-smaller
```

On x86-64, this produces a executable with size 408 bytes.

How is this achieved?

## The optimizations

First, to make this example really simple, we change it from printing a message
to just returning the number 42.

Next, `origin` makes much of its functionality optional, so we add
`default-features = false` when declaring the `origin` dependency, to disable
things like `.init_array`/`.fini_array` support, thread support, and other
things that our simple example doesn't need. We only enable the features
needed for our minimal test program:

```toml
origin = { path = "../..", default-features = false, features = ["origin-start"] }
```

Then, we add a `#[profile.release]` section to our Cargo.toml, to enable
several optimizations:

```toml
# Give the optimizer more latitude to optimize and delete unneeded code.
lto = true
# "abort" is smaller than "unwind".
panic = "abort"
# Tell the optimizer to optimize for size.
opt-level = "z"
# Delete the symbol table from the executable.
strip = true
```

In detail:

> `lto = true`

LTO is Link-Time Optimization, which gives the optimizer the ability to see the
whole program at once, and be more aggressive about deleting unneeded code.

For example, in our test program, it enables inlining of the `origin_main`
function into the code in `origin` that calls it. Since it's just returning the
constant `42`, inlining reduces code size.

> `panic = "abort"`

Rust's default panic mechanism is to perform a stack unwind, however that
mechanism takes some code.

This doesn't actually help our example here, since it's a minimal program that
doesn't contain any `panic` calls, but it's a useful optimization feature in
general.

> `opt-level = "z"`

The "z" optimization level instructs the compiler to prioritize code size above
all other considerations.

For example, on x86-64, in our test program, it uses this code sequence to load
the value `0x2a`, which is our return value of `42`, into the `%rdi` register
to pass to the exit system call:

```asm
  4000bc:	6a 2a                	push   $0x2a
  4000be:	5f                   	pop    %rdi
```

Compare that with the sequence it emits without "z":

```asm
  4000c1:	bf 2a 00 00 00       	mov    $0x2a,%edi
```

The "z" form is two instructions rather than one. It also does a store and a
load, as well as a stack pointer subtract and add. Modern x86-64 processors do
store-to-load forwarding to avoid actually writing to memory and have a Stack
engine for `push`/`pop` sequences and are very good at optimizing those kinds
of instruction sequences; see Agner's
[The microarchitecture of Intel, AMD, and VIA CPUs] for more information.
However, even with these fancy features, it's probably still not completely
free.

But it is 3 bytes instead of 5, so `opt_level = "z"` goes with it.

Amusingly, it doesn't do this same trick for the immediately following
instruction, which looks similar:
```asm
  4000bd:	b8 e7 00 00 00       	mov    $0xe7,%eax
```

Here, the value being loaded is 0xe7, which has the eigth bit set. The x86
`push` instructions immediate field is signed, so `push $0xe7` would need a
4-byte immediate field to zero-extend it. Consequently, using the `push`/`pop`
trick in this case would be longer.

Next, we enable several link arguments in build.rs:

```rust
    // Tell the linker to exclude the .eh_frame_hdr section.
    println!("cargo:rustc-link-arg=-Wl,--no-eh-frame-hdr");
    // Tell the linker to make the text and data readable and writeable. This
    // allows them to occupy the same page.
    println!("cargo:rustc-link-arg=-Wl,-N");
    // Tell the linker to exclude the `.note.gnu.build-id` section.
    println!("cargo:rustc-link-arg=-Wl,--build-id=none");
    // Disable PIE, which adds some code size.
    println!("cargo:rustc-link-arg=-Wl,--no-pie");
    // Disable the `GNU-stack` segment, if we're using lld.
    println!("cargo:rustc-link-arg=-Wl,-z,nognustack");
```

In detail:

> `--no-eh-frame-hdr`

This disables the creation of a `.eh_frame_hdr` section, which we don't need
since we won't be doing any unwinding.

> `-N`

This sets code sections to be writable, so that they can be loaded into memory
together with data. Ordinarily, having read-only code is a very good thing,
but making them writable can save a few bytes.

A related flag is `-n`, which may help code size in some cases, but we don't
use it here because it's not available in the mold linker.

The `-n` and `-N` flags don't actually help our example here, but they may save
some code size in larger programs.

> `--build-id=none`

This disables the creation of a `.note.gnu.build-id` section, which is used by
some build tools. We're not using any extra tools in our simple example here,
so we can disable this.

> `--no-pie`

Position-Independent Executables (PIE) are executables that can be loaded into
a random address in memory, to make some kinds of security exploits harder,
though it takes some extra code and relocation metadata to fix up addresses
once the actual runtime address has been determined. Our simple example here
isn't concerned with security, so we can disable this feature and save the
space.

> -z nognustack

This option is only recognized by ld.lld, so if you happen to be using that,
this disables the use of the `GNU-stack` feature which allows the OS to mark
the stack as non-executable. A non-executable stack is a very good thing, but
omitting this request does save a few bytes.

Finally, we add a RUSTFLAGS flag with .cargo/config.toml:

```toml
rustflags = ["-Z", "trap-unreachable=no"]
```

This disables the use of trap instructions, such as `ud2` on x86-64, at places
the compiler thinks should be unreachable, such as after the `jmp` in `_start`
or after the `syscall` that calls `exit_group`, because rustix uses the
`noreturn` [inline asm option]. Normally this is a very good thing, but it
does take a few extra bytes.

[inline asm option]: https://doc.rust-lang.org/reference/inline-assembly.html#options

## Generated code

With all these optimizations, the generated code looks like this:

```asm
00000000004000b0 <.text>:
  4000b0:	48 89 e7             	mov    %rsp,%rdi
  4000b3:	55                   	push   %rbp
  4000b4:	e9 00 00 00 00       	jmp    0x4000b9
  4000b9:	6a 2a                	push   $0x2a
  4000bb:	5f                   	pop    %rdi
  4000bc:	b8 e7 00 00 00       	mov    $0xe7,%eax
  4000c1:	0f 05                	syscall
```

Those first 3 instructions are origin's `_start` function. The next 5
instructions are `origin::program::entry` and everything, including the user
`origin_main` function and the `exit_group` syscall inlined into it.

## Optimizations not done

In theory this code be made even smaller.

That first `mov $rsp,%rdi` is moving the incoming stack pointer we got from the
OS into the first argument register to pass to `origin::program::entry` so that
it can use it to pick up the command-line arguments, environment variables, and
AUX records. However we don't use any of those, so we don't need that argument.
In theory origin could put that behind a cargo flag, but I didn't feel like
adding separate versions of the `_start` sequence just for that optimization.

Also, in theory, `origin::program::entry` could use the
`llvm.frameaddress intrinsic` to read the incoming stack pointer value, instead
of needing an explicit argument. But having it be an explicit argument makes it
behave more like normal Rust code, which shouldn't be peeking at its caller's
stack memory.

We could omit the `push %rbp` and `jmp`, which are just zeroing out the return
address so that nothing ever unwinds back into the `_start` code, and instead
fall through to the immediately following code, to make a "call" from the
`[naked]` function `_start` written in asm to the Rust `origin::program::entry`
function. This is the transition from assembly code to the first Rust code in
the program. There are sneaky ways to arrange for this code to be able to
fall-through from `_start` into the `origin::program::entry`, but as above, I'm
aiming to have this code behave like normal Rust code, which shouldn't be using
control flow paths that the compiler doesn't know about.

We could also use the `exit` syscall instead of `exit_group`, which in rustix
means using `exit_thread` instead of `exit_group`. This would exit only the
current thread, but that's fine because our tiny program only ever uses one
thread. And it would be slightly smaller on some architectures because the
syscall number for `exit` is lower than the number for `exit_group`. For
example, on x86-64, `exit` is 60 while `exit_group` is 231. That would enable
the compiler to use the same `push`/`pop` trick it does for the 42 constant,
saving 2 bytes. In theory origin could have a feature to enable this, however
it's a very minor optimization, and it would introducue undefined behavior if
somehow some thread got created outside of origin, so I chose not to add it.

## Sources

Many of these optimizations came from the following websites:

 - [Minimizing Rust Binary Size], a great general-purpose resource.
 - [A very small Rust binary indeed], a great resource for more extreme
   code-size optimizations.

[origin-start example]: https://github.com/sunfishcode/origin/blob/main/example-crates/origin-start/README.md
[The microarchitecture of Intel, AMD, and VIA CPUs]: https://www.agner.org/optimize/microarchitecture.pdf
[Minimizing Rust Binary Size]: https://github.com/johnthagen/min-sized-rust
[A very small Rust binary indeed]: https://darkcoding.net/software/a-very-small-rust-binary-indeed/
