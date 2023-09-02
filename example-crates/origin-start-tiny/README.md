This crate is similar to the [`origin-start` example], except that doesn't
print any output, and enables optimizations for small code size. To produce a
small binary, compile with `--release`.

To produce an even smaller binary, use `objcopy` to remove the `.eh_frame`
and `.comment` sections:

```
objcopy -R .eh_frame -R .comment target/release/origin-start-tiny even-smaller
```

For details on the specific optimizations performed, see the options under
`[profile.release]` and the use of `default-features = false`, in Cargo.toml,
and the additional link flags passed in build.rs.

## The optimizations

First, `origin` makes much of its functionality optional, so we add
`default-features = false` to disable things like `.init_array`/`.fini_array`
support, thread support, and other things. We only enable the features needed
for our minimal test program:

```toml
origin = { path = "../..", default-features = false, features = ["origin-program", "origin-start"] }
```

Then, we enable several optimizations in the `#[profile.release]` section of
Cargo.toml:

```toml
# Give the optimizer more lattitude to optimize and delete unneeded code.
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

For example, in our test program, it enables inlining of the `main` function
into the code in `origin` that calls it. Since it's just returning the constant
`42`, inlining reduces code size.

> `panic = "abort"`

Rust's default panic mechanism is to perform a stack unwind, however that
mechanism takes some code.

This doesn't actally help our example here, since it's a minimal program that
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
```
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
    // Tell the linker not to page-align sections.
    println!("cargo:rustc-link-arg=-Wl,-n");
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

> `-n`

This turns of page alignment of sections, so that we don't waste any space on
padding bytes.

> `-N`

This sets code sections to be writable, so that they can be loaded into memory
together with data. Ordinarily, having read-only code is a very good thing,
but making them writable can save a few bytes.

The `-n` and `-N` flags dont actally help our example here, but they can save
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
  4000b9:	50                   	push   %rax
  4000ba:	6a 2a                	push   $0x2a
  4000bc:	5f                   	pop    %rdi
  4000bd:	b8 e7 00 00 00       	mov    $0xe7,%eax
  4000c2:	0f 05                	syscall 
```

Those first 3 instructions are origin's `_start` function. The next 5
instructions are `origin::program::entry` and everything, including the user
`main` function and the `exit_group` syscall inlined into it.

In theory this code code be made even smaller.

That first `mov $rsp,%rdi` is moving the incoming stack pointer we got from the
OS into the first argument register to pass to `origin::program::entry` so that
it can use it to pick up the command-line arguments, environment variables, and
AUX records, however we don't use any of those, so we don't need that argument.
In theory origin could put that behind a cargo flag, but I didn't feel like
adding separate versions of the `_start` sequence just for that optimization.

Also, in theory, `origin::program::entry` could use the
`llvm.frameaddress intrinsic` to read the incoming stack pointer value, instead
of needing an explicit argument. But having it be an explicit argument makes it
behave more like normal Rust code, which shouldn't be peeking at its caller's
stack memory.

And lastly, we could enable the `push %rbp`, `jmp`, and `push %rax`, which are
just zeroing out the return address so that nothing ever unwinds back into the
`_start` code, jumping to the immediately following code, and aligning the
stack pointer, all to make a "call" from the `[naked]` function `_start` written
in asm to the Rust `origin::program::entry` function. This is the transition
from assembly code to the first Rust code in the program. There are sneaky ways
to arrange for this code to be able to fall-through from `_start` into the
`origin::program::entry`, but as above, I'm aiming to have this code behave
like normal Rust code, which shouldn't be using control flow paths that the
compiler doesn't know about.

## Sources

Many of these optimizations came from the following websites:

 - [Minimizing Rust Binary Size](https://github.com/johnthagen/min-sized-rust),
   a great general-purpose resource.
 - [A very small Rust binary indeed](https://darkcoding.net/software/a-very-small-rust-binary-indeed/),
   a great resource for more extreme code-size optimizations.

[origin-start example]: https://github.com/sunfishcode/origin/blob/main/example-crates/origin-start/README.md
[The microarchitecture of Intel, AMD, and VIA CPUs]: https://www.agner.org/optimize/microarchitecture.pdf
