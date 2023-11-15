# Compact-Debug
<!-- cargo-rdme start -->

`{:#?}` formatting, and the `dbg!()` macro, sound nice on paper. But once you try using them...

```text
Goto(
    Address(
        30016,
    ),
),
Label(
    Address(
        29990,
    ),
),
Expr(
    Expr(
        Expr(
            [
                Var(
                    0,
                ),
                Const(
                    0,
                ),
                Op(
                    Ne,
                ),
            ],
        ),
    ),
    Address(
        30016,
    ),
),
```

Your dreams of nice and readable output are shattered by a chunk of output more porous than cotton
candy, with approximately two tokens of content on each line. Screenful upon screenful of vacuous
output for even a moderately complex type. Upset, you reluctantly replace your derived `Debug`
implementation with a manual one that eschews `DebugTuple` in favor of `write_str`. However, this
results in a catastrophic amount of boilerplate code, and doesn't affect types outside of your
control, like the ubiquitous `Option`.

That's where this crate comes in. It monkey-patches the pretty-printing machinery so that
`DebugTuple` is printed on a single line regardless of `#` flag. The above snippet is printed as:

```text
Goto(Address(30016)),
Label(Address(29990)),
Expr(Expr(Expr([
    Var(0),
    Const(0),
    Op(Ne),
])), Address(30016)),

This crate currently only supports x86_64 architecture.
```

<!-- cargo-rdme end -->
